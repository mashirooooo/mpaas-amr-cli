import { spawnSync } from 'node:child_process';
import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { build as bundleCli } from 'esbuild';
import * as tar from 'tar';

process.on('uncaughtException', (error) => {
  console.error('[package:sea] uncaught exception', error);
  process.exit(1);
});
process.on('unhandledRejection', (error) => {
  console.error('[package:sea] unhandled rejection', error);
  process.exit(1);
});

const root = path.resolve(import.meta.dirname, '..');
const dist = path.join(root, 'dist');
const target = process.env.SEA_TARGET || `${process.platform}-${process.arch}`;
const [platform, arch] = target.split('-');
if (!['linux-x64', 'win32-x64', 'darwin-arm64'].includes(target)) {
  throw new Error(`Unsupported SEA target: ${target}. Supported targets: linux-x64, win32-x64, darwin-arm64`);
}
if (target !== `${process.platform}-${process.arch}`) {
  throw new Error(`SEA target ${target} must match the build machine ${process.platform}-${process.arch}. Build on the target platform.`);
}
const executableName = `mpaas-amr-${platform}-${arch}${platform === 'win32' ? '.exe' : ''}`;
await fs.mkdir(dist, { recursive: true });

const compilerRoot = process.env.MINIDEV_COMPILER_SOURCE || path.join(os.homedir(), '.minidev', 'compilers');
const compilerVersion = process.env.MINIDEV_COMPILER_VERSION || 'cubebuild@0.100.12';
const compilerSource = path.join(compilerRoot, compilerVersion);
console.log(`[package:sea] target=${target}`);
console.log(`[package:sea] compiler=${compilerSource}`);
if (!fsSync.existsSync(compilerSource)) {
  throw new Error(`Compiler resources not found at ${compilerSource}. Run minidev download-assets --with-compiler first.`);
}

const openapiName = `mpaas-openapi-rs${platform === 'win32' ? '.exe' : ''}`;
const openapiBinary = process.env.SEA_OPENAPI_BINARY
  ? path.resolve(process.env.SEA_OPENAPI_BINARY)
  : path.join(root, 'rust-sdk', 'target', 'release', openapiName);
if (!process.env.SEA_OPENAPI_BINARY) {
  const buildOpenapi = spawnSync(process.execPath, [path.join(root, 'scripts', 'build-openapi.mjs')], {
    cwd: root,
    stdio: 'inherit',
  });
  if (buildOpenapi.status !== 0) process.exit(buildOpenapi.status || 1);
}
if (!fsSync.existsSync(openapiBinary)) {
  throw new Error(`Native OpenAPI client not found: ${openapiBinary}. Run npm run build:openapi on the target platform.`);
}
const openapiHelp = spawnSync(openapiBinary, ['--help'], { encoding: 'utf8' });
if (openapiHelp.error || openapiHelp.status !== 0 || !openapiHelp.stdout.includes('mpaas-openapi-rs')) {
  throw new Error(`OpenAPI client cannot run on ${target}: ${openapiHelp.error?.message || openapiHelp.stderr || openapiBinary}`);
}
console.log(`[package:sea] OpenAPI client=${openapiBinary}`);

const staging = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-amr-runtime-'));
const runtimeArchive = path.join(dist, 'mpaas-amr-runtime.tar.gz');
console.log(`[package:sea] staging=${staging}`);
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
  await fs.mkdir(path.join(staging, 'bin'));
  const embeddedOpenapi = path.join(staging, 'bin', openapiName);
  await fs.copyFile(openapiBinary, embeddedOpenapi);
  if (platform !== 'win32') await fs.chmod(embeddedOpenapi, 0o755);
  console.log('[package:sea] archiving runtime');
  await tar.c({ cwd: staging, file: runtimeArchive, gzip: true, portable: true }, [
    'node_modules',
    '.minidev/compilers',
    'bin',
  ]);
} finally {
  await fs.rm(staging, { recursive: true, force: true });
}
console.log(`[package:sea] runtime archive=${runtimeArchive}`);

console.log('[package:sea] bundling CLI');
await bundleCli({
  absWorkingDir: root,
  entryPoints: [path.join(root, 'src', 'cli.mjs')],
  bundle: true,
  platform: 'node',
  format: 'cjs',
  external: ['minidev', 'fsevents', 'electron'],
  outfile: path.join(dist, 'bundle.cjs'),
  logLevel: 'info',
});

const seaConfig = JSON.parse(await fs.readFile(path.join(root, 'scripts', 'sea-config.json'), 'utf8'));
console.log('[package:sea] creating SEA blob');
seaConfig.assets = { 'mpaas-amr-runtime.tar': runtimeArchive };
const generatedConfig = path.join(dist, 'sea-config.generated.json');
await fs.writeFile(generatedConfig, JSON.stringify(seaConfig, null, 2));

const result = spawnSync(process.execPath, ['--experimental-sea-config', generatedConfig], {
  cwd: root,
  stdio: 'inherit',
});
if (result.status !== 0) process.exit(result.status || 1);

const binary = path.join(dist, executableName);
const nodeBinary = process.execPath;
console.log(`[package:sea] runtime=${nodeBinary}`);
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
