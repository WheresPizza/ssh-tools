import { mkdtempSync, readdirSync, copyFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = fileURLToPath(new URL('../', import.meta.url));
const temporary = mkdtempSync(join(tmpdir(), 'ssh-gui-icons-'));
const cli = resolve(root, 'node_modules/.bin/tauri');
function generate(args) {
  const result = spawnSync(cli, ['icon', ...args], { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Icon generation failed (${result.status})`);
}
try {
  const desktop = join(temporary, 'desktop');
  generate(['public/app-icon.svg', '--output', desktop]);
  for (const item of readdirSync(desktop, { withFileTypes: true })) {
    if (item.isFile()) copyFileSync(join(desktop, item.name), resolve(root, 'src-tauri/icons', item.name));
  }
  const tray = join(temporary, 'tray');
  generate(['src-tauri/icons/tray-source.svg', '--output', tray, '--png', '22']);
  copyFileSync(join(tray, '22x22.png'), resolve(root, 'src-tauri/icons/tray-icon.png'));
} finally { rmSync(temporary, { recursive: true, force: true }); }
