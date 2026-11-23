import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';

const executable = process.argv[2] && path.resolve(process.argv[2]);
if (!executable) throw new Error('Usage: node scripts/verify-openapi.mjs <rust-openapi-executable>');

const root = path.resolve(import.meta.dirname, '..');
const template = JSON.parse(await fs.readFile(path.join(root, 'demo/openapi/config.example.json'), 'utf8'));
assert.equal(template.action, 'getPackages', 'The demo must use the read-only getPackages action');
const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-openapi-smoke-'));
const { privateKey, publicKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
const config = {
  ...template,
  appId: 'OPENAPI_DEMO_TEST',
  h5Id: '0000000000000012',
  privateKeyFile: './private-key.pem',
  timeoutSeconds: 5,
};
let scenario;
let requestCount = 0;
let serverError;

const server = http.createServer(async (request, response) => {
  requestCount++;
  try {
    assert.equal(request.method, 'GET', 'The demo must only send a GET request');
    const url = new URL(request.url, 'http://localhost');
    assert.equal(url.pathname, '/openapi/miniprogram/getPackages');
    const query = url.searchParams;
    for (const field of ['appId', 'workspaceId', 'tenantId', 'h5Id', 'packageTypes', 'pageNum', 'pageSize']) {
      assert.equal(query.get(field), String(config[field]), `Unexpected ${field}`);
    }
    assert.match(query.get('timestamp') ?? '', /^\d{10}$/);
    assert.ok(Math.abs(Date.now() / 1000 - Number(query.get('timestamp'))) < 30, 'Timestamp must use seconds');
    const signature = query.get('sign') ?? '';
    assert.match(signature, /^[0-9a-f]{512}$/);
    const content = `appId=${query.get('appId')}&workspaceId=${query.get('workspaceId')}&timestamp=${query.get('timestamp')}&url=${url.pathname}`;
    assert.equal(crypto.verify('RSA-SHA256', Buffer.from(content), publicKey, Buffer.from(signature, 'hex')), true, 'Java-compatible path signature');
    let bodyLength = 0;
    for await (const chunk of request) bodyLength += chunk.length;
    assert.equal(bodyLength, 0, 'The query must not upload any request body');
    response.setHeader('Content-Type', scenario.contentType ?? 'application/json');
    response.end(typeof scenario.body === 'string' ? scenario.body : JSON.stringify(scenario.body));
  } catch (error) {
    serverError ??= error;
    response.statusCode = 500;
    response.end('Local mock assertion failed');
  }
});

function run(configPath) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, [configPath], {
      env: { ...process.env, RUST_LOG: 'error', NO_PROXY: '127.0.0.1,localhost', no_proxy: '127.0.0.1,localhost' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    const timer = setTimeout(() => {
      child.kill();
      reject(new Error('OpenAPI demo timed out'));
    }, 15000);
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.once('error', (error) => { clearTimeout(timer); reject(error); });
    child.once('close', (code, signal) => {
      clearTimeout(timer);
      try {
        assert.equal(signal, null, 'OpenAPI client terminated by a signal');
        resolve({ code, result: JSON.parse(stdout), stderr });
      } catch (error) {
        reject(new Error(`Invalid OpenAPI result (${code}): ${stderr}\n${stdout}`, { cause: error }));
      }
    });
  });
}

try {
  await fs.writeFile(path.join(temp, 'private-key.pem'), privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  config.baseUrl = `http://127.0.0.1:${server.address().port}`;
  const configPath = path.join(temp, 'config.json');
  await fs.writeFile(configPath, JSON.stringify(config));

  const scenarios = [
    { name: 'empty resource list', code: 0, body: { data: { success: true, data: [] } } },
    { name: 'business rejection', code: 1, kind: 'api', body: { data: { success: false, errorCode: 'DEMO_REJECTED', resultMsg: 'Demo request rejected' } } },
    { name: 'HTTP 200 login HTML', code: 1, kind: 'response', contentType: 'text/html', body: '<html><body>Login required</body></html>' },
  ];
  for (const current of scenarios) {
    scenario = current;
    const before = requestCount;
    const { code, result, stderr } = await run(configPath);
    if (serverError) throw serverError;
    assert.equal(requestCount - before, 1, `${current.name}: expected exactly one request`);
    assert.equal(code, current.code, `${current.name}: ${stderr}`);
    assert.equal(result.action, 'getPackages');
    assert.equal(result.ok, current.code === 0);
    if (current.code === 0) assert.deepEqual(result.data, []);
    else assert.equal(result.error.kind, current.kind);
  }
  console.log('OpenAPI-only demo verified: signed read-only query, empty success, business rejection and HTTP 200 HTML. No compilation, upload or publication.');
} finally {
  server.closeAllConnections();
  if (server.listening) await new Promise((resolve) => server.close(resolve));
  await fs.rm(temp, { recursive: true, force: true });
}
