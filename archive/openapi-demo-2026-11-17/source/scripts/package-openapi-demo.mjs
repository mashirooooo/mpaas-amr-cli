import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import AdmZip from 'adm-zip';

const [executable, target = `${process.platform}-${process.arch}`] = process.argv.slice(2);
if (!executable || !['linux-x64', 'darwin-arm64', 'win32-x64'].includes(target)) {
  throw new Error('Usage: node scripts/package-openapi-demo.mjs <native-openapi-executable> [linux-x64|darwin-arm64|win32-x64]');
}

const root = path.resolve(import.meta.dirname, '..');
const folder = `mpaas-openapi-demo-${target}`;
const executableName = `mpaas-openapi-rs${target === 'win32-x64' ? '.exe' : ''}`;
const zip = new AdmZip();
zip.addFile(`${folder}/${executableName}`, await fs.readFile(path.resolve(executable)), '', 0o100755);
for (const name of ['README.md', 'config.example.json', '.gitignore']) {
  zip.addFile(`${folder}/${name}`, await fs.readFile(path.join(root, 'demo/openapi', name)), '', 0o100644);
}

const bytes = zip.toBuffer();
const name = `${folder}.zip`;
const output = path.join(root, 'dist', name);
await fs.mkdir(path.dirname(output), { recursive: true });
await fs.writeFile(output, bytes);
const hash = crypto.createHash('sha256').update(bytes).digest('hex');
await fs.writeFile(`${output}.sha256`, `${hash}  ${name}\n`);
console.log(JSON.stringify({ output, bytes: bytes.length, sha256: hash }, null, 2));
