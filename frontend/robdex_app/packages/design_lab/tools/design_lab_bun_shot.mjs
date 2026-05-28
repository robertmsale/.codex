#!/usr/bin/env node
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const designLabRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const localBun = path.join(designLabRoot, 'node_modules', '.bin', 'bun');
const bun = existsSync(localBun) ? localBun : 'bun';
const script = path.join(designLabRoot, 'tools', 'design_lab_bun_shot.ts');

const result = spawnSync(bun, [script, ...process.argv.slice(2)], {
  cwd: designLabRoot,
  stdio: 'inherit',
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
