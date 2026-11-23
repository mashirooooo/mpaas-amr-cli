#!/usr/bin/env node

import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import AdmZip from 'adm-zip';
import * as tar from 'tar';

const UPLOAD_PATH = '/openapi/miniprogram/uploadPackage';
const QUERY_PATH = '/openapi/miniprogram/getPackages';

function fail(message) {
  throw new Error(message);
}

function resolveEndpoint(baseUrl, pathname) {
  const base = new URL(baseUrl);
  const prefix = base.pathname.replace(/\/$/, '');
  base.pathname = `${prefix}${pathname}`;
  base.search = '';
  base.hash = '';
  return base;
}

function redactHeader(name, value) {
  return /authorization|cookie|token|secret|key/i.test(name) ? '<redacted>' : value;
}

function requireConfig(value, name) {
  if (value === undefined || value === null || value === '') fail(`JSON field "${name}" is required`);
  return value;
}

function expandEnvironment(value) {
  if (Array.isArray(value)) return value.map(expandEnvironment);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, expandEnvironment(item)]));
  }
  if (typeof value !== 'string') return value;
  return value.replace(/\$\{([A-Z_][A-Z0-9_]*)\}/g, (_, name) => {
    const replacement = process.env[name];
    if (replacement === undefined) fail(`Environment variable ${name} is required by the JSON config`);
    return replacement;
  });
}

function parseHeaders(values = {}) {
  if (!values || Array.isArray(values) || typeof values !== 'object') {
    fail('JSON field "headers" must be an object');
  }
  const headers = new Headers();
  for (const [name, value] of Object.entries(values)) {
    if (!name.trim()) fail('HTTP header name cannot be empty');
    headers.set(name, String(value));
  }
  return headers;
}

async function inspectPackage(file, appId, expectedType = 'zip') {
  const absolute = path.resolve(file);
  const stat = await fs.stat(absolute).catch(() => null);
  if (!stat?.isFile()) fail(`Package file not found: ${absolute}`);

  let zip;
  try {
    zip = new AdmZip(absolute);
  } catch (error) {
    fail(`File is not a readable ZIP container: ${error.message}`);
  }

  const entries = zip.getEntries();
  if (entries.length === 0) fail('Package is empty');
  const unsafe = entries.find((entry) => {
    const normalized = entry.entryName.replaceAll('\\', '/');
    return normalized.startsWith('/') || normalized.split('/').includes('..');
  });
  if (unsafe) fail(`Unsafe ZIP entry: ${unsafe.entryName}`);

  const names = entries.map((entry) => entry.entryName.replaceAll('\\', '/'));
  const isAmr = names.includes('Manifest.xml') && names.includes('CERT.json')
    && names.some((name) => name.endsWith('.tar'));

  if (expectedType === 'amr') {
    if (!isAmr) fail('AMR structure must contain Manifest.xml, CERT.json and an inner .tar');
    return { file: absolute, bytes: stat.size, format: 'amr', entries: names.length };
  }

  if (isAmr) {
    fail('The file has AMR structure. uploadPackage is documented to accept a build ZIP, not an AMR container');
  }
  if (path.basename(absolute) !== `${appId}.zip`) {
    fail(`ZIP filename must be ${appId}.zip`);
  }
  const invalidRoot = names.find((name) => name !== appId && !name.startsWith(`${appId}/`));
  if (invalidRoot) fail(`Every ZIP entry must be under ${appId}/; found ${invalidRoot}`);
  const hasFile = entries.some((entry) => !entry.isDirectory && entry.entryName.startsWith(`${appId}/`));
  if (!hasFile) fail(`ZIP does not contain files under ${appId}/`);

  return { file: absolute, bytes: stat.size, format: 'zip', entries: names.length, root: `${appId}/` };
}

function readAmrMetadata(amrPath) {
  let amr;
  try {
    amr = new AdmZip(amrPath);
  } catch (error) {
    fail(`File is not a readable AMR/ZIP container: ${error.message}`);
  }
  const manifestEntry = amr.getEntry('Manifest.xml');
  const certEntry = amr.getEntry('CERT.json');
  const tarEntries = amr.getEntries().filter((entry) => !entry.isDirectory && entry.entryName.endsWith('.tar'));
  if (!manifestEntry || !certEntry || tarEntries.length !== 1) {
    fail('AMR must contain Manifest.xml, CERT.json and exactly one inner .tar');
  }
  const manifest = amr.readAsText(manifestEntry);
  const uid = manifest.match(/<uid>\s*([^<]+?)\s*<\/uid>/)?.[1];
  const name = manifest.match(/<name>\s*([^<]+?)\s*<\/name>/)?.[1];
  const version = manifest.match(/<version>\s*([^<]+?)\s*<\/version>/)?.[1];
  if (!uid) fail('AMR Manifest.xml does not contain uid');
  return { amr, uid, name, version, tarEntry: tarEntries[0] };
}

