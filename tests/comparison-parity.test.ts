// @ts-ignore Node built-ins come from the existing test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import { execFileSync } from 'node:child_process';
// @ts-ignore See above.
import { fileURLToPath } from 'node:url';
import { Convoy } from '../web/convoy.ts';
import type { Action } from '../web/fixture.ts';

test('both compiled convoy conditions and routes match native Rust encounters, history and causal trace', async () => {
  const bytes = readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url));
  const binary = fileURLToPath(new URL('../target/debug/examples/convoy', import.meta.url));
  for (const condition of ['current', 'control'] as const) {
    for (const route of ['routine', 'intermediary'] as const) {
      const native = execFileSync(binary, ['--condition', condition, '--route', route, '--trace'], { encoding: 'utf8' });
      const { instance } = await WebAssembly.instantiate(bytes, {});
      const convoy = new Convoy(instance, condition);
      const waits = condition === 'current' && route === 'intermediary' ? 1 : 3;
      const actions: Action[] = [1, 2, 6, 6, 6, 5, 3];
      if (route === 'intermediary') actions.push(4);
      for (let tick = 0; tick < waits; tick += 1) actions.push(6);
      actions.push(5);
      let output = '';
      for (const action of actions) {
        assert.equal(convoy.act(action), 0);
        if (action !== 6) output += `Hour ${convoy.snapshot().time}: ${convoy.snapshot().dialogue}\n`;
      }
      output += `Journal:\n${convoy.snapshot().journal.join('\n')}\n`;
      output += `Trace:\n${convoy.developerTrace()}\n`;
      assert.equal(output, native, `${condition}/${route}`);
      assert.equal(convoy.snapshot().time, waits === 1 ? 4 : 6);
      assert.match(convoy.snapshot().dialogue, /Service is restored/);
    }
  }
});
