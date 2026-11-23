import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import AdmZip from 'adm-zip';
import * as tar from 'tar';

const root = path.resolve(import.meta.dirname, '..');
const cli = path.join(root, 'scripts', 'verify-mpaas-openapi.mjs');

function run(configPath, env = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [cli, configPath], { cwd: root, env: { ...process.env, ...env } });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('error', reject);
    child.on('exit', (code) => resolve({ code, stdout, stderr }));
  });
}

async function writeConfig(directory, name, config) {
  const file = path.join(directory, name);
  await fs.writeFile(file, JSON.stringify(config, null, 2));
  return file;
}

test('inspects, uploads and queries a documented mini-program ZIP', async (t) => {
  const appId = '1111111111111111';
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-openapi-test-'));
  t.after(() => fs.rm(temp, { recursive: true, force: true }));

  const resource = path.join(temp, `${appId}.zip`);
  const zip = new AdmZip();
  zip.addFile(`${appId}/app.json`, Buffer.from('{"pages":["pages/index/index"]}'));
  zip.writeZip(resource);
  const icon = path.join(temp, 'icon.png');
  await fs.writeFile(icon, Buffer.from('not-a-real-png'));

  const requests = [];
  const server = http.createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    requests.push({ url: request.url, headers: request.headers, body: Buffer.concat(chunks).toString('latin1') });
    response.setHeader('content-type', 'application/json');
    response.end(request.method === 'POST' ? '"189"' : '[{"h5Version":"1.0.0.1"}]');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));
  const baseUrl = `http://127.0.0.1:${server.address().port}/gateway`;

  const inspectedConfig = await writeConfig(temp, 'inspect.json', { action: 'inspect', resource, h5Id: appId });
  const inspected = await run(inspectedConfig);
  assert.equal(inspected.code, 0, inspected.stderr);
  assert.equal(JSON.parse(inspected.stdout).package.format, 'zip');

  const uploadConfig = await writeConfig(temp, 'upload.json', {
    action: 'upload', baseUrl, headers: { Authorization: 'Bearer ${TEST_TOKEN}' },
    resource, icon, h5Id: appId, h5Name: 'test-mini-program', h5Version: '1.0.0.1',
    virtualDomain: 'mini.example.test', clientVersionMin: '1.0.0;1.0.0',
  });
  const uploaded = await run(uploadConfig, { TEST_TOKEN: 'test-token' });
  assert.equal(uploaded.code, 0, uploaded.stderr);
  assert.equal(JSON.parse(uploaded.stdout).body, '189');
  assert.equal(requests[0].url, '/gateway/openapi/miniprogram/uploadPackage');
  assert.equal(requests[0].headers.authorization, 'Bearer test-token');
  assert.match(requests[0].body, /name="resourceFile"/);
  assert.match(requests[0].body, /name="resourceType"\r\n\r\n4/);
  assert.doesNotMatch(uploaded.stderr, /test-token/);

  const queryConfig = await writeConfig(temp, 'query.json', {
    action: 'query', baseUrl, headers: { Authorization: 'Bearer ${TEST_TOKEN}' }, h5Id: appId, packageTypes: '1,2',
  });
  const queried = await run(queryConfig, { TEST_TOKEN: 'test-token' });
  assert.equal(queried.code, 0, queried.stderr);
  assert.deepEqual(JSON.parse(queried.stdout).body, [{ h5Version: '1.0.0.1' }]);
  assert.equal(requests[1].url, `/gateway/openapi/miniprogram/getPackages?h5Id=${appId}&packageTypes=1%2C2`);
});

