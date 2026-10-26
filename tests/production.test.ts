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
  const repairBytes = readFileSync(new URL('repair.wasm', dist));
  assert.deepEqual(repairBytes, readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url)));
  const repairModule = new WebAssembly.Module(repairBytes);
  assert.deepEqual(WebAssembly.Module.imports(repairModule), []);
  for (const name of ['dw_version', 'dw_init', 'dw_act', 'dw_time', 'dw_enabled', 'dw_len', 'dw_byte']) assert.ok(WebAssembly.Module.exports(repairModule).some(item => item.name === name));
  for (const file of ['index.html', 'developer.html', 'repair.html', 'repair-developer.html', 'trial-a.html', 'trial-b.html', 'convoy-control-developer.html', 'memory-delivery-a.html', 'memory-delivery-b.html', 'memory-delivery-c.html', 'memory-delivery-d.html', 'memory-delivery-developer.html']) {
    const html = readFileSync(new URL(file, dist), 'utf8');
    assert.match(html, /\/assets\/.+\.js/);
    assert.doesNotMatch(html, /\.ts["']/);
  }
  const files: string[] = readdirSync(dist, { recursive: true });
  assert.ok(files.some(name => name.endsWith('.js')));
  assert.ok(files.every(name => !/historical|simulation\.mjs|app\.mjs|node_modules/.test(name)));
});
