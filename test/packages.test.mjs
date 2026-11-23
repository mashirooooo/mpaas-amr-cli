import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import AdmZip from 'adm-zip';
import * as tar from 'tar';
import { createAmr, createUploadZip, validatePackage } from '../src/packages.mjs';

const appId = '1111111111111111';
const mainUrl = '/index.html#pages/home/home';

async function fixture(t, { rooted = false } = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-packages-test-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const source = path.join(directory, 'source');
  const content = rooted ? path.join(source, appId) : source;
  const files = {
    'index.html': Buffer.from('<html>fixture</html>'),
    'appConfig.json': Buffer.from(JSON.stringify({ pages: ['pages/home/home'] })),
    'assets/icon.bin': Buffer.from([0, 255, 1, 128]),
  };
  for (const [name, bytes] of Object.entries(files)) {
    const file = path.join(content, name);
    await fs.mkdir(path.dirname(file), { recursive: true });
    await fs.writeFile(file, bytes);
  }
  const tarPath = path.join(directory, 'compiled.tar');
  await tar.c({ cwd: source, file: tarPath }, rooted ? [appId] : Object.keys(files));
  return { directory, tarPath, files, output: path.join(directory, `${appId}.zip`) };
}

for (const rooted of [false, true]) {
  test(`upload ZIP preserves compiled files with one app-ID root (already rooted: ${rooted})`, async (t) => {
    const input = await fixture(t, { rooted });
    await createUploadZip({ ...input, appId, mainUrl });
    const zip = new AdmZip(input.output);
    const files = zip.getEntries().filter((entry) => !entry.isDirectory);
    assert.deepEqual(files.map((entry) => entry.entryName).sort(), Object.keys(input.files).map((name) => `${appId}/${name}`).sort());
    for (const [name, bytes] of Object.entries(input.files)) assert.deepEqual(zip.readFile(`${appId}/${name}`), bytes);
    const result = await validatePackage(input.output, { appId, mainUrl });
    assert.equal(result.structureValid, true);
    assert.equal(result.format, 'zip');
    assert.equal(result.fileCount, 3);
  });
}

test('invalid mainUrl fails before replacing an existing output or changing the TAR', async (t) => {
  const input = await fixture(t);
  const originalTar = await fs.readFile(input.tarPath);
  const originalOutput = Buffer.from('existing package');
  await fs.writeFile(input.output, originalOutput);
  await assert.rejects(createUploadZip({ ...input, appId, mainUrl: '/missing.html#pages/home/home' }), /mainUrl/);
  assert.deepEqual(await fs.readFile(input.output), originalOutput);
  assert.deepEqual(await fs.readFile(input.tarPath), originalTar);
});

test('mainUrl must identify a file rather than a directory', async (t) => {
  const input = await fixture(t);
  await createUploadZip({ ...input, appId, mainUrl });
  await assert.rejects(validatePackage(input.output, { appId, mainUrl: '/assets/' }), /mainUrl/);
});

test('upload ZIP rejects files outside the app-ID directory', async (t) => {
  const input = await fixture(t);
  const zip = new AdmZip();
  zip.addFile(`${appId}/index.html`, Buffer.from('valid'));
  zip.addFile('other.txt', Buffer.from('wrong root'));
  zip.writeZip(input.output);
  await assert.rejects(validatePackage(input.output, { appId, mainUrl }), /under/);
});

test('upload resource directories are accepted and validated before Rust packages them', async (t) => {
  const input = await fixture(t);
  const result = await validatePackage(path.join(input.directory, 'source'), { appId, mainUrl });
  assert.equal(result.format, 'directory');
  assert.equal(result.fileCount, 3);
  await fs.symlink(path.join(input.directory, 'source', 'index.html'), path.join(input.directory, 'source', 'unsafe-link'));
  await assert.rejects(validatePackage(path.join(input.directory, 'source'), { appId }), /symbolic link/);
});

