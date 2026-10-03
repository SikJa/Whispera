import { spawnSync } from 'node:child_process';
import { cpSync, existsSync, mkdirSync, readdirSync, realpathSync, unlinkSync } from 'node:fs';
import { resolve, extname, sep } from 'node:path';
const vendor = resolve('vendor/edge-drop');
if (!existsSync(resolve(vendor, 'node_modules/vite/bin/vite.js'))) {
  throw new Error('Run npm install --ignore-scripts in vendor/edge-drop before building. Electron is not used at runtime.');
}
const result = spawnSync(process.execPath, ['node_modules/vite/bin/vite.js', 'build', '--config', 'browser-preview/tauri-vite.config.mjs'], { cwd: vendor, stdio: 'inherit' });
if (result.status !== 0) process.exit(result.status || 1);
const destination = resolve('public/library');
mkdirSync(destination, { recursive: true });
const generated = resolve(destination, 'assets');
if (existsSync(generated)) {
  if (realpathSync(generated) !== generated || !generated.startsWith(destination + sep)) throw new Error('Unexpected generated asset path');
  const current = new Set(readdirSync(resolve(vendor, 'browser-dist/assets')));
  for (const entry of readdirSync(generated, { withFileTypes: true })) {
    if (entry.isFile() && ['.js', '.css'].includes(extname(entry.name)) && !current.has(entry.name)) unlinkSync(resolve(generated, entry.name));
  }
}
cpSync(resolve(vendor, 'browser-dist'), destination, { recursive: true });
cpSync(resolve(vendor, 'LICENSE'), resolve(destination, 'EDGE-DROP-LICENSE.txt'));
