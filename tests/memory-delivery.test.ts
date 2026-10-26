// @ts-ignore Node built-ins come from the existing test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { StudyConvoy, loadStudyConvoy } from '../web/study.ts';
import type { StudyCondition } from '../web/study.ts';
import { Convoy } from '../web/convoy.ts';
import type { Action } from '../web/fixture.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url));
type Exports = Record<string, (...args: number[]) => number>;
const profiles = [
  { condition: 'precise-immediate', id: 1, retention: 1, report: 0, routine: 0, intermediary: 0 },
  { condition: 'incomplete-immediate', id: 2, retention: 2, report: 0, routine: 0, intermediary: 0 },
  { condition: 'precise-scheduled', id: 3, retention: 1, report: 3, routine: 3, intermediary: 1 },
  { condition: 'incomplete-scheduled', id: 4, retention: 2, report: 3, routine: 3, intermediary: 1 },
] as const;

async function raw() {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, abi: instance.exports as Exports };
}
async function fresh(condition: StudyCondition = 'incomplete-scheduled') {
  const { instance, abi } = await raw();
  return { instance, abi, convoy: new StudyConvoy(instance, condition) };
}
function run(convoy: StudyConvoy, actions: readonly Action[]) {
  for (const action of actions) assert.equal(convoy.act(action), 0);
  return convoy.snapshot();
}

test('actual Wasm exposes all four compiled profiles and rejects unknown selectors without mutation', async () => {
  const legacy = await raw();
  assert.equal(legacy.abi.dw_init!(), 0);
  assert.deepEqual([0, 1].map(field => legacy.abi.dw_convoy_study!(field)), [1, 0]);
  for (const field of [2, 3, 4, 5, 6, 0xffff_ffff]) {
    assert.equal(legacy.abi.dw_convoy_study!(field) >>> 0, 0xffff_ffff);
  }
  for (const profile of profiles) {
    const { convoy, abi } = await fresh(profile.condition);
    assert.equal(abi.dw_version!(), 1);
    assert.deepEqual([0, 1, 2, 3, 4, 5].map(field => abi.dw_convoy_study!(field)),
      [1, profile.id, profile.retention, profile.report, profile.routine, profile.intermediary]);
    const initial = convoy.snapshot();
    run(convoy, [1, 2, 5]);
    const before = convoy.snapshot();
    const trace = convoy.developerTrace();
    for (const id of [0, 5, 99, -1, 0xffff_ffff]) {
      assert.equal(abi.dw_convoy_study_select!(id), 1);
      assert.deepEqual(convoy.snapshot(), before);
      assert.equal(convoy.developerTrace(), trace);
      assert.equal(abi.dw_convoy_study!(1), profile.id);
    }
    for (const field of [6, 99, -1, 0xffff_ffff]) {
      assert.equal(abi.dw_convoy_study!(field) >>> 0, 0xffff_ffff);
      assert.deepEqual(convoy.snapshot(), before);
      assert.equal(convoy.developerTrace(), trace);
    }
    assert.deepEqual(run(convoy, [0]), initial);
    assert.equal(abi.dw_convoy_study!(1), profile.id);
    run(convoy, [1, 6]);
    assert.equal(abi.dw_convoy_study_select!(profile.id), 0);
    assert.deepEqual(convoy.snapshot(), initial);
  }
});

test('retention never leaks through local action feedback or choices before a conversation', async () => {
  for (const delivery of ['immediate', 'scheduled'] as const) {
    const { convoy: precise } = await fresh(`precise-${delivery}`);
    const { convoy: incomplete } = await fresh(`incomplete-${delivery}`);
    assert.deepEqual(precise.snapshot(), incomplete.snapshot());
    for (const action of [1, 6, 6, 6, 3, 4] as const) {
      assert.equal(precise.act(action), incomplete.act(action));
      assert.deepEqual(precise.snapshot(), incomplete.snapshot());
    }
    run(precise, [2]);
    run(incomplete, [2]);
    assert.notEqual(precise.snapshot().dialogue, incomplete.snapshot().dialogue);
    assert.deepEqual(precise.snapshot().choices, incomplete.snapshot().choices);
  }
});