test('packaging refuses to replace its input file', async (t) => {
  const input = await fixture(t);
  await fs.copyFile(input.tarPath, input.output);
  const original = await fs.readFile(input.output);
  await assert.rejects(createUploadZip({ tarPath: input.output, output: input.output, appId, mainUrl }), /differ/);
  assert.deepEqual(await fs.readFile(input.output), original);
  await assert.rejects(createAmr({ tarPath: input.tarPath, output: input.tarPath, appId, version: '1.0.0.0', name: 'test' }), /differ/);
  assert.deepEqual(await fs.readFile(input.tarPath), original);
});

test('unsafe TAR paths and symbolic links reject before extraction or output replacement', async (t) => {
  const input = await fixture(t);
  const originalOutput = Buffer.from('existing package');
  await fs.writeFile(input.output, originalOutput);
  for (const entry of [
    { path: '../outside.txt', type: 'File', size: 1 },
    { path: '/absolute.txt', type: 'File', size: 1 },
    { path: 'C:\\outside.txt', type: 'File', size: 1 },
    { path: 'link', type: 'SymbolicLink', size: 0, linkpath: '../outside.txt' },
  ]) {
    const header = new tar.Header({ ...entry, mode: 0o644 });
    header.encode();
    await fs.writeFile(input.tarPath, Buffer.concat([header.block, Buffer.alloc(entry.size ? 512 : 0), Buffer.alloc(1024)]));
    await assert.rejects(createUploadZip({ ...input, appId, mainUrl }), /Unsafe archive path|Unsupported TAR entry/);
    assert.deepEqual(await fs.readFile(input.output), originalOutput);
  }
  await assert.rejects(fs.access(path.join(input.directory, 'outside.txt')), { code: 'ENOENT' });
});

test('AMR escapes XML text and reports structure validity separately from signature verification', async (t) => {
  const input = await fixture(t);
  const output = path.join(input.directory, `${appId}.amr`);
  const name = '供电 A&B <测试> "应用" \'名\'';
  await createAmr({ ...input, output, appId, version: '1.2.3.4', name });
  const xml = new AdmZip(output).readAsText('Manifest.xml');
  assert.match(xml, /A&amp;B &lt;测试&gt; &quot;应用&quot; &apos;名&apos;/);
  const result = await validatePackage(output, { appId });
  assert.equal(result.name, name);
  assert.equal(result.version, '1.2.3.4');
  assert.equal(result.structureValid, true);
  assert.equal(result.signatureVerified, false);
  assert.equal(result.fileCount, 3);
});

test('AMR validation rejects mismatched UID, TAR name and malformed signature maps', async (t) => {
  const input = await fixture(t);
  const output = path.join(input.directory, `${appId}.amr`);
  await createAmr({ ...input, output, appId, version: '1.0.0.0', name: 'test' });
  const original = await fs.readFile(output);
  await assert.rejects(validatePackage(output, { appId: '2222222222222222' }), /uid/);
  for (const [change, expected] of [
    [(zip) => { const bytes = zip.readFile(`${appId}.tar`); zip.deleteFile(`${appId}.tar`); zip.addFile('2222222222222222.tar', bytes); }, /<uid>\.tar/],
    [(zip) => zip.updateFile('CERT.json', Buffer.from('{}')), /CERT\.json/],
    [(zip) => zip.updateFile('CERT.json', Buffer.from('[]')), /CERT\.json/],
    [(zip) => zip.updateFile('Manifest.xml', Buffer.from('<package><uid>invalid</uid><name>test</name><version>1.0.0.0</version></package>')), /16 digits/],
    [(zip) => zip.updateFile(`${appId}.tar`, Buffer.from('not a TAR')), /TAR|tar|archive/i],
  ]) {
    const zip = new AdmZip(original);
    change(zip);
    zip.writeZip(output);
    await assert.rejects(validatePackage(output), expected);
  }
});
