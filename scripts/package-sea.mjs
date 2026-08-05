import { spawnSync } from 'node:child_process';
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import * as tar from 'tar';

const root = path.resolve(import.meta.dirname, '..');
const dist = path.join(root, 'dist');
const target = process.env.SEA_TARGET || `${process.platform}-${process.arch}`;
const [platform, arch] = target.split('-');
if (!['linux-x64', 'win32-x64'].includes(target)) {
  throw new Error(`Unsupported SEA target: ${target}. Supported targets: linux-x64, win32-x64`);
}
const executableName = `mpaas-amr-${platform}-${arch}${platform === 'win32' ? '.exe' : ''}`;
await fs.mkdir(dist, { recursive: true });

const compilerRoot = process.env.MINIDEV_COMPILER_SOURCE || path.join(os.homedir(), '.minidev', 'compilers');
const compilerVersion = process.env.MINIDEV_COMPILER_VERSION || 'cubebuild@0.100.12';
const compilerSource = path.join(compilerRoot, compilerVersion);
if (!fsSync.existsSync(compilerSource)) {
  throw new Error(`Compiler resources not found at ${compilerSource}. Run minidev download-assets --with-compiler first.`);
}

const staging = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-amr-runtime-'));
try {
  await fs.cp(path.join(root, 'node_modules'), path.join(staging, 'node_modules'), {
    recursive: true,
    filter: (source) => {
      const normalized = source.split(path.sep).join('/');
      return !normalized.includes('/node_modules/.bin/') && !normalized.endsWith('/node_modules/.bin');
    },
  });
  await fs.mkdir(path.join(staging, '.minidev'), { recursive: true });
  await fs.cp(compilerSource, path.join(staging, '.minidev', 'compilers', compilerVersion), { recursive: true });
} catch (error) {
  await fs.rm(staging, { recursive: true, force: true });
  throw error;
}

const runtimeArchive = path.join(dist, 'mpaas-amr-runtime.tar.gz');
await tar.c({ cwd: staging, file: runtimeArchive, gzip: true, portable: true }, [
  'node_modules',
  '.minidev/compilers',
]);
await fs.rm(staging, { recursive: true, force: true });

const bundle = spawnSync(path.join(root, 'node_modules', '.bin', 'esbuild'), [
  'src/cli.mjs',
  '--bundle',
  '--platform=node',
  '--format=cjs',
  '--external:minidev',
  '--external:fsevents',
  '--external:electron',
  '--outfile=dist/bundle.cjs',
], { cwd: root, stdio: 'inherit' });
if (bundle.status !== 0) process.exit(bundle.status || 1);

const seaConfig = JSON.parse(await fs.readFile(path.join(root, 'scripts', 'sea-config.json'), 'utf8'));
seaConfig.assets = { 'mpaas-amr-runtime.tar': runtimeArchive };
const generatedConfig = path.join(dist, 'sea-config.generated.json');
await fs.writeFile(generatedConfig, JSON.stringify(seaConfig, null, 2));

const result = spawnSync(process.execPath, ['--experimental-sea-config', generatedConfig], {
  cwd: root,
  stdio: 'inherit',
});
if (result.status !== 0) process.exit(result.status || 1);

const binary = path.join(dist, executableName);
const nodeBinary = process.env.SEA_NODE_BINARY || process.execPath;
if (!fsSync.existsSync(nodeBinary)) throw new Error(`Node runtime not found: ${nodeBinary}`);
await fs.copyFile(nodeBinary, binary);
if (platform !== 'win32') await fs.chmod(binary, 0o755);

// macOS requires the original signature to be removed before postject changes
// the Mach-O layout, then the final executable must be signed again.
if (platform === 'darwin') {
  const removeSignature = spawnSync('codesign', ['--remove-signature', binary], {
    cwd: root,
    stdio: 'inherit',
  });
  if (removeSignature.status !== 0) process.exit(removeSignature.status || 1);
}

const postject = path.join(root, 'node_modules', 'postject', 'dist', 'cli.js');
const injectArgs = [
  postject,
  binary,
  'NODE_SEA_BLOB',
  path.join(dist, 'mpaas-amr-sea.blob'),
  '--sentinel-fuse',
  'NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2',
];
if (platform === 'darwin') injectArgs.push('--macho-segment-name', 'NODE_SEA');

const inject = spawnSync(process.execPath, injectArgs, { cwd: root, stdio: 'inherit' });
if (inject.status !== 0) process.exit(inject.status || 1);

if (platform === 'darwin') {
  const sign = spawnSync('codesign', ['--force', '--sign', '-', binary], {
    cwd: root,
    stdio: 'inherit',
  });
  if (sign.status !== 0) process.exit(sign.status || 1);
}

console.log(`Standalone executable (${platform}/${arch}): ${binary}`);
