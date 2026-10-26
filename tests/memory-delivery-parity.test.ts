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
import { StudyConvoy } from '../web/study.ts';
import type { Action } from '../web/fixture.ts';

const bytes = readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url));
const binary = fileURLToPath(new URL('../target/debug/examples/convoy', import.meta.url));
const cases: readonly (readonly [string, readonly Action[]])[] = [
  ['routine, earlier refusal and immutable testimony', [0, 5, 1, 2, 2, 5, 6, 5, 6, 5, 6, 5, 3, 5, 6, 5, 6, 5, 6, 5, 2]],
  ['intermediary overtakes routine', [0, 1, 2, 6, 6, 6, 5, 3, 4, 5, 6, 5, 6, 6, 5, 2]],
  ['report and routine receipt arrive together', [0, 5, 1, 3, 6, 6, 6, 5, 2]],
  ['verification precedes the report', [0, 1, 3, 4, 6, 5, 6, 6, 5]],
  ['late handoff preserves the confirmed explanation', [0, 1, 6, 6, 6, 5, 3, 6, 6, 6, 5, 4, 6, 5]],
  ['equal correction arrival and disabled repeats', [0, 3, 4, 1, 1, 6, 6, 6, 5, 3, 3, 6, 6, 4, 4, 6, 5]],
  ['restart clears history and retains the selected profile', [0, 1, 2, 6, 6, 6, 5, 3, 4, 6, 5, 0, 2, 5, 1, 3, 6, 6, 6, 5]],
  ['trace eviction preserves later evidence', [0, 1, ...Array.from({ length: 80 }, () => [2, 5] as const).flat(), 6, 6, 6, 5, 3, 4, 6, 6, 6, 5, 2]],
  ['time limit and restart', [0, ...Array<Action>(1000).fill(6), 6, 5, 0, 1, 2, 5]],
];

test('actual Wasm study encounters, history, choices and trace agree with the independent native Rust CLI', async () => {
  for (const condition of ['precise-immediate', 'incomplete-immediate', 'precise-scheduled', 'incomplete-scheduled'] as const) {
    for (const [name, actions] of cases) {
      const native = execFileSync(binary, ['--study', condition, '--actions', actions.join(','), '--trace'],
        { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
      const { instance } = await WebAssembly.instantiate(bytes, {});
      const convoy = new StudyConvoy(instance, condition);
      const enabled = instance.exports.dw_enabled as (action: number) => number;
      let output = '';
      for (let index = 0; index < actions.length; index += 1) {
        const action = actions[index]!;
        const result = convoy.act(action);
        const state = convoy.snapshot();
        output += `Action ${action}: result ${result}\nHour ${state.time}: ${state.dialogue}\n`;
        assert.deepEqual(state.choices.map(choice => choice.enabled),
          Array.from({ length: 6 }, (_, id) => enabled(id + 1) === 1), `${condition}/${name}/${index}`);
      }
      output += `Journal:\n${convoy.snapshot().journal.join('\n')}\n`;
      output += `Enabled: [${Array.from({ length: 7 }, (_, id) => enabled(id)).join(', ')}]\n`;
      output += `Trace:\n${convoy.developerTrace()}\n`;
      assert.equal(output, native, `${condition}/${name}`);
    }
  }
});
