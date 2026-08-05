#!/usr/bin/env node

import fs from 'node:fs/promises';
import fsSync from 'node:fs';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import AdmZip from 'adm-zip';
import { Command } from 'commander';
import * as tar from 'tar';

const embeddedRuntimeRoot = path.join(os.tmpdir(), 'mpaas-amr-runtime');

async function prepareEmbeddedRuntime() {
  const sea = await import('node:sea');
  if (!sea.isSea()) return null;

  const { getAsset } = sea;
  const archive = getAsset('mpaas-amr-runtime.tar');
  const marker = path.join(embeddedRuntimeRoot, '.ready');
  const archiveHash = crypto.createHash('sha256').update(Buffer.from(archive)).digest('hex');
  const ready = fsSync.existsSync(marker) && (await fs.readFile(marker, 'utf8').catch(() => '')) === archiveHash;
  if (!ready) {
    await fs.rm(embeddedRuntimeRoot, { recursive: true, force: true });
    await fs.mkdir(embeddedRuntimeRoot, { recursive: true });
    const archivePath = path.join(embeddedRuntimeRoot, 'runtime.tar');
    await fs.writeFile(archivePath, Buffer.from(archive));
    await tar.x({ file: archivePath, cwd: embeddedRuntimeRoot });
    await fs.rm(archivePath, { force: true });
    await fs.writeFile(marker, archiveHash);
  }
  process.env.MINIDEV_COMPILEDIR = path.join(embeddedRuntimeRoot, '.minidev');
  return path.join(embeddedRuntimeRoot, 'node_modules', 'minidev');
}

async function loadMinidev() {
  const runtimePackage = await prepareEmbeddedRuntime();
  if (runtimePackage) {
    const runtimeRequire = createRequire(path.join(runtimePackage, 'package.json'));
    return runtimeRequire('minidev');
  }
  return import('minidev');
}

const program = new Command();
program.name('mpaas-amr').description('Build and package an mPaaS AMR file');

function requireValue(value, name) {
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function safeVersion(value) {
  if (!/^[0-9]+(?:\.[0-9]+){2,3}$/.test(value)) {
    throw new Error(`Invalid version: ${value}`);
  }
  return value;
}

function manifestXml(appId, name, version) {
  return `<?xml version="1.0" encoding="utf-8"?>\n<package>\n  <uid>${appId}</uid>\n  <name>${name}</name>\n  <version>${version}</version>\n</package>\n`;
}

async function wrapAmr({ tarPath, appId, name, version, outputDir }) {
  requireValue(tarPath, 'tarPath');
  requireValue(appId, 'appId');
  requireValue(name, 'name');
  safeVersion(requireValue(version, 'version'));

  const tarName = `${appId}.tar`;
  const output = path.resolve(outputDir, `${appId}.amr`);
  const zip = new AdmZip();
  zip.addFile(tarName, await fs.readFile(tarPath));
  zip.addFile('Manifest.xml', Buffer.from(manifestXml(appId, name, version), 'utf8'));
  zip.addFile('CERT.json', Buffer.from(JSON.stringify({ [tarName]: '', 'Manifest.xml': '' }), 'utf8'));
  await fs.mkdir(outputDir, { recursive: true });
  zip.writeZip(output);
  return output;
}

async function build(options) {
  const minidevPackage = await loadMinidev();
  const minidev = minidevPackage.minidev || minidevPackage.default?.minidev;
  const EBuildTarget = minidevPackage.EBuildTarget || minidevPackage.default?.EBuildTarget;
  const appId = requireValue(options.appId, '--app-id');
  const version = safeVersion(requireValue(options.version, '--version'));
  const project = path.resolve(requireValue(options.project, '--project'));
  const outputRoot = path.resolve(options.output || 'artifacts');
  const buildDir = path.join(outputRoot, version, 'build');

  await fs.rm(buildDir, { recursive: true, force: true });
  await fs.mkdir(buildDir, { recursive: true });

  const result = await minidev.build({
    project,
    appId,
    output: buildDir,
    buildTarget: EBuildTarget.Publish,
    minify: options.minify,
    parallel: options.parallel,
    enableTypescript: options.typescript,
    enableLess: options.less,
  });

  const tarPath = result?.result?.tarFilePath;
  if (!tarPath || !fsSync.existsSync(tarPath)) {
    throw new Error(`minidev did not produce tarFilePath: ${tarPath || '<empty>'}`);
  }

  const amr = await wrapAmr({
    tarPath,
    appId,
    name: options.name || 'h5app',
    version,
    outputDir: path.join(outputRoot, version),
  });

  console.log(JSON.stringify({ appId, version, tarPath, amr }, null, 2));
}

async function validate(amrPath) {
  const absolute = path.resolve(amrPath);
  const zip = new AdmZip(absolute);
  const names = zip.getEntries().map((entry) => entry.entryName).sort();
  if (names.length !== 3 || !names.includes('Manifest.xml') || !names.includes('CERT.json')) {
    throw new Error(`Unexpected AMR entries: ${names.join(', ')}`);
  }

  const manifest = zip.readAsText(zip.getEntry('Manifest.xml'));
  const match = manifest.match(/<uid>([^<]+)<\/uid>[\s\S]*<version>([^<]+)<\/version>/);
  if (!match) throw new Error('Manifest.xml is missing uid/version');

  const tarEntry = names.find((name) => name.endsWith('.tar'));
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-amr-'));
  const tarPath = path.join(tempDir, tarEntry);
  await fs.writeFile(tarPath, zip.readFile(zip.getEntry(tarEntry)));
  const tarEntries = [];
  await tar.t({ file: tarPath, onentry: (entry) => tarEntries.push(entry.path) });
  if (tarEntries.length === 0) throw new Error('Inner tar is empty');

  console.log(JSON.stringify({
    amr: absolute,
    entries: names,
    uid: match[1],
    version: match[2],
    tarEntry,
    tarFileCount: tarEntries.length,
    valid: true,
  }, null, 2));
}

function downloadAssets(options) {
  const args = ['download-assets'];
  if (options.cleanPrevious) args.push('--clean-previous');
  if (options.withCompiler) args.push('--with-compiler');
  return new Promise((resolve, reject) => {
    const defaultCli = process.platform === 'win32' ? 'minidev.cmd' : 'minidev';
    const child = spawn(process.env.MINIDEV_CLI || defaultCli, args, {
      stdio: 'inherit',
      env: process.env,
      shell: process.platform === 'win32',
    });
    child.on('error', reject);
    child.on('exit', (code) => code === 0 ? resolve() : reject(new Error(`minidev exited with ${code}`)));
  });
}

program.command('build')
  .requiredOption('-p, --project <path>')
  .requiredOption('-a, --app-id <appId>')
  .requiredOption('-v, --version <version>')
  .option('-n, --name <name>', 'Manifest package name', 'h5app')
  .option('-o, --output <path>', 'Artifact root', 'artifacts')
  .option('--minify', 'Minify output')
  .option('--parallel', 'Enable parallel compilation')
  .option('--typescript', 'Enable TypeScript compilation')
  .option('--less', 'Enable Less compilation')
  .action(build);

program.command('validate <amr>')
  .action(validate);

program.command('download-assets')
  .option('--clean-previous')
  .option('--with-compiler')
  .action(downloadAssets);

program.parseAsync().catch((error) => {
  console.error(`[mpaas-amr] ${error.message}`);
  process.exitCode = 1;
});
