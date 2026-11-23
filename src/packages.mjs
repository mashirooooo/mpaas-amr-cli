import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import AdmZip from 'adm-zip';
import * as tar from 'tar';

export function validateAppId(value) {
  if (!/^\d{16}$/.test(value ?? '')) throw new Error('Small-program app ID must contain exactly 16 digits');
  return value;
}

export function validateVersion(value) {
  if (!/^\d+(?:\.\d+){2,3}$/.test(value ?? '')) throw new Error(`Invalid version: ${value}`);
  return value;
}

function safeArchivePath(value) {
  const normalized = value.replaceAll('\\', '/');
  if (!normalized || path.posix.isAbsolute(normalized) || path.win32.isAbsolute(value)
    || normalized.split('/').some((part) => part === '..' || part.includes(':'))) {
    throw new Error(`Unsafe archive path: ${value}`);
  }
  return normalized.replace(/^\.\//, '');
}

function xmlText(value) {
  return String(value).replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[char]);
}

function readManifest(xml) {
  const read = (name) => {
    const matches = [...xml.matchAll(new RegExp(`<${name}>([^<]*)</${name}>`, 'g'))];
    if (matches.length !== 1) throw new Error(`Manifest.xml must contain one ${name}`);
    const value = matches[0][1];
    if (/&(?!amp;|lt;|gt;|quot;|apos;)/.test(value)) throw new Error('Manifest.xml contains an invalid XML entity');
    return value.replace(/&(amp|lt|gt|quot|apos);/g, (_, entity) => ({ amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" })[entity]);
  };
  return { appId: validateAppId(read('uid')), version: validateVersion(read('version')), name: read('name') };
}

function archiveEntries(zip) {
  const entries = zip.getEntries();
  const names = entries.map((entry) => safeArchivePath(entry.entryName));
  if (new Set(names).size !== names.length) throw new Error('Archive contains duplicate paths');
  if (entries.some((entry) => ((entry.header.fileAttr >>> 16) & 0o170000) === 0o120000)) {
    throw new Error('Archive contains a symbolic link');
  }
  return entries;
}

function inspectUploadZip(zip, appId, mainUrl) {
  validateAppId(appId);
  const entries = archiveEntries(zip);
  if (entries.some((entry) => !entry.entryName.startsWith(`${appId}/`))) {
    throw new Error(`Every ZIP entry must be under ${appId}/`);
  }
  const files = entries.filter((entry) => !entry.isDirectory);
  if (!files.length) throw new Error('Upload ZIP contains no files');
  if (mainUrl) {
    const entry = mainUrl.split(/[?#]/)[0].replace(/^\//, '');
    const target = entry && zip.getEntry(`${appId}/${safeArchivePath(entry)}`);
    if (!target || target.isDirectory) {
      throw new Error(`mainUrl does not resolve to a file in the ZIP: ${mainUrl}`);
    }
  }
  return { format: 'zip', appId, fileCount: files.length, structureValid: true };
}

async function inspectUploadDirectory(directory, appId, mainUrl) {
  validateAppId(appId);
  let fileCount = 0;
  async function visit(current) {
    const entries = await fs.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const full = path.join(current, entry.name);
      const metadata = await fs.lstat(full);
      if (metadata.isSymbolicLink()) throw new Error('Upload resource directory contains a symbolic link');
      if (metadata.isDirectory()) {
        await visit(full);
      } else if (metadata.isFile()) {
        fileCount += 1;
      } else {
        throw new Error(`Upload resource directory contains a non-file entry: ${entry.name}`);
      }
    }
  }
  await visit(directory);
  if (!fileCount) throw new Error('Upload resource directory contains no files');
  if (mainUrl) {
    const relative = mainUrl.split(/[?#]/)[0].replace(/^\//, '');
    const target = path.resolve(directory, relative);
    if (!target.startsWith(`${path.resolve(directory)}${path.sep}`)) throw new Error(`mainUrl escapes the upload resource directory: ${mainUrl}`);
    const metadata = await fs.lstat(target).catch(() => null);
    if (!metadata?.isFile()) throw new Error(`mainUrl does not resolve to a file in the resource directory: ${mainUrl}`);
  }
  return { format: 'directory', appId, fileCount, structureValid: true };
}

async function withExtractedTar(tarPath, action) {
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-tar-'));
  try {
    let fileCount = 0;
    let entryError;
    await tar.t({ file: tarPath, strict: true, onentry(entry) {
      try {
        safeArchivePath(entry.path);
        if (!['File', 'OldFile', 'Directory'].includes(entry.type)) throw new Error(`Unsupported TAR entry: ${entry.type}`);
      } catch (error) {
        entryError ??= error;
      }
      if (entry.type !== 'Directory') fileCount += 1;
    } });
    if (entryError) throw entryError;
    if (!fileCount) throw new Error('Inner TAR contains no files');
    await tar.x({ file: tarPath, cwd: temp, strict: true, preservePaths: false });
    return await action(temp, fileCount);
  } finally {
    await fs.rm(temp, { recursive: true, force: true });
  }
}

/** 将主包和编译器生成的分包合并为一个可上传的资源 TAR。 */
export async function mergePackageTars({ mainTar, subPackages, output }) {
  const staging = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-merged-tar-'));
  try {
    const packages = [mainTar, ...(subPackages ?? []).map((item) => item.tarFilePath)];
    if (packages.some((file) => typeof file !== 'string' || !file)) {
      throw new Error('Compiler subpackage result is missing a TAR file');
    }
    for (const file of packages) {
      await withExtractedTar(file, async (temp) => {
        await fs.cp(temp, staging, { recursive: true, force: true });
      });
    }
    await fs.mkdir(path.dirname(output), { recursive: true });
    await tar.c({ cwd: staging, file: output, portable: true }, ['.']);
    return output;
  } finally {
    await fs.rm(staging, { recursive: true, force: true });
  }
}

async function writeZip(zip, output) {
  await fs.mkdir(path.dirname(output), { recursive: true });
  const temporary = `${output}.${crypto.randomUUID()}.tmp`;
  try {
    zip.writeZip(temporary);
    await fs.rename(temporary, output);
  } finally {
    await fs.rm(temporary, { force: true });
  }
  return output;
}

export async function createUploadZip({ tarPath, appId, output, mainUrl }) {
  validateAppId(appId);
  if (path.basename(output) !== `${appId}.zip`) throw new Error(`Upload ZIP filename must be ${appId}.zip`);
  if (path.resolve(tarPath) === path.resolve(output)) throw new Error('Input and output paths must differ');
  return withExtractedTar(tarPath, async (temp) => {
    const children = await fs.readdir(temp, { withFileTypes: true });
    const rooted = children.length === 1 && children[0].isDirectory() && children[0].name === appId;
    const zip = new AdmZip();
    zip.addLocalFolder(rooted ? path.join(temp, appId) : temp, appId);
    inspectUploadZip(zip, appId, mainUrl);
    return writeZip(zip, output);
  });
}

export async function createAmr({ tarPath, appId, name, version, output }) {
  validateAppId(appId);
  validateVersion(version);
  if (path.resolve(tarPath) === path.resolve(output)) throw new Error('Input and output paths must differ');
  const tarName = `${appId}.tar`;
  const zip = new AdmZip();
  zip.addFile(tarName, await fs.readFile(tarPath));
  zip.addFile('Manifest.xml', Buffer.from(`<?xml version="1.0" encoding="utf-8"?>\n<package>\n  <uid>${appId}</uid>\n  <name>${xmlText(name)}</name>\n  <version>${version}</version>\n</package>\n`));
  zip.addFile('CERT.json', Buffer.from(JSON.stringify({ [tarName]: '', 'Manifest.xml': '' })));
  return writeZip(zip, output);
}

export async function validatePackage(file, { appId, mainUrl } = {}) {
  const absolute = path.resolve(file);
  const fileMetadata = await fs.lstat(absolute);
  if (fileMetadata.isDirectory()) return { file: absolute, ...(await inspectUploadDirectory(absolute, appId ?? path.basename(absolute), mainUrl)) };
  if (!fileMetadata.isFile()) throw new Error('Upload resource must be a directory or file');
  const zip = new AdmZip(absolute);
  const entries = archiveEntries(zip);
  if (!zip.getEntry('Manifest.xml')) {
    const uid = validateAppId(appId ?? path.basename(absolute, '.zip'));
    if (path.basename(absolute) !== `${uid}.zip`) throw new Error(`Upload ZIP filename must be ${uid}.zip`);
    return { file: absolute, ...inspectUploadZip(zip, uid, mainUrl) };
  }
  const metadata = readManifest(zip.readAsText('Manifest.xml'));
  if (appId && appId !== metadata.appId) throw new Error('AMR uid does not match app ID');
  const expected = ['Manifest.xml', 'CERT.json', `${metadata.appId}.tar`];
  if (entries.length !== expected.length || expected.some((name) => !zip.getEntry(name))) {
    throw new Error('AMR must contain Manifest.xml, CERT.json and <uid>.tar');
  }
  const cert = JSON.parse(zip.readAsText('CERT.json'));
  if (!cert || typeof cert !== 'object' || Array.isArray(cert)
    || ['Manifest.xml', `${metadata.appId}.tar`].some((name) => typeof cert[name] !== 'string')) {
    throw new Error('CERT.json must map Manifest.xml and <uid>.tar to signature strings');
  }
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-amr-check-'));
  try {
    const tarPath = path.join(temp, `${metadata.appId}.tar`);
    await fs.writeFile(tarPath, zip.readFile(`${metadata.appId}.tar`));
    const fileCount = await withExtractedTar(tarPath, (_, count) => count);
    return { file: absolute, format: 'amr', ...metadata, fileCount, structureValid: true, signatureVerified: false };
  } finally {
    await fs.rm(temp, { recursive: true, force: true });
  }
}