test('actual Wasm applies retention and delivery independently across every profile and route', async () => {
  for (const profile of profiles) {
    for (const intermediary of [false, true]) {
      const { convoy } = await fresh(profile.condition);
      run(convoy, [1, 2]);
      const testimony = convoy.snapshot().dialogue;
      assert.match(testimony, /hold|held|stopped/i);
      assert.match(testimony, profile.retention === 1 ? /strap/ : /uncertain|out of view|did not retain/i);
      const choices = convoy.snapshot().choices;
      for (let hour = 1; hour <= profile.report; hour += 1) {
        const before = convoy.snapshot();
        const elapsed = run(convoy, [6]);
        assert.equal(elapsed.time, hour);
        assert.deepEqual(elapsed.journal, before.journal);
        assert.deepEqual(elapsed.choices, choices);
        assert.doesNotMatch(elapsed.dialogue, /Service is (restricted|restored|available)/);
        if (hour < profile.report) assert.match(run(convoy, [5]).dialogue, /available/);
      }
      const encountered = run(convoy, [5]);
      assert.match(encountered.dialogue, profile.retention === 1 ? /available/ : /Service is restricted/);
      if (profile.retention === 1) assert.doesNotMatch(encountered.dialogue, /restored|refusing/);
      const refusal = profile.retention === 2 ? encountered.dialogue : undefined;
      run(convoy, [3]);
      if (intermediary) run(convoy, [4]);
      const releaseHour = convoy.snapshot().time;
      const delay = intermediary ? profile.intermediary : profile.routine;
      for (let elapsed = 0; elapsed < delay; elapsed += 1) {
        const asked = run(convoy, [5]);
        assert.match(asked.dialogue, profile.retention === 1 ? /available/ : /Service is restricted/);
        const before = convoy.snapshot();
        const after = run(convoy, [6]);
        assert.deepEqual(after.journal, before.journal);
        assert.deepEqual(after.choices, before.choices);
      }
      assert.equal(convoy.snapshot().time, releaseHour + delay);
      const final = run(convoy, [5]);
      if (profile.retention === 2) {
        assert.match(final.dialogue, /Service is restored/);
        assert.match(final.dialogue, /remember refusing you/);
        assert.ok(final.journal.includes(refusal!));
        assert.match(final.dialogue, new RegExp(`At hour ${releaseHour + delay}, I restored`));
      } else {
        assert.match(final.dialogue, /available/);
        assert.doesNotMatch(final.dialogue, /restored|refusing/);
        assert.ok(final.journal.every(entry => !/Service is restricted|Service is restored/.test(entry)));
      }
      run(convoy, [2, 2]);
      assert.equal(convoy.snapshot().dialogue, testimony);
      assert.equal(convoy.snapshot().journal.filter(entry => entry === testimony).length, 1);
    }
  }
});

test('scheduled unknown information stays unknown until an encounter and precise context avoids an invented restoration', async () => {
  for (const condition of ['precise-scheduled', 'incomplete-scheduled'] as const) {
    const { convoy } = await fresh(condition);
    assert.match(run(convoy, [5]).dialogue, /available/);
    run(convoy, [1]);
    assert.match(run(convoy, [5]).dialogue, /available/);
    const before = convoy.snapshot();
    run(convoy, [6, 6, 6]);
    assert.deepEqual(convoy.snapshot().journal, before.journal);
    assert.deepEqual(convoy.snapshot().choices, before.choices);
    assert.match(run(convoy, [5]).dialogue, condition === 'precise-scheduled' ? /available/ : /restricted/);
  }
  for (const actions of [[1, 3, 6, 6, 6, 5], [1, 3, 4, 6, 5, 6, 6, 5]] as const) {
    const { convoy } = await fresh('incomplete-scheduled');
    const final = run(convoy, actions);
    assert.match(final.dialogue, /available/);
    assert.doesNotMatch(final.dialogue, /restored|refusing/);
    assert.ok(final.journal.every(entry => !/restricted|restored|refusing/.test(entry)));
  }
});

