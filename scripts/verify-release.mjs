import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import AdmZip from 'adm-zip';

const appId = '0000000000000012';
const root = path.resolve(import.meta.dirname, '..');
const normalVersions = ['0.0.9', '0.0.19', '0.0.2'];
const iconBytes = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aX1cAAAAASUVORK5CYII=', 'base64');
const previousResource = {
  id: 91, h5Id: appId, h5Version: '0.0.19', h5Name: '上一版本小程序',
  mainUrl: '/index.html#pages/previous/home', platform: 'android',
  clientVersionMin: '2.1;2.2', clientVersionMax: '8.1;8.2', autoInstall: 1, installType: 1,
  extendInfo: JSON.stringify({ custom: 'previous-custom', launchParams: { page: 'pages/previous/home', appxRouteFramework: 'YES', legacyFlag: 'KEEP' } }),
  extraData: JSON.stringify({ enableTabBar: 1, enableOptionMenu: 1, enableKeepAlive: 1 }),
  memo: 'must-not-be-forwarded', downloadUrl: 'https://unused.example.test/previous.amr',
};

function multipart(request, bytes) {
  const boundary = request.headers['content-type']?.match(/boundary=(?:"([^"]+)"|([^;]+))/)?.slice(1).find(Boolean);
  assert.ok(boundary, 'multipart boundary missing');
  return new Map(bytes.toString('latin1').split(`--${boundary}`).slice(1, -1).map((part) => {
    const separator = part.indexOf('\r\n\r\n');
    const name = part.slice(0, separator).match(/name="([^"]+)"/)[1];
    return [name, Buffer.from(part.slice(separator + 4, -2), 'latin1')];
  }));
}

