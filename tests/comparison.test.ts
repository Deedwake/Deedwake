// @ts-ignore Node built-ins come from the test runtime; no Node typings dependency.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { Convoy, loadConvoy } from '../web/convoy.ts';
import type { Action, ConvoyCondition } from '../web/convoy.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url));
type Exports = Record<string, (...args: number[]) => number>;
async function fresh(condition: ConvoyCondition = 'current') {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, convoy: new Convoy(instance, condition), abi: instance.exports as Exports };
}
function run(convoy: Convoy, actions: readonly Action[]) {
  for (const action of actions) assert.equal(convoy.act(action), 0);
  return convoy.snapshot();
}
const incident: readonly Action[] = [1, 2, 6, 6, 6, 5, 3];
const conditions = ['current', 'control'] as const;

test('all four actual Wasm paths preserve refusal history and restore at the compiled arrival time', async () => {
  for (const condition of conditions) {
    for (const intermediary of [false, true]) {
      const { convoy } = await fresh(condition);
      run(convoy, incident);
      if (intermediary) run(convoy, [4]);
      assert.match(run(convoy, [5]).dialogue, /Service is restricted/);
      const arrival = condition === 'current' && intermediary ? 4 : 6;
      for (let hour = 4; hour <= arrival; hour += 1) {
        const before = convoy.snapshot();
        const elapsed = run(convoy, [6]);
        assert.equal(elapsed.time, hour);
        assert.deepEqual(elapsed.journal, before.journal);
        assert.deepEqual(elapsed.choices, before.choices);
        assert.doesNotMatch(elapsed.dialogue, /restored|restricted/);
        const asked = run(convoy, [5]);
        assert.match(asked.dialogue, hour < arrival ? /Service is restricted/ : /Service is restored/);
      }
      const restored = convoy.snapshot();
      assert.match(restored.dialogue, /remember refusing you/);
      assert.ok(restored.journal.some(entry => /Service is restricted/.test(entry)));
      assert.ok(restored.journal.some(entry => /reason and later events were out of view/.test(entry)));
    }
  }
});

test('trial player states match before correction, apart from compiled timing labels and honest route prose', async () => {
  const { convoy: current } = await fresh('current');
  const { convoy: control } = await fresh('control');
  const observations = (convoy: Convoy) => {
    const view = convoy.snapshot();
    return { ...view, choices: view.choices.map(({ id, enabled }) => ({ id, enabled })) };
  };
  assert.deepEqual(observations(current), observations(control));
  for (const action of [1, 2, 6, 5, 6, 5, 6, 5] as const) {
    run(current, [action]);
    run(control, [action]);
    assert.deepEqual(observations(current), observations(control));
  }
  for (const convoy of [current, control]) run(convoy, [3, 4, 5]);
  assert.deepEqual(observations(current), observations(control));
  assert.match(current.snapshot().choices[2]!.label, /routine pouch \(3 hours\)/);
  assert.match(current.snapshot().choices[3]!.label, /intermediary pouch \(1 hour\)/);
  assert.match(control.snapshot().choices[2]!.label, /routine pouch \(3 hours\)/);
  assert.match(control.snapshot().choices[3]!.label, /intermediary pouch \(3 hours\)/);
});

test('compiled condition metadata, invalid selectors, repeated actions and restart preserve their contract', async () => {
  for (const [condition, id, delay] of [['current', 1, 1], ['control', 2, 3]] as const) {
    const { convoy, abi } = await fresh(condition);
    assert.equal(abi.dw_version!(), 1);
    assert.deepEqual([0, 1, 2, 3].map(field => abi.dw_convoy_condition!(field)), [1, id, 3, delay]);
    for (const field of [4, 99, -1, 0xffff_ffff]) assert.equal(abi.dw_convoy_condition!(field) >>> 0, 0xffff_ffff);
    const initial = convoy.snapshot();
    run(convoy, incident);
    run(convoy, [4]);
    const sent = convoy.snapshot();
    const trace = convoy.developerTrace();
    for (const action of [1, 3, 4] as const) {
      assert.equal(convoy.act(action), 1);
      assert.deepEqual(convoy.snapshot(), sent);
      assert.equal(convoy.developerTrace(), trace);
    }
    for (const invalid of [0, 3, 99, -1, 0xffff_ffff]) {
      assert.equal(abi.dw_convoy_select!(invalid), 1);
      assert.deepEqual(convoy.snapshot(), sent);
      assert.equal(convoy.developerTrace(), trace);
      assert.equal(abi.dw_convoy_condition!(1), id);
    }
    assert.deepEqual(run(convoy, [0]), initial);
    assert.equal(abi.dw_convoy_condition!(1), id);
    run(convoy, [1, 6]);
    assert.equal(abi.dw_convoy_select!(id), 0);
    assert.deepEqual(convoy.snapshot(), initial);
    assert.equal(abi.dw_convoy_select!(id === 1 ? 2 : 1), 0);
    assert.equal(abi.dw_convoy_condition!(1), id === 1 ? 2 : 1);
    assert.equal(abi.dw_time!(), 0);
    assert.equal(abi.dw_len!(1), 0);
  }
});