test('late handoff, repeated commands and evicted trace preserve testimony and an earlier refusal', async () => {
  const { convoy } = await fresh('incomplete-scheduled');
  run(convoy, [1, 2]);
  const testimony = convoy.snapshot().dialogue;
  run(convoy, [6, 6, 6, 5]);
  const refusal = convoy.snapshot().dialogue;
  const refused = convoy.snapshot();
  assert.equal(convoy.act(1), 1);
  assert.deepEqual(convoy.snapshot(), refused);
  run(convoy, [3]);
  const released = convoy.snapshot();
  assert.equal(convoy.act(3), 1);
  assert.deepEqual(convoy.snapshot(), released);
  run(convoy, [6, 6, 6, 5]);
  const restored = convoy.snapshot().dialogue;
  assert.match(restored, /your gate release receipt/);
  run(convoy, [4]);
  const sent = convoy.snapshot();
  const trace = convoy.developerTrace();
  assert.equal(convoy.act(4), 1);
  assert.deepEqual(convoy.snapshot(), sent);
  assert.equal(convoy.developerTrace(), trace);
  run(convoy, [6, 5]);
  assert.equal(convoy.snapshot().dialogue, restored);
  for (let repeat = 0; repeat < 90; repeat += 1) run(convoy, [2, 5]);
  assert.match(convoy.developerTrace(), /Dropped entries: [1-9]\d*/);
  assert.equal(convoy.snapshot().dialogue, restored);
  assert.ok(convoy.snapshot().journal.includes(refusal));
  run(convoy, [2]);
  assert.equal(convoy.snapshot().dialogue, testimony);
  assert.equal(convoy.snapshot().journal.filter(entry => entry === testimony).length, 1);
});

test('actual Wasm stops at hour 1000 without mutation and reset preserves the selected study', async () => {
  for (const profile of profiles) {
    const { convoy, abi } = await fresh(profile.condition);
    const initial = convoy.snapshot();
    for (let hour = 0; hour < 1000; hour += 1) assert.equal(convoy.act(6), 0);
    const limit = convoy.snapshot();
    const trace = convoy.developerTrace();
    assert.equal(limit.time, 1000);
    assert.equal(limit.choices.find(choice => choice.id === 6)!.enabled, false);
    assert.equal(convoy.act(6), 2);
    assert.deepEqual(convoy.snapshot(), limit);
    assert.equal(convoy.developerTrace(), trace);
    assert.deepEqual(run(convoy, [0]), initial);
    assert.equal(abi.dw_convoy_study!(1), profile.id);
  }
});

test('player snapshots read ordinary fields only and loading selects each requested compiled study', async () => {
  const { instance, abi } = await raw();
  const reads: number[] = [];
  const convoy = new StudyConvoy({ exports: { ...instance.exports,
    dw_len: (field: number) => { reads.push(field); return abi.dw_len!(field); },
  } } as WebAssembly.Instance, 'incomplete-scheduled');
  run(convoy, [1, 2, 6, 6, 6, 5]);
  assert.deepEqual(Object.keys(convoy.snapshot()).sort(), ['choices', 'dialogue', 'journal', 'time']);
  assert.ok(reads.length > 0 && reads.every(field => field === 0 || field === 1));
  const before = convoy.snapshot();
  assert.ok(convoy.developerTrace().length > 0);
  assert.deepEqual(convoy.snapshot(), before);
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/wasm' }));
  try {
    for (const profile of profiles) {
      const loaded = await loadStudyConvoy(url, profile.condition);
      assert.match(run(loaded, [1, 2]).dialogue, profile.retention === 1 ? /strap/ : /uncertain|out of view|did not retain/i);
      assert.equal(loaded.snapshot().time, 0);
    }
  } finally {
    URL.revokeObjectURL(url);
  }
});

