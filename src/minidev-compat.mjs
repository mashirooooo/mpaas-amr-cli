import fs from 'node:fs/promises';
import path from 'node:path';

const CLIENT_TYPE_MARKER =
  '(0,c.isDefined)(t.enableModuleGraph)&&(e.enableModuleGraph=t.enableModuleGraph);';
const CLIENT_TYPE_PATCH =
  '(0,c.isDefined)(t.enableModuleGraph)&&(e.enableModuleGraph=t.enableModuleGraph),(0,c.isDefined)(t.clientType)&&(e.clientType=t.clientType),(0,c.isDefined)(t.bundleId)&&(e.bundleId=t.bundleId);';

/**
 * minidev 2.2.5 的公开 build 参数没有把端类型转交给编译上下文。
 * PM 使用本地集成的 minidev，因此在加载它前补齐这两个编译参数。
 */
export async function ensureMinidevBuildOptions(packageRoot) {
  const entry = path.join(packageRoot, 'lib', 'index.js');
  if ((await fs.readFile(entry, 'utf8')).includes(CLIENT_TYPE_PATCH)) return;
  const lock = `${entry}.pm-compat.lock`;
  let lockHandle;
  for (let attempt = 0; attempt < 200; attempt += 1) {
    try {
      lockHandle = await fs.open(lock, 'wx');
      break;
    } catch (error) {
      if (error.code !== 'EEXIST') throw error;
      if ((await fs.readFile(entry, 'utf8')).includes(CLIENT_TYPE_PATCH)) return;
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  }
  if (!lockHandle) throw new Error('Timed out waiting for minidev runtime compatibility patch');
  try {
    const source = await fs.readFile(entry, 'utf8');
    if (source.includes(CLIENT_TYPE_PATCH)) return;
    const markerIndex = source.indexOf(CLIENT_TYPE_MARKER);
    if (markerIndex < 0) {
      throw new Error('Unsupported minidev runtime: build clientType forwarding is unavailable');
    }
    const patched = source.replace(CLIENT_TYPE_MARKER, CLIENT_TYPE_PATCH);
    const temporary = `${entry}.${process.pid}.tmp`;
    try {
      await fs.writeFile(temporary, patched);
      try {
        await fs.rename(temporary, entry);
      } catch {
        // Windows cannot replace an existing module with rename in every runtime.
        await fs.copyFile(temporary, entry);
      }
    } finally {
      await fs.rm(temporary, { force: true });
    }
  } finally {
    await lockHandle.close();
    await fs.rm(lock, { force: true });
  }
}