async function convertAmr(amrPath, outputPath, expectedAppId) {
  const absoluteAmr = path.resolve(amrPath);
  const stat = await fs.stat(absoluteAmr).catch(() => null);
  if (!stat?.isFile()) fail(`AMR file not found: ${absoluteAmr}`);
  const metadata = readAmrMetadata(absoluteAmr);
  if (!/^\d{16}$/.test(metadata.uid)) fail(`AMR uid must contain exactly 16 digits: ${metadata.uid}`);
  if (expectedAppId && metadata.uid !== expectedAppId) {
    fail(`AMR uid ${metadata.uid} does not match JSON field "h5Id" ${expectedAppId}`);
  }

  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-amr-convert-'));
  try {
    const innerTar = path.join(temp, `${metadata.uid}.tar`);
    const extracted = path.join(temp, 'extracted');
    await fs.mkdir(extracted);
    await fs.writeFile(innerTar, metadata.amr.readFile(metadata.tarEntry));

    let fileCount = 0;
    await tar.t({
      file: innerTar,
      strict: true,
      onentry: (entry) => {
        const normalized = entry.path.replaceAll('\\', '/');
        if (normalized.startsWith('/') || normalized.split('/').includes('..')) {
          fail(`Unsafe inner tar entry: ${entry.path}`);
        }
        if (!['File', 'OldFile', 'Directory'].includes(entry.type)) {
          fail(`Unsupported inner tar entry type ${entry.type}: ${entry.path}`);
        }
        if (entry.type !== 'Directory') fileCount += 1;
      },
    });
    if (fileCount === 0) fail('AMR inner tar does not contain files');
    await tar.x({ file: innerTar, cwd: extracted, strict: true, preservePaths: false });

    const topLevel = await fs.readdir(extracted, { withFileTypes: true });
    const alreadyRooted = topLevel.length === 1 && topLevel[0].isDirectory() && topLevel[0].name === metadata.uid;
    const contentRoot = alreadyRooted ? path.join(extracted, metadata.uid) : extracted;
    const output = path.resolve(outputPath || `${metadata.uid}.zip`);
    await fs.mkdir(path.dirname(output), { recursive: true });
    const uploadZip = new AdmZip();
    uploadZip.addLocalFolder(contentRoot, metadata.uid);
    uploadZip.writeZip(output);
    const details = await inspectPackage(output, metadata.uid, 'zip');
    return { ...details, source: absoluteAmr, uid: metadata.uid, version: metadata.version, converted: true };
  } finally {
    await fs.rm(temp, { recursive: true, force: true });
  }
}

async function prepareUploadPackage(resource, appId) {
  const absolute = path.resolve(resource);
  let entries;
  try {
    entries = new AdmZip(absolute).getEntries().map((entry) => entry.entryName);
  } catch (error) {
    fail(`Package is not a readable ZIP or AMR container: ${error.message}`);
  }
  const isAmr = entries.includes('Manifest.xml') && entries.includes('CERT.json')
    && entries.some((name) => name.endsWith('.tar'));
  if (!isAmr) return { details: await inspectPackage(absolute, appId), cleanup: null };

  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-amr-upload-'));
  const converted = path.join(temp, `${appId}.zip`);
  try {
    return { details: await convertAmr(absolute, converted, appId), cleanup: temp };
  } catch (error) {
    await fs.rm(temp, { recursive: true, force: true });
    throw error;
  }
}

function applyConnectionOptions(options) {
  if (options.insecure) {
    process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';
    console.error('[warning] TLS certificate verification is disabled');
  }
  const timeout = Number(options.timeout ?? 30000);
  if (!Number.isInteger(timeout) || timeout <= 0) fail('JSON field "timeout" must be a positive integer');
  return { headers: parseHeaders(options.headers), signal: AbortSignal.timeout(timeout) };
}

async function readResponse(response) {
  const text = await response.text();
  let body = text;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {
    // Some private-cloud versions return a plain resource ID or success string.
  }
  if (!response.ok) {
    const error = new Error(`HTTP ${response.status} ${response.statusText}`);
    error.response = body;
    throw error;
  }
  return body;
}

