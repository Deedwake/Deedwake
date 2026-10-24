// @ts-ignore Node built-ins use the runtime's built-in test runner; no third-party test types.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync, readdirSync } from 'node:fs';

test('production export carries real Rust binary and generated browser assets only', () => {
  const dist = new URL('../dist/', import.meta.url);
  const bytes = readFileSync(new URL('convoy.wasm', dist));
  assert.deepEqual(bytes, readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url)));
  const module = new WebAssembly.Module(bytes);
  assert.deepEqual(WebAssembly.Module.imports(module), []);
  const exports = WebAssembly.Module.exports(module).map(item => item.name);
  for (const name of ['dw_version', 'dw_init', 'dw_act', 'dw_time', 'dw_enabled', 'dw_len', 'dw_byte']) assert.ok(exports.includes(name));
  for (const file of ['index.html', 'developer.html']) {
    const html = readFileSync(new URL(file, dist), 'utf8');
    assert.match(html, /\/assets\/.+\.js/);
    assert.doesNotMatch(html, /\.ts["']/);
  }
  const files: string[] = readdirSync(dist, { recursive: true });
  assert.ok(files.some(name => name.endsWith('.js')));
  assert.ok(files.every(name => !/historical|simulation\.mjs|app\.mjs|node_modules/.test(name)));
});
