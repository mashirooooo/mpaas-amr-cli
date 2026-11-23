import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { loadConfig } from './config.mjs';
import { getNextVersionPlan, prepareUploadConfig } from './release.mjs';
import { validatePackage } from './packages.mjs';
import { compileInChild, runOpenApiConfig } from './runtime.mjs';

async function isSourceProject(directory) {
  for (const marker of ['mini.project.json', 'mini.project.jsonc']) {
    const metadata = await fs.stat(path.join(directory, marker)).catch(() => null);
    if (metadata?.isFile()) return true;
  }
  let sourceFile = false;
  async function visit(current) {
    if (sourceFile) return;
    for (const entry of await fs.readdir(current, { withFileTypes: true })) {
      if (sourceFile) return;
      const full = path.join(current, entry.name);
      const metadata = await fs.lstat(full);
      if (metadata.isSymbolicLink()) continue;
      if (metadata.isDirectory()) {
        await visit(full);
      } else if (metadata.isFile() && /\.(axml|acss|less)$/i.test(entry.name)) {
        sourceFile = true;
      }
    }
  }
  await visit(directory);
  return sourceFile;
}

async function compileUploadResource(config, sourceProject, publish) {
  const h5Id = config.upload.h5Id;
  const { query, plan } = await getNextVersionPlan(config, h5Id, config.upload.packageType ?? 1);
  if (!plan) {
    console.log(JSON.stringify(query.result, null, 2));
    return { code: query.code };
  }
  if (config.upload.h5Version && config.upload.h5Version !== plan.nextVersion) {
    throw new Error(`upload.h5Version ${config.upload.h5Version} differs from the next backend version ${plan.nextVersion}; rebuild or inspect the server before retrying`);
  }
  if (config.build?.version && config.build.version !== plan.nextVersion) {
    throw new Error(`build.version conflicts with the next backend version; omit build.version to assign it automatically`);
  }
  console.error(`[mpaas-amr] Source resource detected; compiling ${h5Id} ${plan.nextVersion}`);
  const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'mpaas-upload-'));
  try {
    const build = await compileInChild({
      ...config.build,
      project: sourceProject,
      appId: config.build?.appId ?? h5Id,
      version: plan.nextVersion,
      output: config.build?.output ?? path.join(temporary, 'artifacts'),
    }, path.join(temporary, 'build.json'));
    const uploadConfig = prepareUploadConfig(config, build, {
      action: publish ? 'uploadAndPublish' : 'upload',
    });
    await validatePackage(build.zip, { appId: build.appId, mainUrl: build.mainUrl });
    console.error(`[mpaas-amr] Compiled resource ready: ${build.zip}`);
    return { config: uploadConfig, temporary };
  } catch (error) {
    await fs.rm(temporary, { recursive: true, force: true });
    throw error;
  }
}

export async function uploadPackage(file, { publish = false } = {}) {
  const config = await loadConfig(file);
  if (!config.upload?.h5Id) throw new Error('upload requires upload.h5Id');
  const resourcePath = config.upload.resource;
  const resource = resourcePath ? await fs.stat(resourcePath).catch(() => null) : null;
  const sourceProject = config.build?.project
    ?? (resource?.isDirectory() && await isSourceProject(resourcePath) ? resourcePath : null);
  if (!resource && !sourceProject) throw new Error(`upload.resource does not exist: ${resourcePath}`);
  if (sourceProject) {
    const compiled = await compileUploadResource(config, sourceProject, publish);
    if (compiled.code !== undefined) return compiled.code;
    try {
      return await runOpenApiConfig(compiled.config);
    } finally {
      await fs.rm(compiled.temporary, { recursive: true, force: true });
    }
  }
  await validatePackage(resourcePath, { appId: config.upload.h5Id, mainUrl: config.upload.mainUrl });
  return runOpenApiConfig({ ...config, action: publish ? 'uploadAndPublish' : 'upload' });
}

export async function publishPackage(file) {
  const config = await loadConfig(file);
  return runOpenApiConfig({ ...config, action: 'publish' });
}