test('explicit player snapshots never request developer trace and loaders select the requested compiled condition', async () => {
  const { instance } = await fresh();
  const reads: number[] = [];
  const actual = instance.exports.dw_len as (field: number) => number;
  const convoy = new Convoy({ exports: { ...instance.exports, dw_len: (field: number) => { reads.push(field); return actual(field); } } } as WebAssembly.Instance, 'control');
  run(convoy, incident);
  assert.deepEqual(Object.keys(convoy.snapshot()).sort(), ['choices', 'dialogue', 'journal', 'time']);
  assert.ok(reads.length > 0 && reads.every(field => field === 0 || field === 1));
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/wasm' }));
  try {
    for (const condition of conditions) {
      const loaded = await loadConvoy(url, condition);
      const label = loaded.snapshot().choices[3]!.label;
      assert.match(label, condition === 'current' ? /\(1 hour\)/ : /\(3 hours\)/);
    }
  } finally {
    URL.revokeObjectURL(url);
  }
});

test('host validates condition capability before selecting and legacy loading needs no extension', async () => {
  const { instance } = await fresh();
  const real = instance.exports as Exports;
  let selections = 0;
  const select = (id: number) => { selections += 1; return real.dw_convoy_select!(id); };
  const bind = (overrides: Record<string, unknown>, condition: ConvoyCondition = 'control') =>
    new Convoy({ exports: { ...instance.exports, dw_convoy_select: select, ...overrides } } as WebAssembly.Instance, condition);
  const invalidMetadata = (field: number, value: number) => ({ dw_convoy_condition: (actual: number) => actual === field ? value : real.dw_convoy_condition!(actual) });
  const invalid: [Record<string, unknown>, RegExp][] = [
    [{ dw_convoy_select: undefined }, /incompatible Wasm export: dw_convoy_select/],
    [{ dw_convoy_select: () => 0 }, /incompatible Wasm export: dw_convoy_select/],
    [{ dw_convoy_condition: undefined }, /incompatible Wasm export: dw_convoy_condition/],
    [{ dw_convoy_condition: () => 1 }, /incompatible Wasm export: dw_convoy_condition/],
    [invalidMetadata(0, 2), /Unsupported Wasm convoy condition version/],
    [invalidMetadata(4, 0), /Invalid Wasm convoy condition bounds/],
    [invalidMetadata(0xffff_ffff, 0), /Invalid Wasm convoy condition bounds/],
  ];
  for (const value of [0, 3, NaN, -1, 0xffff_ffff]) invalid.push([invalidMetadata(1, value), /Invalid Wasm convoy condition id/]);
  for (const field of [2, 3]) {
    for (const value of [0, -1, 1001, 1.5, NaN, 0xffff_ffff]) invalid.push([invalidMetadata(field, value), /Invalid Wasm convoy condition timing/]);
  }
  for (const [overrides, error] of invalid) {
    assert.throws(() => bind(overrides), error);
    assert.equal(selections, 0);
  }
  assert.throws(() => bind({}, 'other' as ConvoyCondition), /Invalid convoy condition/);
  assert.equal(selections, 0);
  for (const status of [1, 2, -1, NaN]) {
    assert.throws(() => bind({ dw_convoy_select: (_id: number) => status }), /condition selection failed/);
  }
  assert.throws(() => bind({ dw_convoy_select: (_id: number) => 0 }), /condition selection mismatch/);
  const legacy = new Convoy({ exports: { ...instance.exports, dw_convoy_select: undefined, dw_convoy_condition: undefined } } as unknown as WebAssembly.Instance);
  assert.equal(legacy.snapshot().time, 0);
});

test('ordinary trial HTML is matched and has no developer access or links', async () => {
  const a = await readFile(new URL('../trial-a.html', import.meta.url), 'utf8');
  const b = await readFile(new URL('../trial-b.html', import.meta.url), 'utf8');
  assert.equal(a.replaceAll('Trial A', 'Trial B').replace('data-trial="A"', 'data-trial="B"').replace('data-convoy-condition="current"', 'data-convoy-condition="control"'), b);
  for (const html of [a, b]) assert.doesNotMatch(html, /developer|trace|href=|priority|faster|best/i);
  const control = await readFile(new URL('../convoy-control-developer.html', import.meta.url), 'utf8');
  assert.match(control, /data-developer="true"/);
  assert.match(control, /data-convoy-condition="control"/);
});
