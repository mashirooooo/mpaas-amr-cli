import fs from 'node:fs/promises';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import * as tar from 'tar';
import { ensureMinidevBuildOptions } from './minidev-compat.mjs';

let runtimePromise;

async function extractRuntime() {
  const sea = await import('node:sea');
  if (!sea.isSea()) return null;

  const archive = Buffer.from(sea.getAsset('mpaas-amr-runtime.tar'));
  const hash = crypto.createHash('sha256').update(archive).digest('hex');
  const user = process.getuid?.() ?? os.userInfo().username;
  const cache = path.join(os.tmpdir(), `mpaas-amr-runtime-${user}`);
  const root = path.join(cache, hash);
  const ready = async () => (await fs.readFile(path.join(root, '.ready'), 'utf8').catch(() => '')) === hash;
  if (!(await ready())) {
    await fs.mkdir(cache, { recursive: true, mode: 0o700 });
    const staging = await fs.mkdtemp(path.join(cache, '.extract-'));
    try {
      const archivePath = path.join(staging, 'runtime.tar');
      await fs.writeFile(archivePath, archive);
      await tar.x({ file: archivePath, cwd: staging, strict: true });
      await fs.rm(archivePath);
      await fs.writeFile(path.join(staging, '.ready'), hash);
      try {
        await fs.rename(staging, root);
      } catch (error) {
        // A concurrent process may have finished publishing the same runtime.
        if (!(await ready())) throw error;
      }
    } finally {
      await fs.rm(staging, { recursive: true, force: true });
    }
  }
  process.env.MINIDEV_COMPILEDIR = path.join(root, '.minidev');
  return root;
}

export function prepareRuntime() {
  runtimePromise ??= extractRuntime();
  return runtimePromise;
}

export async function loadMinidev() {
  const runtime = await prepareRuntime();
  const require = runtime
    ? createRequire(path.join(runtime, 'node_modules', 'minidev', 'package.json'))
    : createRequire(new URL('../package.json', import.meta.url));
  const packageRoot = runtime
    ? path.join(runtime, 'node_modules', 'minidev')
    : path.dirname(require.resolve('minidev/package.json'));
  await ensureMinidevBuildOptions(packageRoot);
  return require('minidev');
}

export async function runOpenApi(configPath, { clientHelp = false, captureOutput = false } = {}) {
  const runtime = await prepareRuntime();
  const name = `mpaas-openapi-rs${process.platform === 'win32' ? '.exe' : ''}`;
  const binary = runtime
    ? path.join(runtime, 'bin', name)
    : process.env.MPAAS_OPENAPI_BINARY || fileURLToPath(new URL(`../rust-sdk/target/release/${name}`, import.meta.url));
  await fs.access(binary).catch(() => {
    throw new Error('Rust OpenAPI client is missing. Run npm run build:openapi before using the source CLI.');
  });
  return new Promise((resolve, reject) => {
    const child = spawn(binary, clientHelp ? ['--help'] : [path.resolve(configPath)], {
      stdio: captureOutput ? ['ignore', 'pipe', 'inherit'] : 'inherit',
    });
    let stdout = '';
    if (captureOutput) child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.once('error', reject);
    child.once('close', (code, signal) => {
      if (signal) reject(new Error(`OpenAPI client terminated by ${signal}`));
      else if (!captureOutput) resolve(code ?? 1);
      else {
        try { resolve({ code: code ?? 1, result: JSON.parse(stdout) }); }
        catch { reject(new Error('OpenAPI client did not return valid JSON')); }
      }
    });
  });
}

export async function runOpenApiConfig(config, options = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-openapi-'));
  try {
    const file = path.join(directory, 'config.json');
    await fs.writeFile(file, JSON.stringify(config), { mode: 0o600 });
    return await runOpenApi(file, options);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
}

export async function compileInChild(options, resultFile) {
  const sea = await import('node:sea');
  const args = sea.isSea() ? [] : [path.resolve(process.argv[1])];
  args.push('build', '--format', 'both', '--result-file', resultFile);
  for (const [field, flag] of Object.entries({
    project: '--project', appId: '--app-id', version: '--version', name: '--name', output: '--output',
    clientType: '--client-type', bundleId: '--bundle-id',
  })) {
    if (options[field] !== undefined) args.push(`${flag}=${options[field]}`);
  }
  for (const [field, flag] of Object.entries({
    minify: '--minify', parallel: '--parallel', typescript: '--typescript', less: '--less',
    enableModuleGraph: '--enable-module-graph', enableSplitChunk: '--enable-split-chunk',
    subpackageOnly: '--subpackage-only',
  })) {
    if (options[field]) args.push(flag);
  }
  await new Promise((resolve, reject) => {
    const child = spawn(process.execPath, args, { stdio: ['ignore', process.stderr, process.stderr] });
    child.once('error', reject);
    child.once('close', (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`Compilation failed (${signal || code})`));
    });
  });
  return JSON.parse(await fs.readFile(resultFile, 'utf8'));
}
