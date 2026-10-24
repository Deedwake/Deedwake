// @ts-ignore Node built-ins are provided by the test runtime; no Node typings dependency.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { Repair, loadRepair } from '../web/repair.ts';
import type { Action } from '../web/fixture.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url));
const diagnosis = 'Inspection confirmed: cable-only fault in the fictional lamp.';
const rejected = 'Repair rejected: no cable availability receipt has reached the technician.';
const ready = 'Repair confirmed: fictional cable replaced; lamp ready for collection.';
const collected = 'Collection confirmed: the fictional lamp was handed over.';
const journey: readonly Action[] = [1, 6, 6, 2, 3, 5, 6, 6, 3, 5, 4, 6, 5, 4, 6, 4];

async function fresh() {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, repair: new Repair(instance) };
}
function run(repair: Repair, actions: readonly Action[]) {
  for (const action of actions) assert.equal(repair.act(action), 0);
  return repair.snapshot();
}

test('repair Wasm rejects invalid and disabled commands and bounds without mutation', async () => {
  const { instance, repair } = await fresh();
  const abi = instance.exports as Record<string, (...args: number[]) => number>;
  assert.equal(abi.dw_version!(), 1);
  run(repair, [1]);
  const before = repair.snapshot();
  const trace = repair.developerTrace();
  assert.equal(abi.dw_act!(99), 1);
  assert.equal(abi.dw_enabled!(99), 0);
  assert.equal(abi.dw_act!(1), 1);
  assert.equal(abi.dw_len!(99) >>> 0, 0xffff_ffff);
  assert.equal(abi.dw_byte!(99, 0), 256);
  assert.equal(abi.dw_byte!(0, abi.dw_len!(0)), 256);
  assert.equal(abi.dw_byte!(0, -1), 256);
  assert.deepEqual(repair.snapshot(), before);
  assert.equal(repair.developerTrace(), trace);
  assert.throws(() => repair.act(99 as Action), /Invalid Wasm action/);
});