test('study extensions are optional for legacy callers and malformed capabilities fail before selection', async () => {
  const { instance, abi } = await raw();
  const legacy = new Convoy({ exports: { ...instance.exports,
    dw_convoy_study: undefined, dw_convoy_study_select: undefined,
  } } as unknown as WebAssembly.Instance);
  assert.equal(legacy.snapshot().time, 0);
  let selections = 0;
  const select = (id: number) => { selections += 1; return abi.dw_convoy_study_select!(id); };
  const bind = (overrides: Record<string, unknown>, condition: StudyCondition = 'precise-scheduled') =>
    new StudyConvoy({ exports: { ...instance.exports, dw_convoy_study_select: select, ...overrides } } as WebAssembly.Instance, condition);
  const metadata = (field: number, value: number) => ({
    dw_convoy_study: (actual: number) => actual === field ? value : abi.dw_convoy_study!(actual),
  });
  for (const name of ['dw_convoy_study', 'dw_convoy_study_select']) {
    for (const value of [undefined, 1, () => 0, (_a: number, _b: number) => 0]) {
      assert.throws(() => bind({ [name]: value }), new RegExp(`incompatible Wasm export: ${name}`));
      assert.equal(selections, 0);
    }
  }
  for (const [field, values] of [
    [0, [0, 2, NaN, -1, 1.5]],
    [1, [1, 5, NaN, -1, 0x1_0000_0000]],
    [2, [0, 1, 2, NaN]], [3, [0, 3, NaN]], [4, [0, 3, NaN]], [5, [0, 1, NaN]],
    [6, [0, NaN]], [0xffff_ffff, [0, NaN]],
  ] as const) {
    for (const value of values) {
      assert.throws(() => bind(metadata(field, value)), /Wasm.*study/i, `${field}: ${value}`);
      assert.equal(selections, 0);
    }
  }
  assert.throws(() => bind({}, 'other' as StudyCondition), /Invalid.*study/i);
  assert.equal(selections, 0);
  for (const status of [1, 2, -1, NaN, 0.5]) {
    assert.throws(() => bind({ dw_convoy_study_select: (_id: number) => status }), /selection failed/i);
  }
  assert.throws(() => bind({ dw_convoy_study_select: (_id: number) => 0 }), /study/i);
});

test('selected metadata must match the requested compiled profile and remain valid after reset', async () => {
  for (const profile of profiles) {
    for (const [field, invalid] of [[0, 2], [1, profile.id === 1 ? 2 : 1], [2, profile.retention === 1 ? 2 : 1],
      [3, profile.report === 0 ? 3 : 0], [4, 1000], [5, 1001], [5, 0.5], [3, NaN], [6, 0]] as const) {
      const { instance, abi } = await raw();
      let selected = false;
      const exports = { ...instance.exports,
        dw_convoy_study_select: (id: number) => { selected = true; return abi.dw_convoy_study_select!(id); },
        dw_convoy_study: (actual: number) => selected && actual === field ? invalid : abi.dw_convoy_study!(actual),
      };
      assert.throws(() => new StudyConvoy({ exports } as WebAssembly.Instance, profile.condition), /Wasm.*study/i,
        `${profile.condition}/${field}: ${invalid}`);
    }
    const { instance, abi } = await raw();
    let corrupt = false;
    const convoy = new StudyConvoy({ exports: { ...instance.exports,
      dw_convoy_study: (field: number) => corrupt && field === 1 ? 0 : abi.dw_convoy_study!(field),
    } } as WebAssembly.Instance, profile.condition);
    corrupt = true;
    assert.throws(() => convoy.snapshot(), /Wasm.*study/i);
    assert.throws(() => convoy.act(0), /Wasm.*study/i);
  }
});

test('the four ordinary study pages are matched and isolated from developer controls', async () => {
  const pages: string[] = [];
  for (const [index, profile] of profiles.entries()) {
    const letter = 'abcd'[index]!;
    const html = await readFile(new URL(`../memory-delivery-${letter}.html`, import.meta.url), 'utf8');
    assert.match(html, new RegExp(`data-study-trial="${letter.toUpperCase()}"`));
    assert.ok(html.includes(`data-study="${profile.condition}"`));
    assert.doesNotMatch(html, /developer|trace|href=|data-convoy-condition|data-trial=|checkpoint/i);
    pages.push(html
      .replace(/study [A-D]/, 'study X')
      .replace(/data-study-trial="[A-D]"/, 'data-study-trial="X"')
      .replace(/data-study="[^"]+"/, 'data-study="profile"'));
  }
  for (const html of pages) assert.equal(html, pages[0]);
  const developer = await readFile(new URL('../memory-delivery-developer.html', import.meta.url), 'utf8');
  assert.match(developer, /data-developer="true"/);
  assert.match(developer, /data-study="precise-immediate"/);
  assert.doesNotMatch(developer, /data-study-trial|data-convoy-condition|checkpoint/);
});