export async function verifyWorkflows(executableArgument, { compile = true, client = false } = {}) {
  if (!executableArgument) throw new Error('A standalone executable is required');
  if (compile && client) throw new Error('The Rust client does not compile projects');
  const executable = path.resolve(executableArgument);
  const { privateKey, publicKey } = crypto.generateKeyPairSync('rsa', { modulusLength: 2048 });
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-deploy-smoke-'));
  const requests = [];
  let scenario;
  let taskStatus;
  let versionQueries;
  let serverError;

  const server = http.createServer(async (request, response) => {
    try {
      const url = new URL(request.url, 'http://localhost');
      const query = url.searchParams;
      const content = `appId=${query.get('appId')}&workspaceId=${query.get('workspaceId')}&timestamp=${query.get('timestamp')}&url=${url.pathname}`;
      const signature = query.get('sign') ?? '';
      assert.match(signature, /^[0-9a-f]{512}$/);
      assert.equal(crypto.verify('RSA-SHA256', Buffer.from(content), publicKey, Buffer.from(signature, 'hex')), true, 'Java-compatible path signature');
      for (const [name, value] of [['appId', 'TEST_APP'], ['workspaceId', 'dev'], ['tenantId', 'ANTCLOUD']]) assert.equal(query.get(name), value);
      const chunks = [];
      for await (const chunk of request) chunks.push(chunk);
      const bytes = Buffer.concat(chunks);
      const fields = request.method === 'POST' ? multipart(request, bytes) : new Map();
      if (request.method === 'POST') {
        for (const [name, value] of [['appId', 'TEST_APP'], ['workspaceId', 'dev'], ['tenantId', 'ANTCLOUD']]) assert.equal(fields.get(name)?.toString(), value);
      } else {
        assert.equal(request.method, 'GET');
        assert.equal(bytes.length, 0);
      }
      requests.push(url.pathname.split('/').at(-1));
      let data;
      switch (url.pathname) {
        case '/openapi/miniprogram/getPackages': {
          assert.equal(request.method, 'GET');
          assert.equal(query.get('h5Id'), appId);
          assert.equal(query.get('packageTypes'), '1');
          assert.equal(query.get('pageNum'), '1');
          assert.ok(Number(query.get('pageSize')) >= normalVersions.length);
          versionQueries++;
          const versions = scenario.versionConflict && versionQueries > 1 ? [...normalVersions, '0.1.0'] : scenario.emptyVersions ? [] : normalVersions;
          data = versions.map((h5Version, index) => ({ id: 90 + index, h5Id: appId, h5Version }));
          break;
        }
        case '/openapi/nebula/getNebulaResourceById':
          assert.equal(request.method, 'GET');
          assert.equal(query.get('id'), '91', 'Defaults must come from the latest version, not the first or last list item');
          data = previousResource;
          break;
        case '/openapi/miniprogram/getAppList':
          assert.equal(request.method, 'GET');
          assert.ok([...query.keys()].every((key) => ['appId', 'workspaceId', 'tenantId', 'timestamp', 'sign'].includes(key)), 'getAppList must not invent business parameters');
          data = [
            { h5Id: '0000000000000099', h5Name: '错误小程序', vhost: 'wrong.example.test' },
            { h5Id: appId, h5Name: '应用默认名称', vhost: `${appId}.mini.example.test` },
          ];
          break;
        case '/openapi/miniprogram/uploadPackage': {
          assert.equal(request.method, 'POST');
          const expected = {
            resourceType: '4', h5Id: appId, h5Version: scenario.emptyVersions ? '0.0.1' : '0.1.0',
            mainUrl: scenario.compiled ? '/index.html#pages/start/home' : '/index.html#pages/index/index',
            ...scenario.expectedUpload,
          };
          for (const [name, value] of Object.entries(expected)) assert.equal(fields.get(name)?.toString(), String(value), `Unexpected upload ${name}`);
          for (const name of ['id', 'memo', 'downloadUrl', 'extraData', ...(scenario.omittedUpload ?? [])]) assert.equal(fields.has(name), false, `Unexpected inherited ${name}`);
          const extendInfo = JSON.parse(fields.get('extendInfo').toString());
          assert.equal(extendInfo.launchParams.appxRouteFramework, 'YES');
          if (!scenario.emptyVersions) {
            assert.equal(extendInfo.custom, scenario.explicitOverride ? 'explicit-custom' : 'previous-custom');
            assert.equal(extendInfo.launchParams.legacyFlag, 'KEEP');
          }
          if (scenario.compiled) assert.equal(extendInfo.launchParams.page, 'pages/start/home');
          if (scenario.explicitOverride) assert.equal(extendInfo.launchParams.page, 'pages/override/home');
          const zip = new AdmZip(fields.get('resourceFile'));
          assert.ok(zip.getEntry(`${appId}/index.html`));
          assert.ok(zip.getEntries().every((entry) => entry.entryName.startsWith(`${appId}/`)));
          data = '101';
          break;
        }
        case '/openapi/miniprogram/createNebulaTask':
          assert.equal(request.method, 'POST');
          assert.equal(fields.get('id')?.toString(), '0');
          assert.equal(fields.get('packageId')?.toString(), '101');
          assert.equal(fields.get('publishType')?.toString(), '3');
          assert.equal(fields.has('publishMode'), false);
          if (scenario.rejectTask) {
            response.setHeader('Content-Type', 'application/json');
            response.end(JSON.stringify({ data: { success: false, errorCode: 'TEST_FAILURE', resultMsg: 'task rejected' } }));
            return;
          }
          data = '202';
          break;
        case '/openapi/miniprogram/getTaskDetail':
          assert.equal(request.method, 'GET');
          assert.equal(query.get('taskId'), '202');
          data = { id: 202, packageId: 101, taskStatus };
          break;
        case '/openapi/miniprogram/changeTaskStatus':
          assert.equal(request.method, 'POST');
          assert.equal(fields.get('bizType')?.toString(), 'nebula');
          assert.equal(fields.get('packageId')?.toString(), '101');
          assert.equal(fields.get('taskId')?.toString(), '202');
          assert.equal(fields.get('taskStatus')?.toString(), '1');
          taskStatus = 1;
          data = 'success';
          break;
        default:
          throw new Error(`Unexpected endpoint: ${url.pathname}`);
      }
      response.setHeader('Content-Type', 'application/json');
      response.end(JSON.stringify({ data: { data, success: true, errorCode: null, resultMsg: '' }, requestId: 'local-smoke' }));
    } catch (error) {
      serverError ??= error;
      response.statusCode = 500;
      response.end(JSON.stringify({ data: { success: false, resultMsg: error.message } }));
    }
  });

  async function run(command, config, current, flags = []) {
    scenario = current;
    taskStatus = current.taskStatus ?? 0;
    versionQueries = 0;
    requests.length = 0;
    serverError = undefined;
    const configPath = path.join(temp, 'deploy.json');
    const action = command === 'upload' && flags.includes('--publish') ? 'uploadAndPublish' : command;
    const preparedConfig = client || command === 'getUploadDefaults' ? { ...config, action } : config;
    await fs.writeFile(configPath, JSON.stringify(preparedConfig), { mode: 0o600 });
    const outcome = await new Promise((resolve, reject) => {
      const args = client ? [configPath] : command === 'getUploadDefaults' ? ['openapi', configPath] : [command, configPath, ...flags];
      const child = spawn(executable, args, {
        env: { ...process.env, NO_PROXY: '127.0.0.1,localhost', no_proxy: '127.0.0.1,localhost' },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let stdout = '';
      let stderr = '';
      const timeout = setTimeout(() => { child.kill(); reject(new Error(`Standalone ${command} timed out`)); }, current.compiled ? 120000 : 15000);
      child.stdout.on('data', (chunk) => { stdout += chunk; });
      child.stderr.on('data', (chunk) => { stderr += chunk; });
      child.once('error', (error) => { clearTimeout(timeout); reject(error); });
      child.once('close', (code, signal) => {
        clearTimeout(timeout);
        try {
          assert.equal(signal, null, `${command} terminated by a signal`);
          resolve({ code, result: JSON.parse(stdout), stderr });
        } catch (error) {
          reject(new Error(`Invalid ${command} result (${code}): ${stderr}\n${stdout}`, { cause: error }));
        }
      });
    });
    if (serverError) throw serverError;
    return outcome;
  }

  function checkSuccess(outcome, expectedRequests, { published = false, uploaded = false } = {}) {
    assert.equal(outcome.code, 0, `${outcome.stderr}\n${JSON.stringify(outcome.result)}`);
    assert.equal(outcome.result.ok, true);
    if (uploaded || published) assert.equal(String(outcome.result.packageId), '101');
    if (published) {
      assert.equal(String(outcome.result.taskId), '202');
      assert.equal(outcome.result.taskStatus, 1);
    }
    assert.deepEqual(requests, expectedRequests);
  }

  try {
    await fs.writeFile(path.join(temp, 'key.pem'), privateKey.export({ type: 'pkcs8', format: 'pem' }), { mode: 0o600 });
    await fs.writeFile(path.join(temp, 'icon.png'), iconBytes);
    const zip = new AdmZip();
    zip.addFile(`${appId}/index.html`, Buffer.from('<!doctype html><title>OpenAPI deployment smoke</title>'));
    zip.writeZip(path.join(temp, `${appId}.zip`));
    const resourceDirectory = path.join(temp, 'resource-directory');
    await fs.mkdir(resourceDirectory, { recursive: true });
    await fs.writeFile(path.join(resourceDirectory, 'index.html'), '<!doctype html><title>OpenAPI directory smoke</title>');
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(0, '127.0.0.1', resolve);
    });
    const common = {
      baseUrl: `http://127.0.0.1:${server.address().port}`, appId: 'TEST_APP', workspaceId: 'dev', tenantId: 'ANTCLOUD', privateKeyFile: './key.pem', timeoutSeconds: 5,
    };
    const upload = {
      resource: `./${appId}.zip`, icon: './icon.png', h5Id: appId, h5Name: '自动部署验证',
      mainUrl: '/index.html#pages/index/index', vhost: `${appId}.mini.example.test`, clientVersionMin: '1.0;1.0',
      extendInfo: { launchParams: { appxRouteFramework: 'YES' } },
    };
    const publish = { publishType: 3, memo: '本地集成验证' };
    const fullPublish = ['createNebulaTask', 'getTaskDetail', 'changeTaskStatus', 'getTaskDetail'];

    if (compile) {
      await fs.cp(path.join(root, 'fixtures', 'minimal'), path.join(temp, 'project'), { recursive: true });
      await fs.mkdir(path.join(temp, 'project', 'pages', 'start'));
      for (const extension of ['js', 'axml', 'acss']) {
        await fs.rename(path.join(temp, 'project', 'pages', 'index', `index.${extension}`), path.join(temp, 'project', 'pages', 'start', `home.${extension}`));
      }
      await fs.writeFile(path.join(temp, 'project', 'app.json'), JSON.stringify({ pages: ['pages/start/home'] }));
      const config = {
        ...common,
        build: { project: './project', appId, output: './artifacts' },
        upload: { icon: './icon.png', vhost: upload.vhost, clientVersionMin: upload.clientVersionMin },
        publish,
      };
      const expectedUpload = { h5Name: previousResource.h5Name };
      const success = await run('release', config, { compiled: true, expectedUpload });
      checkSuccess(success, ['getPackages', 'getPackages', 'getNebulaResourceById', 'uploadPackage', ...fullPublish], { uploaded: true, published: true });
      const build = JSON.parse(await fs.readFile(path.join(temp, 'artifacts', '0.1.0', `${appId}.build.json`), 'utf8'));
      assert.equal(build.version, '0.1.0');
      assert.equal(build.mainUrl, '/index.html#pages/start/home');

      const sourceUpload = {
        ...common,
        upload: { resource: './project', h5Id: appId, icon: './icon.png', vhost: upload.vhost, clientVersionMin: upload.clientVersionMin },
      };
      const sourceUploaded = await run('upload', sourceUpload, { compiled: true, expectedUpload });
      checkSuccess(sourceUploaded, ['getPackages', 'getPackages', 'getNebulaResourceById', 'uploadPackage'], { uploaded: true });

      const failure = await run('release', config, { compiled: true, rejectTask: true, expectedUpload });
      assert.notEqual(failure.code, 0);
      assert.equal(failure.result.ok, false);
      assert.equal(String(failure.result.packageId), '101');
      assert.deepEqual(requests, ['getPackages', 'getPackages', 'getNebulaResourceById', 'uploadPackage', 'createNebulaTask']);

      const conflict = await run('release', config, { compiled: true, versionConflict: true });
      assert.notEqual(conflict.code, 0);
      assert.equal(conflict.result.ok, false);
      assert.ok(conflict.result.error);
      assert.equal(conflict.result.packageId ?? null, null);
      assert.deepEqual(requests, ['getPackages', 'getPackages']);
    }

    const uploadOnly = await run('upload', { ...common, upload }, {});
    checkSuccess(uploadOnly, ['getPackages', 'getNebulaResourceById', 'uploadPackage'], { uploaded: true });
    assert.equal(uploadOnly.result.h5Version, '0.1.0');

    const firstUpload = await run('upload', { ...common, upload }, { emptyVersions: true });
    checkSuccess(firstUpload, ['getPackages', 'uploadPackage'], { uploaded: true });
    assert.equal(firstUpload.result.h5Version, '0.0.1');

    const minimalUpload = { resource: resourceDirectory, icon: upload.icon, h5Id: appId };
    const inheritedSettings = {
      h5Name: previousResource.h5Name, vhost: upload.vhost, mainUrl: previousResource.mainUrl,
      clientVersionMin: previousResource.clientVersionMin, clientVersionMax: previousResource.clientVersionMax,
      platform: 'android', autoInstall: 1, installType: 1,
      enableTabBar: 1, enableOptionMenu: 1, enableKeepAlive: 1,
    };
    const inheritedUpload = await run('upload', { ...common, upload: minimalUpload }, { expectedUpload: inheritedSettings });
    checkSuccess(inheritedUpload, ['getPackages', 'getNebulaResourceById', 'getAppList', 'uploadPackage'], { uploaded: true });
    assert.equal(inheritedUpload.result.h5Version, '0.1.0');

    const overrideSettings = {
      h5Name: '显式名称', vhost: `${appId}.override.example.test`, mainUrl: '/index.html#pages/override/home',
      clientVersionMin: previousResource.clientVersionMin, platform: 'android',
      autoInstall: 0, installType: 0, enableTabBar: 0, enableOptionMenu: 0, enableKeepAlive: 0,
    };
    const explicitUpload = {
      ...minimalUpload, h5Name: overrideSettings.h5Name, vhost: overrideSettings.vhost, mainUrl: overrideSettings.mainUrl,
      clientVersionMax: null, autoInstall: 0, installType: 0, enableTabBar: 0, enableOptionMenu: 0, enableKeepAlive: 0,
      extendInfo: { custom: 'explicit-custom', launchParams: { page: 'pages/override/home' } },
    };
    const overridden = await run('upload', { ...common, upload: explicitUpload }, {
      expectedUpload: overrideSettings, omittedUpload: ['clientVersionMax'], explicitOverride: true,
    });
    checkSuccess(overridden, ['getPackages', 'getNebulaResourceById', 'uploadPackage'], { uploaded: true });

    const preview = await run('getUploadDefaults', { ...common, h5Id: appId, packageTypes: '1' }, {});
    checkSuccess(preview, ['getPackages', 'getNebulaResourceById', 'getAppList']);
    assert.equal(preview.result.data.latestVersion, '0.0.19');
    assert.equal(preview.result.data.nextVersion, '0.1.0');
    const defaults = preview.result.data.uploadDefaults;
    for (const [name, value] of Object.entries(inheritedSettings)) assert.equal(defaults[name], value, `Unexpected preview ${name}`);
    assert.equal(defaults.extendInfo.custom, 'previous-custom');
    assert.equal(defaults.extendInfo.launchParams.page, 'pages/previous/home');
    assert.equal(defaults.extendInfo.launchParams.legacyFlag, 'KEEP');
    for (const name of ['id', 'h5Version', 'memo', 'downloadUrl', 'extraData']) assert.equal(Object.hasOwn(defaults, name), false, `Unexpected preview field ${name}`);

    const uploadAndPublish = await run('upload', { ...common, upload, publish }, {}, ['--publish']);
    checkSuccess(uploadAndPublish, ['getPackages', 'getNebulaResourceById', 'uploadPackage', ...fullPublish], { uploaded: true, published: true });
    assert.equal(uploadAndPublish.result.h5Version, '0.1.0');

    const newTask = await run('publish', { ...common, packageId: '101', publish }, {});
    checkSuccess(newTask, fullPublish, { published: true });

    const resumed = await run('publish', { ...common, packageId: '101', taskId: '202' }, {});
    checkSuccess(resumed, ['getTaskDetail', 'changeTaskStatus', 'getTaskDetail'], { published: true });

    const alreadyPublishing = await run('publish', { ...common, packageId: '101', taskId: '202' }, { taskStatus: 1 });
    checkSuccess(alreadyPublishing, ['getTaskDetail'], { published: true });

    console.log(`${compile ? 'Compile with automatic version → ZIP → upload → publish, source-directory upload compilation, partial failure and version conflict; ' : ''}upload-only, initial version, inherited settings, explicit overrides, read-only defaults preview, upload-and-publish and publish recovery verified against a local RSA mock.`);
  } finally {
    server.closeAllConnections();
    if (server.listening) await new Promise((resolve) => server.close(resolve));
    await fs.rm(temp, { recursive: true, force: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (!process.argv[2]) throw new Error('Usage: node scripts/verify-release.mjs <standalone-executable>');
  await verifyWorkflows(process.argv[2]);
}
