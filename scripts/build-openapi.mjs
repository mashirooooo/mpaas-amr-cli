import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const sdkRoot = path.resolve(import.meta.dirname, '..', 'rust-sdk');
const targetDirectory = path.join(sdkRoot, 'target');
const executableName = `mpaas-openapi-rs${process.platform === 'win32' ? '.exe' : ''}`;
const executable = path.join(targetDirectory, 'release', executableName);
const environment = { ...process.env };
delete environment.CARGO_BUILD_TARGET;
const cargoArguments = ['build', '--release', '--locked', '--target-dir', targetDirectory];
let compiledExecutable = executable;

if (process.platform === 'win32') {
  const rustc = spawnSync('rustc', ['-vV'], { encoding: 'utf8' });
  const host = rustc.stdout?.match(/^host: (.+)$/m)?.[1].trim();
  if (rustc.error || rustc.status !== 0 || !host?.endsWith('-windows-msvc')) {
    console.error('[build:openapi] Windows builds require a native MSVC Rust toolchain.');
    process.exit(1);
  }
  // An explicit target keeps the static CRT flags away from host proc macros.
  cargoArguments.push('--target', host);
  const targetFlags = `CARGO_TARGET_${host.toUpperCase().replaceAll('-', '_')}_RUSTFLAGS`;
  environment[targetFlags] = `${environment[targetFlags] || ''} -C target-feature=+crt-static`.trim();
  if (environment.RUSTFLAGS) environment.RUSTFLAGS += ' -C target-feature=+crt-static';
  if (environment.CARGO_ENCODED_RUSTFLAGS) {
    environment.CARGO_ENCODED_RUSTFLAGS += '\u001f-C\u001ftarget-feature=+crt-static';
  }
  compiledExecutable = path.join(targetDirectory, host, 'release', executableName);
}

console.log(`[build:openapi] building native client for ${process.platform}-${process.arch}`);
const result = spawnSync('cargo', cargoArguments, {
  cwd: sdkRoot,
  env: environment,
  stdio: 'inherit',
});
if (result.error) {
  console.error(`[build:openapi] Could not run Cargo: ${result.error.message}. Install Rust on the build machine.`);
  process.exit(1);
}
if (result.status !== 0) process.exit(result.status || 1);
if (compiledExecutable !== executable) {
  fs.mkdirSync(path.dirname(executable), { recursive: true });
  fs.copyFileSync(compiledExecutable, executable);
}
if (!fs.existsSync(executable)) {
  console.error(`[build:openapi] Native client was not produced: ${executable}`);
  process.exit(1);
}
console.log(`[build:openapi] ${executable}`);
