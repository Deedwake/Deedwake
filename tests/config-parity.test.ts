// @ts-ignore Node built-in imports use the native runtime without added test types.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import { execFileSync } from 'node:child_process';
// @ts-ignore See above.
import { fileURLToPath } from 'node:url';
import { Repair } from '../web/repair.ts';

test('generated native and actual Wasm have identical encounters, history and causal trace', async () => {
  const native: string = execFileSync(fileURLToPath(new URL('../target/debug/examples/repair_cafe', import.meta.url)), ['--trace'], { encoding: 'utf8' });
  const bytes = readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const repair = new Repair(instance);
  let output = '';
  const actions = [1, 6, 6, 2, 3, 6, 6, 3, 5, 4, 6, 5, 4, 6, 4] as const;
  for (const action of actions) {
    const result = repair.act(action);
    assert.ok(result >= 0 && result <= 2);
    if (result !== 0) output += `Hour ${repair.snapshot().time}: Choice ${action} unavailable (status ${result}); state unchanged.\n`;
    else if (action !== 6) output += `Hour ${repair.snapshot().time}: ${repair.snapshot().dialogue}\n`;
  }
  output += `\nLearned journal:\n${repair.snapshot().journal.join('\n')}\n`;
  output += `\nCausal trace:\n${repair.developerTrace()}\n`;
  assert.equal(output, native);
});