test('converts an AMR inner tar to an app-ID-rooted upload ZIP', async (t) => {
  const appId = '1111111111111111';
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-openapi-amr-test-'));
  t.after(() => fs.rm(temp, { recursive: true, force: true }));
  const inner = path.join(temp, 'inner');
  await fs.mkdir(inner);
  await fs.writeFile(path.join(inner, 'app.json'), '{"pages":[]}');
  const tarPath = path.join(temp, `${appId}.tar`);
  await tar.c({ cwd: inner, file: tarPath }, ['app.json']);

  const amr = path.join(temp, `${appId}.amr`);
  const zip = new AdmZip();
  zip.addFile('Manifest.xml', Buffer.from(`<package><uid>${appId}</uid><version>1.0.0.1</version></package>`));
  zip.addFile('CERT.json', Buffer.from('{}'));
  zip.addFile(`${appId}.tar`, await fs.readFile(tarPath));
  zip.writeZip(amr);

  const output = path.join(temp, `${appId}.zip`);
  const config = await writeConfig(temp, 'convert.json', { action: 'convertAmr', resource: amr, output, h5Id: appId });
  const result = await run(config);
  assert.equal(result.code, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).package.converted, true);
  const converted = new AdmZip(output);
  assert.deepEqual(converted.getEntries().map((entry) => entry.entryName), [`${appId}/app.json`]);
});

test('upload automatically converts AMR to ZIP before sending multipart data', async (t) => {
  const appId = '1111111111111111';
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-openapi-amr-upload-test-'));
  t.after(() => fs.rm(temp, { recursive: true, force: true }));
  const inner = path.join(temp, 'inner');
  await fs.mkdir(inner);
  await fs.writeFile(path.join(inner, 'index.html'), '<html>test mini-program</html>');
  const fixtureTar = path.join(temp, 'fixture.tar');
  await tar.c({ cwd: inner, file: fixtureTar }, ['index.html']);
  const amr = path.join(temp, `${appId}.amr`);
  const rewritten = new AdmZip();
  rewritten.addFile('Manifest.xml', Buffer.from(`<package><uid>${appId}</uid><version>1.0.0.1</version></package>`));
  rewritten.addFile('CERT.json', Buffer.from('{}'));
  rewritten.addFile(`${appId}.tar`, await fs.readFile(fixtureTar));
  rewritten.writeZip(amr);
  const icon = path.join(temp, 'icon.png');
  await fs.writeFile(icon, Buffer.from('png'));

  let requestBody = '';
  const server = http.createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    requestBody = Buffer.concat(chunks).toString('latin1');
    response.end('"200"');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));

  const config = await writeConfig(temp, 'upload-amr.json', {
    baseUrl: `http://127.0.0.1:${server.address().port}`,
    resource: amr,
    icon,
    virtualDomain: 'example.test',
    clientVersionMin: '1.0;1.0',
  });
  const result = await run(config);
  assert.equal(result.code, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).convertedFromAmr, true);
  assert.match(requestBody, new RegExp(`filename="${appId}\\.zip"`));
  assert.doesNotMatch(requestBody, /filename=".*\.amr"/);
});

test('validates configured session headers with a read-only query before upload', async (t) => {
  const appId = '1111111111111111';
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-key-test-'));
  t.after(() => fs.rm(temp, { recursive: true, force: true }));
  const resource = path.join(temp, `${appId}.zip`);
  const zip = new AdmZip();
  zip.addFile(`${appId}/app.json`, Buffer.from('{}'));
  zip.writeZip(resource);
  const icon = path.join(temp, 'icon.png');
  await fs.writeFile(icon, Buffer.from('png'));

  const calls = [];
  const server = http.createServer(async (request, response) => {
    for await (const _ of request) { /* drain body */ }
    calls.push({ method: request.method, url: request.url, headers: request.headers });
    response.setHeader('content-type', 'application/json');
    response.end(request.method === 'GET' ? '[]' : '"189"');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve) => server.close(resolve)));

  const config = await writeConfig(temp, 'session-upload.json', {
    baseUrl: `http://127.0.0.1:${server.address().port}`,
    headers: { Cookie: 'SESSION=session-test', 'X-CSRF-Token': 'csrf-test' },
    validateBeforeUpload: true,
    resource,
    icon,
    h5Version: '1.0.0.1',
    virtualDomain: 'example.test',
    clientVersionMin: '1.0;1.0',
  });
  const result = await run(config);
  assert.equal(result.code, 0, result.stderr);
  assert.deepEqual(calls.map((call) => call.method), ['GET', 'POST']);
  assert.equal(calls[0].headers.cookie, 'SESSION=session-test');
  assert.equal(calls[0].headers['x-csrf-token'], 'csrf-test');
  assert.equal(calls[1].headers.cookie, 'SESSION=session-test');
  assert.doesNotMatch(result.stderr, /session-test|csrf-test/);
});
