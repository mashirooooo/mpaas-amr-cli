import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import AdmZip from 'adm-zip';

const [executable, target = `${process.platform}-${process.arch}`] = process.argv.slice(2);
if (!executable || !['linux-x64', 'darwin-arm64', 'win32-x64'].includes(target)) {
  throw new Error('Usage: node scripts/package-publisher.mjs <native-openapi-executable> [linux-x64|darwin-arm64|win32-x64]');
}

const root = path.resolve(import.meta.dirname, '..');
const folder = `mpaas-publisher-${target}`;
const binaryName = `mpaas-openapi-rs${target === 'win32-x64' ? '.exe' : ''}`;
const archive = new AdmZip();
archive.addFile(`${folder}/${binaryName}`, await fs.readFile(path.resolve(executable)), '', 0o100755);
archive.addFile(`${folder}/README.md`, await fs.readFile(path.join(root, 'docs/publishing.md')), '', 0o100644);
archive.addFile(`${folder}/api-reference.md`, await fs.readFile(path.join(root, 'docs/api-reference.md')), '', 0o100644);
for (const name of ['version', 'upload-defaults', 'upload', 'upload-and-publish', 'publish', 'api']) {
  archive.addFile(`${folder}/${name}.example.json`, await fs.readFile(path.join(root, 'config', `${name}.example.json`)), '', 0o100644);
}
archive.addFile(`${folder}/.gitignore`, Buffer.from('*.local.json\nkeys/\nresult.json\nerror.log\n'), '', 0o100644);
const output = path.join(root, 'dist', `${folder}.zip`);
const bytes = archive.toBuffer();
await fs.mkdir(path.dirname(output), { recursive: true });
await fs.writeFile(output, bytes);
const sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
await fs.writeFile(`${output}.sha256`, `${sha256}  ${path.basename(output)}\n`);
console.log(JSON.stringify({ output, bytes: bytes.length, sha256 }, null, 2));