test('repair rejection remains historical after the later repair and delayed collection succeed', async () => {
  const { repair } = await fresh();
  const rejectedView = run(repair, [1, 6, 6, 2, 3]);
  assert.equal(rejectedView.time, 2);
  assert.equal(rejectedView.dialogue, rejected);
  assert.ok(rejectedView.journal.includes(diagnosis));
  assert.ok(!rejectedView.journal.includes(ready));
  const rejectedTrace = repair.developerTrace();
  assert.match(rejectedTrace, /Proposed/);
  assert.match(rejectedTrace, /Rejected/);
  assert.ok(!rejectedTrace.includes(ready));
  assert.match(run(repair, [5]).dialogue, /only the donor's incomplete broken report/);

  const repairedView = run(repair, [6, 6, 3]);
  assert.equal(repairedView.time, 4);
  assert.equal(repairedView.dialogue, ready);
  assert.ok(repairedView.journal.includes(rejected));
  assert.match(repair.developerTrace(), /Rejected/);
  assert.match(repair.developerTrace(), /Accepted/);
  assert.match(repair.developerTrace(), /Confirmed/);
  assert.match(run(repair, [5]).dialogue, /inspection finding has arrived, but no confirmed repair receipt/);
  assert.match(run(repair, [4]).dialogue, /no confirmed readiness record/);
  assert.match(run(repair, [6, 5]).dialogue, /confirmed repair receipt has arrived/);
  assert.match(run(repair, [4]).dialogue, /no confirmed readiness record/);
  const complete = run(repair, [6, 4]);
  assert.equal(complete.time, 6);
  assert.equal(complete.dialogue, collected);
  assert.ok(complete.journal.includes(rejected));
});

test('each recipient answers from received records and time never remotely teaches the player', async () => {
  const { repair } = await fresh();
  assert.match(run(repair, [5]).dialogue, /no donor report has reached me yet/);
  run(repair, [1]);
  const beforeArrival = repair.snapshot();
  const trace = repair.developerTrace();
  const atOne = run(repair, [6]);
  assert.deepEqual(atOne.journal, beforeArrival.journal);
  assert.deepEqual(atOne.choices, beforeArrival.choices);
  assert.equal(atOne.dialogue, 'One fictional hour passes. Visit a desk to learn what has reached it.');
  assert.notEqual(repair.developerTrace(), trace);
  assert.match(run(repair, [5]).dialogue, /only the donor's incomplete broken report/);
  assert.match(run(repair, [2]).dialogue, /no donor report has reached me/);
  run(repair, [6, 2]);
  assert.equal(repair.snapshot().dialogue, diagnosis);
  assert.match(run(repair, [5]).dialogue, /only the donor's incomplete broken report/);
  const beforeFinding = repair.snapshot();
  const atThree = run(repair, [6]);
  assert.deepEqual(atThree.journal, beforeFinding.journal);
  assert.deepEqual(atThree.choices, beforeFinding.choices);
  assert.match(run(repair, [5]).dialogue, /inspection finding has arrived, but no confirmed repair receipt/);
});

test('repeated rejected requests and completed choices add no fabricated outcomes or duplicate entries', async () => {
  const { repair } = await fresh();
  run(repair, [1, 6, 6, 2, 3]);
  const before = repair.snapshot();
  const trace = repair.developerTrace();
  for (let repeat = 0; repeat < 10; repeat += 1) {
    assert.equal(repair.act(1), 1);
    assert.equal(repair.act(2), 1);
    assert.equal(repair.act(3), 0);
  }
  assert.deepEqual(repair.snapshot(), before);
  assert.equal(repair.developerTrace(), trace);
  run(repair, [6, 6, 3, 6, 6, 4, 5]);
  const complete = repair.snapshot();
  const completeTrace = repair.developerTrace();
  for (let repeat = 0; repeat < 10; repeat += 1) {
    assert.equal(repair.act(3), 1);
    assert.equal(repair.act(4), 1);
    assert.equal(repair.act(5), 0);
  }
  assert.deepEqual(repair.snapshot(), complete);
  assert.equal(repair.developerTrace(), completeTrace);
});

test('repair replay is deterministic and restart resets all fixture state', async () => {
  const { repair } = await fresh();
  const initial = repair.snapshot();
  const initialTrace = repair.developerTrace();
  const expected = run(repair, journey);
  const expectedTrace = repair.developerTrace();
  assert.deepEqual(run(repair, [0]), initial);
  assert.equal(repair.developerTrace(), initialTrace);
  assert.deepEqual(run(repair, journey), expected);
  assert.equal(repair.developerTrace(), expectedTrace);
  const { repair: separate } = await fresh();
  assert.deepEqual(run(separate, journey), expected);
  assert.equal(separate.developerTrace(), expectedTrace);
});

test('repair player projection reads only dialogue and learned journal fields from real Wasm', async () => {
  const { instance } = await fresh();
  const reads: number[] = [];
  const exports = { ...instance.exports };
  const realLength = exports.dw_len as (field: number) => number;
  const realByte = exports.dw_byte as (field: number, index: number) => number;
  exports.dw_len = (field: number) => { reads.push(field); return realLength(field); };
  exports.dw_byte = (field: number, index: number) => { reads.push(field); return realByte(field, index); };
  const repair = new Repair({ exports } as WebAssembly.Instance);
  const view = run(repair, journey);
  assert.deepEqual(Object.keys(view).sort(), ['choices', 'dialogue', 'journal', 'time']);
  assert.ok(reads.includes(0));
  assert.ok(reads.includes(1));
  assert.ok(reads.every(field => field === 0 || field === 1));
  assert.ok(repair.developerTrace().length > 0);
  assert.equal(reads.at(-1), 2);
});

test('repair time limit preserves state and restart remains available', async () => {
  const { repair } = await fresh();
  for (let hour = 0; hour < 1000; hour += 1) assert.equal(repair.act(6), 0);
  const before = repair.snapshot();
  const trace = repair.developerTrace();
  assert.equal(before.time, 1000);
  assert.equal(before.choices.find(choice => choice.id === 6)?.enabled, false);
  assert.equal(repair.act(6), 2);
  assert.deepEqual(repair.snapshot(), before);
  assert.equal(repair.developerTrace(), trace);
  assert.equal(run(repair, [0]).time, 0);
});

test('repair loader instantiates real Wasm and reports fetch, HTTP and compile failures', async () => {
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/wasm' }));
  try {
    const repair = await loadRepair(url);
    assert.ok(repair instanceof Repair);
    assert.equal(run(repair, [1]).time, 0);
    assert.equal(run(repair, [6, 6, 2]).dialogue, diagnosis);
  } finally {
    URL.revokeObjectURL(url);
  }
  await assert.rejects(loadRepair('unsupported:repair.wasm'), /Could not fetch repair.wasm/);
  await assert.rejects(loadRepair('data:application/wasm,not-a-wasm-binary'), /Could not start repair.wasm/);
  const realFetch = globalThis.fetch;
  try {
    globalThis.fetch = async () => new Response('', { status: 404 });
    await assert.rejects(loadRepair('/repair.wasm'), /Could not fetch repair.wasm: HTTP 404/);
  } finally {
    globalThis.fetch = realFetch;
  }
});

test('shared host rejects incompatible and corrupt repair ABI output', async () => {
  const { instance } = await fresh();
  const bind = (overrides: Record<string, (...args: number[]) => number>) =>
    new Repair({ exports: { ...instance.exports, ...overrides } } as WebAssembly.Instance);
  assert.throws(() => bind({ dw_byte: () => 0 }), /incompatible Wasm export: dw_byte/);
  assert.throws(() => bind({ dw_version: () => 2 }), /Unsupported Wasm ABI version/);
  assert.throws(() => bind({ dw_init: () => 1 }), /Wasm initialization failed/);
  assert.throws(() => bind({ dw_len: (_field: number) => 1024 * 1024 + 1 }).snapshot(), /Invalid Wasm text length/);
  assert.throws(() => bind({ dw_len: (_field: number) => -1 }).snapshot(), /Invalid Wasm text length/);
  assert.throws(() => bind({ dw_byte: (_field: number, _index: number) => 256 }).snapshot(), /Invalid Wasm text byte/);
  assert.throws(() => bind({ dw_byte: (_field: number, _index: number) => 255 }).snapshot(), /encoded data|encoding/i);
  assert.throws(() => bind({ dw_time: () => NaN }).snapshot(), /Invalid Wasm time/);
  assert.throws(() => bind({ dw_enabled: (_action: number) => 2 }).snapshot(), /Invalid Wasm choice status/);
  assert.throws(() => bind({ dw_act: (_action: number) => 3 }).act(1), /Invalid Wasm action status/);
});

test('ordinary repair page keeps developer access on its separate explicit route', async () => {
  const html = await readFile(new URL('../repair.html', import.meta.url), 'utf8');
  const developerHtml = await readFile(new URL('../repair-developer.html', import.meta.url), 'utf8');
  assert.match(html, /data-example="repair"/);
  assert.doesNotMatch(html, /developer|trace/i);
  assert.match(developerHtml, /data-example="repair"/);
  assert.match(developerHtml, /data-developer="true"/);
});