async function hydrateUploadOptions(options) {
  requireConfig(options.resource, 'resource');
  const absolute = path.resolve(options.resource);
  const stat = await fs.stat(absolute).catch(() => null);
  if (!stat?.isFile()) fail(`Package file not found: ${absolute}`);

  let archive;
  try {
    archive = new AdmZip(absolute);
  } catch (error) {
    fail(`Package is not a readable ZIP or AMR container: ${error.message}`);
  }
  const entries = archive.getEntries().map((entry) => entry.entryName.replaceAll('\\', '/'));
  const isAmr = entries.includes('Manifest.xml') && entries.includes('CERT.json')
    && entries.some((name) => name.endsWith('.tar'));

  if (isAmr) {
    const metadata = readAmrMetadata(absolute);
    options.h5Id ??= metadata.uid;
    options.h5Version ??= metadata.version;
    options.h5Name ??= metadata.name || metadata.uid;
  } else if (!options.h5Id) {
    const roots = [...new Set(entries.filter(Boolean).map((name) => name.split('/')[0]))];
    if (roots.length === 1 && /^\d{16}$/.test(roots[0])) options.h5Id = roots[0];
  }

  if (!options.h5Name && options.h5Id) options.h5Name = options.h5Id;
  if (!options.vhost && options.h5Id && options.virtualDomain) {
    options.vhost = `${options.h5Id}.${String(options.virtualDomain).replace(/^\.+/, '')}`;
  }
  return options;
}

async function postPackage(options, expectedType) {
  await hydrateUploadOptions(options);
  requireConfig(options.baseUrl, 'baseUrl');
  requireConfig(options.resource, 'resource');
  requireConfig(options.icon, 'icon');
  requireConfig(options.h5Id, 'h5Id');
  requireConfig(options.h5Name, 'h5Name');
  requireConfig(options.h5Version, 'h5Version');
  requireConfig(options.vhost, 'vhost');
  requireConfig(options.clientVersionMin, 'clientVersionMin');
  if (!/^\d{16}$/.test(options.h5Id)) fail('JSON field "h5Id" must contain exactly 16 digits');
  if (!/^\d+(?:\.\d+){2,3}$/.test(options.h5Version)) fail('JSON field "h5Version" must contain 3 or 4 numeric segments');
  options.platform ??= 'all';
  options.autoInstall ??= 0;
  options.installType ??= 0;
  options.enableTabBar ??= 0;
  options.enableOptionMenu ??= 1;
  options.enableKeepAlive ??= 0;
  options.packageType ??= 1;
  options.needDiff ??= false;
  options.extendInfo ??= {};
  options.mainUrl ??= '/index.html#pages/index/index';
  if (!['all', 'Android', 'iOS'].includes(options.platform)) fail('JSON field "platform" must be all, Android or iOS');
  for (const field of ['autoInstall', 'installType', 'enableTabBar', 'enableOptionMenu', 'enableKeepAlive']) {
    if (![0, 1, '0', '1'].includes(options[field])) fail(`JSON field "${field}" must be 0 or 1`);
  }
  const extendInfo = typeof options.extendInfo === 'string' ? options.extendInfo : JSON.stringify(options.extendInfo);
  try { JSON.parse(extendInfo); } catch { fail('JSON field "extendInfo" must contain valid JSON'); }
  const icon = path.resolve(options.icon);
  const iconStat = await fs.stat(icon).catch(() => null);
  if (!iconStat?.isFile()) fail(`Icon file not found: ${icon}`);
  if (!/\.(?:png|jpe?g)$/i.test(icon)) fail('JSON field "icon" must point to a PNG or JPG file');

  const packageType = Number(options.packageType);
  if (![1, 2, 3, 4].includes(packageType)) fail('JSON field "packageType" must be 1, 2, 3 or 4');
  if ((packageType === 3 || packageType === 4) && !options.userId) {
    fail('JSON field "userId" is required for package type 3 or 4');
  }

  if (options.validateBeforeUpload === true) {
    const validation = await requestPackages(options, options.h5Id, String(packageType));
    console.error(JSON.stringify({
      sessionValidation: {
        ok: true,
        status: validation.status,
        h5Id: options.h5Id,
        method: 'GET',
        path: QUERY_PATH,
      },
    }, null, 2));
  }

  const endpoint = resolveEndpoint(options.baseUrl, UPLOAD_PATH);
  const connection = applyConnectionOptions(options);
  const prepared = expectedType === 'amr'
    ? { details: await inspectPackage(options.resource, options.h5Id, 'amr'), cleanup: null }
    : await prepareUploadPackage(options.resource, options.h5Id);
  const details = prepared.details;

  const form = new FormData();
  const fields = {
    h5Id: options.h5Id,
    h5Name: options.h5Name,
    h5Version: options.h5Version,
    mainUrl: options.mainUrl,
    vhost: options.vhost,
    extendInfo,
    autoInstall: options.autoInstall,
    resourceType: '4',
    installType: options.installType,
    platform: options.platform,
    clientVersionMin: options.clientVersionMin,
    clientVersionMax: options.clientVersionMax,
    enableTabBar: options.enableTabBar,
    enableOptionMenu: options.enableOptionMenu,
    enableKeepAlive: options.enableKeepAlive,
    packageType: String(packageType),
    needDiffFlag: String(options.needDiff),
    userId: options.userId,
    uuid: options.uuid,
  };
  for (const [name, value] of Object.entries(fields)) {
    if (value !== undefined && value !== '') form.append(name, String(value));
  }

  const resourceMime = expectedType === 'amr' ? 'application/octet-stream' : 'application/zip';
  form.append('resourceFile', new Blob([await fs.readFile(details.file)], { type: resourceMime }), path.basename(details.file));
  const iconMime = /\.png$/i.test(icon) ? 'image/png' : 'image/jpeg';
  form.append('iconFile', new Blob([await fs.readFile(icon)], { type: iconMime }), path.basename(icon));

  console.error(JSON.stringify({
    request: {
      method: 'POST',
      url: endpoint.href,
      package: details,
      resolved: {
        h5Id: options.h5Id,
        h5Name: options.h5Name,
        h5Version: options.h5Version,
        vhost: options.vhost,
      },
      packageType,
    },
    headers: Object.fromEntries([...connection.headers].map(([name, value]) => [name, redactHeader(name, value)])),
  }, null, 2));
  try {
    const response = await fetch(endpoint, { method: 'POST', headers: connection.headers, body: form, signal: connection.signal });
    const body = await readResponse(response);
    console.log(JSON.stringify({ ok: true, status: response.status, convertedFromAmr: Boolean(details.converted), body }, null, 2));
  } finally {
    if (prepared.cleanup) await fs.rm(prepared.cleanup, { recursive: true, force: true });
  }
}

