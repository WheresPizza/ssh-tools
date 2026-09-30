import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
const root = fileURLToPath(new URL('../', import.meta.url));
// macOS system Perl (used by DMG packaging) does not support C.UTF-8.
const env = process.platform === 'darwin'
  ? { ...process.env, LANG: 'en_US.UTF-8', LC_ALL: 'en_US.UTF-8', LC_CTYPE: 'en_US.UTF-8' }
  : process.env;
const result = spawnSync(resolve(root, 'node_modules/.bin/tauri'), ['build', ...process.argv.slice(2)], { cwd: root, env, stdio: 'inherit' });
if (result.error) throw result.error;
process.exit(result.status ?? 1);
