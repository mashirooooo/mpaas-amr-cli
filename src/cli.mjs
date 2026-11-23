#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'commander';
import { buildProject } from './build.mjs';
import { validatePackage } from './packages.mjs';
import { releaseProject } from './release.mjs';
import { uploadPackage, publishPackage } from './deploy.mjs';
import { prepareRuntime, runOpenApi } from './runtime.mjs';

const program = new Command();
program.name('mpaas-amr').description('Compile, package, upload and publish an mPaaS mini-program');

program.command('build')
  .requiredOption('-p, --project <path>')
  .requiredOption('-a, --app-id <appId>')
  .requiredOption('-v, --version <version>')
  .option('-n, --name <name>', 'Package name')
  .option('-o, --output <path>', 'Artifact root', 'artifacts')
  .option('--format <format>', 'amr, zip or both', 'both')
  .option('--minify', 'Minify output')
  .option('--parallel', 'Enable parallel compilation')
  .option('--typescript', 'Enable TypeScript compilation')
  .option('--less', 'Enable Less compilation')
  .option('--client-type <clientType>', 'Compiler client type', 'mPaaS')
  .option('--bundle-id <bundleId>', 'Compiler bundle ID')
  .option('--enable-module-graph', 'Emit the compiler module graph')
  .option('--enable-split-chunk', 'Enable split chunk compilation')
  .option('--subpackage-only', 'Build only subpackages')
  .addOption(new Option('--result-file <path>').hideHelp())
  .action(async (options) => {
    const result = await buildProject(options);
    if (options.resultFile) await fs.writeFile(options.resultFile, JSON.stringify(result));
    else console.log(JSON.stringify(result, null, 2));
  });

program.command('validate <package>')
  .option('--app-id <appId>', 'Expected small-program ID')
  .option('--main-url <url>', 'Check that the entry file exists')
  .action(async (file, options) => console.log(JSON.stringify(await validatePackage(file, options), null, 2)));

program.command('openapi [config]')
  .description('Run the embedded Rust RSA OpenAPI client with a JSON config')
  .option('--client-help', 'Show the Rust client help')
  .action(async (config, options) => {
    if (!config && !options.clientHelp) throw new Error('openapi requires a JSON config file');
    process.exitCode = await runOpenApi(config, { clientHelp: options.clientHelp });
  });

program.command('release <config>')
  .description('Resolve the next backend version, compile, upload and publish')
  .option('--prepare-only', 'Query the next version, compile and validate without uploading or publishing')
  .action(async (config, options) => { process.exitCode = await releaseProject(config, options); });

program.command('upload <config>')
  .description('Compile a source directory when needed, then upload a mini-program resource directory or ZIP')
  .option('--publish', 'Create and start a publish task after uploading')
  .action(async (config, options) => { process.exitCode = await uploadPackage(config, options); });

program.command('publish <config>')
  .description('Publish an existing package, or resume the configured task')
  .action(async (config) => { process.exitCode = await publishPackage(config); });

program.command('download-assets')
  .option('--clean-previous')
  .option('--with-compiler')
  .action(async (options) => {
    if (await prepareRuntime()) {
      console.error('[mpaas-amr] Compiler resources are already embedded in this executable.');
      return;
    }
    await new Promise((resolve, reject) => {
    const require = createRequire(import.meta.url);
    const pkg = require('minidev/package.json');
    const bin = typeof pkg.bin === 'string' ? pkg.bin : pkg.bin.minidev;
    const cli = path.resolve(path.dirname(require.resolve('minidev/package.json')), bin);
    const args = [cli, 'download-assets'];
    if (options.cleanPrevious) args.push('--clean-previous');
    if (options.withCompiler) args.push('--with-compiler');
    const child = spawn(process.execPath, args, { stdio: 'inherit' });
    child.once('error', reject);
    child.once('exit', (code) => code === 0 ? resolve() : reject(new Error(`minidev exited with ${code}`)));
    });
  });

program.parseAsync().catch((error) => {
  console.error(`[mpaas-amr] ${error.message}`);
  process.exitCode = 1;
});
