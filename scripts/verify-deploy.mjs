import { verifyWorkflows } from './verify-release.mjs';

if (!process.argv[2] || process.argv.slice(3).some((argument) => argument !== '--client')) {
  throw new Error('Usage: node scripts/verify-deploy.mjs <standalone-or-rust-executable> [--client]');
}
await verifyWorkflows(process.argv[2], { compile: false, client: process.argv.includes('--client') });
