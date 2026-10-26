// @ts-ignore Node built-ins are supplied by the runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import { execFileSync } from 'node:child_process';
// @ts-ignore See above.
import { fileURLToPath } from 'node:url';
// @ts-ignore Node built-ins are supplied by the runtime.
import process from 'node:process';
import { CheckpointSession } from '../web/checkpoint.ts';
import { ConfiguredConvoy } from '../web/configured.ts';
import type { Action } from '../web/fixture.ts';

const bytes = readFileSync(process.env.DEEDWAKE_CONFIGURED_WASM ?? new URL('../public/convoy.wasm', import.meta.url));
const binary = process.env.DEEDWAKE_CONFIGURED_NATIVE ?? fileURLToPath(new URL('../target/debug/examples/convoy_configured', import.meta.url));
const paths: readonly (readonly Action[])[] = [
  [0, 5, 1, 2, 2, 5, 6, 5, 6, 5, 6, 5, 3, 4, 6, 5, 6, 6, 5],
  [0, 1, 6, 6, 6, 5, 3, 6, 6, 6, 5, 4, 6, 5],
  [0, 1, 3, 4, 6, 5, 6, 6, 5],
  [0, 3, 4, 1, 1, 3, 3, 4, 4, 6, 6, 6, 5, 0, 1, 2, 5],
  [0, 1, ...Array.from({ length: 90 }, () => [2, 5] as const).flat(), 6, 6, 6, 3, 4, 6, 5],
  [0, ...Array<Action>(1000).fill(6), 6, 5, 0, 1, 2, 5],
];

test('generated native configured host and actual Wasm agree on attempts, dialogue, learned journal, choices and trace', async () => {
  for (const actions of paths) {
    const native = execFileSync(binary, ['--actions', actions.join(','), '--trace'], { encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 });
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const scene = new ConfiguredConvoy(instance);
    const enabled = instance.exports.dw_enabled as (action: number) => number;
    let output = '';
    for (const action of actions) {
      const result = scene.act(action), view = scene.snapshot();
      output += `Action ${action}: result ${result}\nHour ${view.time}: ${view.dialogue}\n`;
    }
    output += `Journal:\n${scene.snapshot().journal.join('\n')}\n`;
    output += `Enabled: [${Array.from({ length: 7 }, (_, id) => enabled(id)).join(', ')}]\n`;
    output += `Trace:\n${scene.developerTrace()}\n`;
    assert.equal(output, native);
  }
});

// This also runs against separately built configuration variants using the same test.
test('configured native/Wasm artifact resumes pending work, clears on reset and never learns remote decisions', async () => {
  const session = await CheckpointSession.fromConfiguredBytes(bytes);
  const reference = new ConfiguredConvoy((await WebAssembly.instantiate(bytes, {})).instance);
  for (const action of [1, 2, 6, 5, 3, 4, 6, 5, 6, 6, 5] as const) {
    const oldJournal = session.snapshot().journal;
    assert.equal(session.act(action), reference.act(action));
    if (action === 6) assert.deepEqual(session.snapshot().journal, oldJournal);
    const file = session.saveCheckpoint();
    await session.loadCheckpoint(new Blob([file]));
    assert.deepEqual(session.snapshot(), reference.snapshot());
    assert.equal(session.developerTrace(), reference.developerTrace());
  }
  session.act(0); reference.act(0);
  assert.deepEqual(session.snapshot(), reference.snapshot());
  assert.equal(session.developerTrace(), reference.developerTrace());
});
