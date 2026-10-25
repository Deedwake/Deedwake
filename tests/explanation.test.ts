// @ts-ignore Node built-ins come from the test runtime; no Node typings dependency.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { Convoy } from '../web/convoy.ts';
import type { Action, ConvoyCondition } from '../web/convoy.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url));
const conditions = ['current', 'control'] as const;
const incident: readonly Action[] = [1, 2, 6, 6, 6, 5, 3];
async function fresh(condition: ConvoyCondition = 'current') {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, convoy: new Convoy(instance, condition) };
}
function run(convoy: Convoy, actions: readonly Action[]) {
  for (const action of actions) assert.equal(convoy.act(action), 0);
  return convoy.snapshot();
}
function wait(convoy: Convoy, hours: number) {
  for (let hour = 0; hour < hours; hour += 1) run(convoy, [6]);
}

test('visits explain the confirmed decision hour and actual cited evidence on all four Wasm paths', async () => {
  for (const condition of conditions) {
    for (const intermediary of [false, true]) {
      const { convoy } = await fresh(condition);
      const refused = run(convoy, [1, 2, 6, 6, 6, 5]);
      assert.match(refused.dialogue, /At hour 3, I restricted loading based on Mara's incomplete account\./);
      const refusal = refused.dialogue;
      run(convoy, [3]);
      if (intermediary) run(convoy, [4]);
      const before = convoy.snapshot();
      const fast = condition === 'current' && intermediary;
      const decisionHour = fast ? 4 : 6;
      wait(convoy, decisionHour - 3);
      assert.deepEqual(convoy.snapshot().journal, before.journal);
      assert.doesNotMatch(convoy.snapshot().dialogue, /I restored loading/);
      const restored = run(convoy, [5]);
      const evidence = fast ? "Sela's verified release receipt" : 'your gate release receipt';
      assert.ok(restored.dialogue.includes(`At hour ${decisionHour}, I restored loading based on ${evidence}.`));
      assert.match(restored.dialogue, /remember refusing you/);
      assert.ok(restored.journal.includes(refusal));
      assert.doesNotMatch(restored.dialogue, /basis|RecordId|ActionId|Dropped entries/);
      const journal = restored.journal;
      wait(convoy, 2);
      assert.deepEqual(convoy.snapshot().journal, journal);
      const later = run(convoy, [5]);
      assert.equal(later.dialogue, restored.dialogue);
      assert.deepEqual(later.journal, journal);
      assert.equal(run(convoy, [5]).time, later.time);
    }
  }
});

test('late intermediary delivery and simultaneous arrivals cannot replace the cause of an earlier decision', async () => {
  for (const condition of conditions) {
    const { convoy } = await fresh(condition);
    run(convoy, incident);
    wait(convoy, 2);
    run(convoy, [4]); // Sela sent at hour 5; routine receipt is already due at hour 6.
    wait(convoy, 1);
    const restored = run(convoy, [5]);
    assert.match(restored.dialogue, /At hour 6, I restored loading based on your gate release receipt\./);
    assert.doesNotMatch(restored.dialogue, /based on Sela/);
    wait(convoy, 3);
    assert.equal(run(convoy, [5]).dialogue, restored.dialogue);
  }
  const { convoy } = await fresh();
  run(convoy, incident);
  wait(convoy, 3);
  const restored = run(convoy, [5]);
  run(convoy, [4]); // Verification created after restoration cannot get retrospective credit.
  wait(convoy, 1);
  assert.equal(run(convoy, [5]).dialogue, restored.dialogue);
});

test('confirmed restoration without a witnessed refusal differs from service that was never restricted', async () => {
  for (const condition of conditions) {
    const { convoy } = await fresh(condition);
    run(convoy, [1, 6, 6, 6, 3, 4]);
    wait(convoy, condition === 'current' ? 1 : 3);
    const firstVisit = run(convoy, [5]);
    assert.match(firstVisit.dialogue, /Service is restored/);
    assert.match(firstVisit.dialogue, /I restored loading based on/);
    assert.doesNotMatch(firstVisit.dialogue, /remember refusing you/);
    assert.equal(firstVisit.journal.filter(entry => /Service is restricted/.test(entry)).length, 0);

    run(convoy, [0, 1, 3]); // Report and routine release receipt both arrive at hour 3.
    wait(convoy, 3);
    const neverRestricted = run(convoy, [5]);
    assert.match(neverRestricted.dialogue, /Loading service is available/);
    assert.doesNotMatch(neverRestricted.dialogue, /restored|restricted|At hour|based on|remember refusing/);
  }
});

test('decision explanations survive debug trace eviction and reset cannot reuse their handles', async () => {
  for (const condition of conditions) {
    const { convoy } = await fresh(condition);
    const initial = convoy.snapshot();
    run(convoy, incident);
    run(convoy, [4]);
    wait(convoy, 3);
    const explained = run(convoy, [5]);
    for (let repeat = 0; repeat < 160; repeat += 1) run(convoy, [2]);
    assert.match(convoy.developerTrace(), /Dropped entries: [1-9]/);
    assert.equal(run(convoy, [5]).dialogue, explained.dialogue);
    const journal = convoy.snapshot().journal;
    assert.deepEqual(run(convoy, [5]).journal, journal);
    assert.deepEqual(run(convoy, [0]), initial);
    assert.match(run(convoy, [5]).dialogue, /Loading service is available/);
    assert.doesNotMatch(convoy.snapshot().dialogue, /based on|At hour|restored/);
    run(convoy, incident);
    assert.match(run(convoy, [5]).dialogue, /At hour 3, I restricted loading based on Mara's incomplete account\./);
  }
});

test('ordinary explanation rendering reads no privileged Wasm field and never learns a remote decision while waiting', async () => {
  const { instance } = await fresh();
  const fields: number[] = [];
  const length = instance.exports.dw_len as (field: number) => number;
  const convoy = new Convoy({ exports: {
    ...instance.exports,
    dw_len: (field: number) => { fields.push(field); return length(field); },
  } } as WebAssembly.Instance, 'current');
  run(convoy, [1, 6, 6]);
  const before = convoy.snapshot();
  wait(convoy, 1);
  assert.deepEqual(convoy.snapshot().journal, before.journal);
  assert.doesNotMatch(convoy.snapshot().dialogue, /restricted loading based on/);
  assert.match(run(convoy, [5]).dialogue, /restricted loading based on Mara's incomplete account/);
  assert.ok(fields.length > 0 && fields.every(field => field === 0 || field === 1));
});
