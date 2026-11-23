import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { loadReleaseConfig, prepareUploadConfig } from '../src/release.mjs';

const build = {
  appId: '1111111111111111', version: '1.2.3.4', name: 'test mini-program',
  zip: '/compiled/1111111111111111.zip', mainUrl: '/index.html#pages/home/home',
  extendInfo: {
    compilerField: 'keep',
    launchParams: { page: 'pages/home/home', enableJSC: 'YES', enableWK: 'YES', appxRouteFramework: 'YES' },
  },
};

test('prepared upload preserves compiler entry and launch metadata alongside caller extensions', () => {
  const config = {
    appId: 'HOST_APP', upload: { extendInfo: JSON.stringify({ custom: 42, launchParams: { customLaunch: 'keep', enableJSC: 'NO' } }) },
    publish: { memo: 'test release' },
  };
  const before = JSON.stringify(config);
  const result = prepareUploadConfig(config, build);
  assert.equal(result.action, 'uploadAndPublish');
  assert.equal(result.upload.mainUrl, build.mainUrl);
  assert.equal(result.upload.resource, build.zip);
  assert.equal(result.upload.h5Id, build.appId);
  assert.equal(result.upload.h5Version, build.version);
  assert.equal(Object.hasOwn(result.upload, 'subpackageFlag'), false);
  assert.deepEqual(result.upload.extendInfo, {
    custom: 42, compilerField: 'keep',
    launchParams: { customLaunch: 'keep', ...build.extendInfo.launchParams },
  });
  assert.equal(JSON.stringify(config), before);
  assert.deepEqual(result.publish, config.publish);
});

test('prepared upload rejects app, version, entry and whole-package conflicts', () => {
  for (const upload of [
    { h5Id: '2222222222222222' },
    { h5Version: '9.9.9.9' },
    { mainUrl: '/index.html#pages/wrong/wrong' },
    { subpackageFlag: true },
  ]) {
    assert.throws(() => prepareUploadConfig({ upload }, build), /conflicts|whole package/);
  }
});

test('compiled upload leaves the previous name available while keeping the new entry and launch parameters', () => {
  const inherited = prepareUploadConfig({ upload: {} }, build);
  assert.equal(inherited.upload.h5Name, undefined);
  assert.equal(inherited.upload.mainUrl, build.mainUrl);
  assert.deepEqual(inherited.upload.extendInfo.launchParams, build.extendInfo.launchParams);
  assert.equal(prepareUploadConfig({ upload: { h5Name: 'explicit name' } }, build).upload.h5Name, 'explicit name');
  assert.equal(prepareUploadConfig({ build: { name: 'new project name' }, upload: {} }, build).upload.h5Name, 'new project name');
});

test('release configuration resolves paths relative to its file and expands parsed JSON string values', async (t) => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-release-test-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const configDir = path.join(directory, 'config');
  await fs.mkdir(configDir);
  const variable = 'MPAAS_TEST_ESCAPED_VALUE';
  const previous = process.env[variable];
  process.env[variable] = 'C:\\Users\\测试\\"quoted"\\private.pem\nsecond line';
  t.after(() => { if (previous === undefined) delete process.env[variable]; else process.env[variable] = previous; });
  const file = path.join(configDir, 'release.json');
  await fs.writeFile(file, JSON.stringify({
    privateKeyFile: '../keys/private.pem',
    upload: { resource: '../packages/app.zip', icon: './icon.png', h5Name: '${MPAAS_TEST_ESCAPED_VALUE}' },
    build: { project: '../mini-project' },
  }));
  const result = await loadReleaseConfig(file);
  assert.equal(result.privateKeyFile, path.join(directory, 'keys', 'private.pem'));
  assert.equal(result.upload.resource, path.join(directory, 'packages', 'app.zip'));
  assert.equal(result.upload.icon, path.join(configDir, 'icon.png'));
  assert.equal(result.build.project, path.join(directory, 'mini-project'));
  assert.equal(result.build.output, path.join(directory, 'artifacts'));
  assert.equal(result.upload.h5Name, process.env[variable]);
});

test('release configuration rejects an unresolved environment placeholder', async (t) => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-release-env-test-'));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const variable = 'MPAAS_RELEASE_TEST_MISSING_ENV';
  const previous = process.env[variable];
  delete process.env[variable];
  t.after(() => { if (previous !== undefined) process.env[variable] = previous; });
  const file = path.join(directory, 'release.json');
  await fs.writeFile(file, JSON.stringify({ privateKeyFile: '${MPAAS_RELEASE_TEST_MISSING_ENV}' }));
  await assert.rejects(loadReleaseConfig(file), /Environment variable MPAAS_RELEASE_TEST_MISSING_ENV is required/);
});
