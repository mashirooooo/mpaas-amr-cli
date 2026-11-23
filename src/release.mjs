import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { compileInChild, runOpenApiConfig } from './runtime.mjs';
import { validatePackage } from './packages.mjs';
import { loadConfig } from './config.mjs';

export { loadConfig as loadReleaseConfig } from './config.mjs';

export function prepareUploadConfig(config, build, { action = 'uploadAndPublish' } = {}) {
  const upload = config.upload ?? {};
  const buildSubpackageFlag = build.subpackageFlag === true;
  for (const [field, value] of Object.entries({ h5Id: build.appId, h5Version: build.version, mainUrl: build.mainUrl })) {
    if (upload[field] && upload[field] !== value) throw new Error(`upload.${field} conflicts with compiler output`);
  }
  if (upload.subpackageFlag !== undefined && upload.subpackageFlag !== buildSubpackageFlag) {
    throw new Error('upload.subpackageFlag conflicts with compiler output');
  }
  const extra = typeof upload.extendInfo === 'string' ? JSON.parse(upload.extendInfo) : upload.extendInfo ?? {};
  const prepared = {
    ...config, action,
    upload: {
      ...upload, resource: build.zip, h5Id: build.appId, h5Version: build.version,
      h5Name: upload.h5Name ?? config.build?.name, mainUrl: build.mainUrl,
      extendInfo: { ...extra, ...build.extendInfo, launchParams: { ...extra.launchParams, ...build.extendInfo.launchParams } },
    },
  };
  if (buildSubpackageFlag) prepared.upload.subpackageFlag = true;
  else delete prepared.upload.subpackageFlag;
  return prepared;
}

export async function getNextVersionPlan(config, h5Id, packageTypes = '1') {
  const query = await runOpenApiConfig({
    ...config, action: 'getNextVersion', h5Id, packageTypes: String(packageTypes),
  }, { captureOutput: true });
  if (query.code !== 0) return { query, plan: null };
  const plan = query.result.data;
  if (query.result.ok !== true || typeof plan?.nextVersion !== 'string') {
    throw new Error('OpenAPI client did not return nextVersion');
  }
  return { query, plan };
}

export async function releaseProject(file, { prepareOnly = false } = {}) {
  const config = await loadConfig(file);
  if (!config.build?.project || !config.build?.appId) throw new Error('release requires build.project and build.appId');
  if (!config.upload || !config.publish) throw new Error('release requires upload and publish configuration');
  if ((config.upload.packageType ?? 1) !== 1) throw new Error('release supports functional packages (packageType=1)');
  const { query, plan: versionPlan } = await getNextVersionPlan(config, config.build.appId, '1');
  if (!versionPlan) {
    console.log(JSON.stringify(query.result, null, 2));
    return query.code;
  }
  if (config.build.version && config.build.version !== versionPlan.nextVersion) throw new Error('build.version conflicts with the next backend version; omit build.version to assign it automatically');
  console.error(`[mpaas-amr] Next version: ${versionPlan.nextVersion}`);
  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-release-'));
  try {
    const build = await compileInChild({ ...config.build, version: versionPlan.nextVersion }, path.join(temp, 'build.json'));
    const uploadConfig = prepareUploadConfig(config, build);
    await validatePackage(build.zip, { appId: build.appId, mainUrl: build.mainUrl });
    if (prepareOnly) {
      console.log(JSON.stringify({ ok: true, prepared: true, uploaded: false, published: false, versionPlan, build }, null, 2));
      return 0;
    }
    return await runOpenApiConfig(uploadConfig);
  } finally {
    await fs.rm(temp, { recursive: true, force: true });
  }
}
