// @ts-ignore Node built-ins are supplied by the runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import process from 'node:process';
import { ConfiguredConvoy } from '../web/configured.ts';
import { CheckpointSession } from '../web/checkpoint.ts';
import type { Action } from '../web/fixture.ts';

const bytes = readFileSync(process.env.DEEDWAKE_CONFIGURED_WASM ?? new URL('../public/convoy.wasm', import.meta.url));
const fresh = async () => new ConfiguredConvoy((await WebAssembly.instantiate(bytes, {})).instance);
function act(scene: ConfiguredConvoy | CheckpointSession, actions: readonly Action[]) {
  for (const action of actions) assert.equal(scene.act(action), 0);
}

test('confidence distinguishes actual received verification from sufficient evidence without remote journal disclosure', async () => {
  const scene = await fresh();
  const config = scene.configuration;
  assert.deepEqual([config.initial, config.routine, config.intermediary], [3, 3, 1]);
  act(scene, [1, 2, 6, 6, 6, 5]);
  const earlier = scene.snapshot().dialogue;
  act(scene, [3, 4, 5]);
  assert.doesNotMatch(scene.snapshot().dialogue, /I received Sela's account, but her verification/);
  const beforeArrival = scene.snapshot().journal;
  act(scene, [6]);
  assert.deepEqual(scene.snapshot().journal, beforeArrival);
  act(scene, [5]);
  const atFour = scene.snapshot().dialogue;
  if (config.retention === 'incomplete') {
    if (config.selaConfidence === 'trusted') {
      assert.match(atFour, config.reaction === 'restrict_until_correction' ? /Service is restored/ : /warning is resolved/);
      assert.match(atFour, /Sela's verified release receipt/);
    } else {
      assert.match(atFour, /I received Sela's account, but her verification alone is unverified for me/);
      assert.doesNotMatch(atFour, /Service is restored|warning is resolved/);
    }
  } else {
    assert.match(atFour, /inspection context keeps loading service available/);
    assert.doesNotMatch(atFour, /Service is restored|warning is resolved/);
  }
  act(scene, [2, 5]);
  assert.equal(scene.snapshot().dialogue, atFour);
  const atFourHistory = scene.snapshot().journal;
  act(scene, [6, 6]);
  assert.deepEqual(scene.snapshot().journal, atFourHistory);
  act(scene, [5]);
  const final = scene.snapshot();
  if (config.retention === 'incomplete' && config.selaConfidence === 'unverified') {
    assert.match(final.dialogue, /your gate release receipt/);
    assert.match(final.dialogue, /At hour 6/);
  }
  if (config.reaction === 'warn_and_continue' || config.retention === 'precise') {
    assert.doesNotMatch(final.dialogue, /remember refusing/);
  }
  assert.ok(final.journal.includes(earlier));
  act(scene, [0]);
  assert.deepEqual(scene.snapshot(), (await fresh()).snapshot());
  assert.equal(scene.configuration.selaConfidence, config.selaConfidence);
});

test('successful-input replay preserves confidence decisions and history at every boundary', async () => {
  const actions: Action[] = [1, 2, 6, 6, 6, 5, 3, 4, 5, 6, 5, 2, 5, 6, 6, 5];
  for (let prefix = 0; prefix <= actions.length; prefix += 1) {
    const session = await CheckpointSession.fromConfiguredBytes(bytes), reference = await fresh();
    act(session, actions.slice(0, prefix)); act(reference, actions.slice(0, prefix));
    const save = session.saveCheckpoint();
    assert.equal(JSON.parse(save).schema, 2);
    act(session, [2, 5]);
    for (let repeat = 0; repeat < 2; repeat += 1) {
      await session.loadCheckpoint(new Blob([save]));
      assert.deepEqual(session.snapshot(), reference.snapshot());
      assert.equal(session.developerTrace(), reference.developerTrace());
    }
    for (const action of actions.slice(prefix)) {
      act(session, [action]); act(reference, [action]);
      assert.deepEqual(session.snapshot(), reference.snapshot());
      assert.equal(session.developerTrace(), reference.developerTrace());
    }
  }
});