async function queryPackages(options) {
  requireConfig(options.baseUrl, 'baseUrl');
  requireConfig(options.h5Id, 'h5Id');
  const result = await requestPackages(options, options.h5Id, options.packageTypes ?? '1');
  console.log(JSON.stringify({ ok: true, status: result.status, body: result.body }, null, 2));
}

async function requestPackages(options, h5Id, packageTypes) {
  const endpoint = resolveEndpoint(options.baseUrl, QUERY_PATH);
  endpoint.searchParams.set('h5Id', h5Id);
  endpoint.searchParams.set('packageTypes', packageTypes);
  const connection = applyConnectionOptions(options);
  const response = await fetch(endpoint, { headers: connection.headers, signal: connection.signal });
  const body = await readResponse(response);
  return { status: response.status, body };
}

function resolveConfigPath(configDir, value) {
  return value && !path.isAbsolute(value) ? path.resolve(configDir, value) : value;
}

async function loadConfig(configPath) {
  const absolute = path.resolve(configPath);
  let config;
  try {
    config = JSON.parse(await fs.readFile(absolute, 'utf8'));
  } catch (error) {
    fail(`Cannot read JSON config ${absolute}: ${error.message}`);
  }
  config = expandEnvironment(config);
  const configDir = path.dirname(absolute);
  for (const field of ['resource', 'icon', 'output']) config[field] = resolveConfigPath(configDir, config[field]);
  return config;
}

async function run(config) {
  const action = config.action ?? 'upload';
  switch (action) {
    case 'inspect':
      console.log(JSON.stringify({ ok: true, package: await inspectPackage(requireConfig(config.resource, 'resource'), requireConfig(config.h5Id, 'h5Id')) }, null, 2));
      return;
    case 'convertAmr':
      console.log(JSON.stringify({ ok: true, package: await convertAmr(requireConfig(config.resource, 'resource'), config.output, config.h5Id) }, null, 2));
      return;
    case 'upload':
      await postPackage(config, 'zip');
      return;
    case 'query':
      await queryPackages(config);
      return;
    case 'probeAmr':
      if (config.confirmUnsupportedAmrUpload !== true) fail('probeAmr requires "confirmUnsupportedAmrUpload": true');
      await postPackage(config, 'amr');
      return;
    default:
      fail(`Unsupported JSON action: ${action}`);
  }
}

async function main() {
  const configPath = process.argv[2];
  if (!configPath || process.argv.length !== 3) {
    console.error('Usage: node scripts/verify-mpaas-openapi.mjs <config.json>');
    process.exitCode = 2;
    return;
  }
  await run(await loadConfig(configPath));
}

main().catch((error) => {
  console.error(`[verify-mpaas-openapi] ${error.message}`);
  if (error.response !== undefined) console.error(JSON.stringify(error.response, null, 2));
  process.exitCode = 1;
});
