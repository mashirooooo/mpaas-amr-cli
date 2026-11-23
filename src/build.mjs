import fs from 'node:fs/promises';
import path from 'node:path';
import { loadMinidev } from './runtime.mjs';
import { createAmr, createUploadZip, mergePackageTars, validateAppId, validateVersion } from './packages.mjs';

export async function buildProject(options) {
  const appId = validateAppId(options.appId);
  const version = validateVersion(options.version);
  const clientType = options.clientType || 'mPaaS';
  if (!options.project) throw new Error('--project is required');
  const format = options.format ?? 'both';
  if (!['amr', 'zip', 'both'].includes(format)) throw new Error('--format must be amr, zip or both');
  const project = await fs.realpath(path.resolve(options.project));
  const requestedOutput = path.resolve(options.output ?? 'artifacts', version);
  await fs.mkdir(requestedOutput, { recursive: true });
  // The compiler compares real paths; macOS /var and /private/var must agree.
  const output = await fs.realpath(requestedOutput);
  const workRoot = path.join(output, '.work');
  await fs.mkdir(workRoot, { recursive: true });
  const work = await fs.mkdtemp(path.join(workRoot, `${appId}-`));
  try {
    const library = await loadMinidev();
    const minidev = library.minidev ?? library.default?.minidev ?? library.default;
    const target = library.EBuildTarget ?? library.default?.EBuildTarget;
    if (!minidev?.build || !target?.Publish) throw new Error('Unsupported minidev module exports');
    library.setSilent?.(true);
    library.setSpinnerEnabled?.(false);
    console.error(`[mpaas-amr] Compiling ${appId} ${version}`);
    const result = await minidev.build({
      project, appId, output: work, cacheDir: path.join(work, '.cache'),
      buildTarget: target.Publish, minify: options.minify, parallel: options.parallel,
      enableTypescript: options.typescript, enableLess: options.less,
      clientType, bundleId: options.bundleId,
      enableModuleGraph: options.enableModuleGraph,
      enableSplitChunk: options.enableSplitChunk,
      enableSubPackagesOnlyBuild: options.subpackageOnly,
    });
    const buildResult = result?.result;
    const compiledTar = buildResult?.tarFilePath;
    if (!compiledTar) throw new Error('minidev did not produce a small-program TAR');
    const metadata = JSON.parse(await fs.readFile(path.join(work, 'mini.built.json'), 'utf8'));
    const packageInfo = metadata.packageInfo;
    if (!packageInfo?.mainUrl) throw new Error('Compiler metadata is missing packageInfo.mainUrl');
    const expectedClientTypes = clientType === 'alipay'
      ? ['alipay', 'com.alipay.alipaywallet']
      : [clientType];
    if (packageInfo.clientType && !expectedClientTypes.includes(packageInfo.clientType)) {
      throw new Error(`Compiler clientType mismatch: expected ${clientType}, received ${packageInfo.clientType}`);
    }
    if (packageInfo.appType && packageInfo.appType !== 'tinyApp') {
      throw new Error(`Unsupported mPaaS package type: ${packageInfo.appType}`);
    }
    const name = options.name || appId;
    const tarPath = path.join(output, `${appId}.tar`);
    const stagedTar = path.join(work, 'publish.tar');
    const publishTar = buildResult.enableSubPack
      ? await mergePackageTars({
        mainTar: compiledTar,
        subPackages: buildResult.subPackages,
        output: path.join(work, 'merged.tar'),
      })
      : compiledTar;
    await fs.copyFile(publishTar, stagedTar);
    const report = {
      appId, name, version, tarPath, mainUrl: packageInfo.mainUrl,
      extendInfo: JSON.parse(packageInfo.extendInfo || '{}'),
      clientType: packageInfo.clientType || clientType,
      appType: packageInfo.appType,
      subUrl: packageInfo.subUrl || undefined,
      extraInfo: packageInfo.extraInfo || undefined,
      pluginList: Array.isArray(packageInfo.pluginList) ? packageInfo.pluginList : [],
      compileVersion: metadata.compileVersion, renderType: metadata.renderType,
      subpackageFlag: buildResult.enableSubPack === true,
    };
    if (format !== 'amr') report.zip = await createUploadZip({ tarPath: stagedTar, appId, mainUrl: report.mainUrl, output: path.join(output, `${appId}.zip`) });
    if (format !== 'zip') report.amr = await createAmr({ tarPath: stagedTar, appId, name, version, output: path.join(output, `${appId}.amr`) });
    await fs.rename(stagedTar, tarPath);
    const reportFile = path.join(output, `${appId}.build.json`);
    const stagedReport = path.join(work, 'build.json');
    await fs.writeFile(stagedReport, JSON.stringify(report, null, 2));
    await fs.rename(stagedReport, reportFile);
    return { ...report, report: reportFile };
  } finally {
    await fs.rm(work, { recursive: true, force: true });
  }
}
