import fs from 'node:fs/promises';
import path from 'node:path';

function expandEnvironment(value) {
  if (Array.isArray(value)) return value.map(expandEnvironment);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, expandEnvironment(item)]));
  if (typeof value !== 'string') return value;
  return value.replace(/\$\{([A-Z_][A-Z0-9_]*)\}/g, (_, name) => {
    if (process.env[name] === undefined) throw new Error(`Environment variable ${name} is required`);
    return process.env[name];
  });
}

export async function loadConfig(file) {
  const configPath = path.resolve(file);
  const base = path.dirname(configPath);
  const config = expandEnvironment(JSON.parse(await fs.readFile(configPath, 'utf8')));
  if (!config || typeof config !== 'object' || Array.isArray(config)) throw new Error('Configuration must be a JSON object');
  const resolve = (value) => value ? path.resolve(base, value) : value;
  config.privateKeyFile = resolve(config.privateKeyFile);
  if (config.upload) {
    config.upload.icon = resolve(config.upload.icon);
    config.upload.resource = resolve(config.upload.resource);
  }
  if (config.build) {
    config.build.project = resolve(config.build.project);
    config.build.output = resolve(config.build.output ?? '../artifacts');
  }
  return config;
}
