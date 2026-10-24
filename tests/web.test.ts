// @ts-ignore Node built-ins are provided by the test runtime; no Node typings dependency.
import test from 'node:test';
// @ts-ignore Node built-ins are provided by the test runtime; no Node typings dependency.
import assert from 'node:assert/strict';
// @ts-ignore Node built-ins are provided by the test runtime; no Node typings dependency.
import { readFile } from 'node:fs/promises';
import { Convoy, loadConvoy } from '../web/convoy.ts';
import type { Action } from '../web/convoy.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url));
async function fresh() {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, convoy: new Convoy(instance) };
}
function run(convoy: Convoy, actions: readonly Action[]) {
  for (const action of actions) assert.equal(convoy.act(action), 0);
  return convoy.snapshot();
}
const incident: readonly Action[] = [1, 6, 6, 6, 5, 3];

test('actual Wasm ABI loads and rejects invalid commands and byte/field bounds', async () => {
  const { instance, convoy } = await fresh();
  const abi = instance.exports as Record<string, (...args: number[]) => number>;
  assert.equal(abi.dw_version!(), 1);
  const initial = convoy.snapshot();
  assert.equal(initial.time, 0);
  assert.equal(abi.dw_act!(99), 1);
  assert.equal(abi.dw_enabled!(99), 0);
  assert.equal(abi.dw_act!(3), 1);
  assert.equal(abi.dw_act!(4), 1);
  assert.equal(abi.dw_len!(99) >>> 0, 0xffff_ffff);
  assert.equal(abi.dw_byte!(99, 0), 256);
  assert.equal(abi.dw_byte!(0, abi.dw_len!(0)), 256);
  assert.equal(abi.dw_byte!(0, -1), 256);
  assert.deepEqual(convoy.snapshot(), initial);
  assert.throws(() => convoy.act(99 as Action), /Invalid Wasm action/);
});

test('arrival changes no learned journal entry until the player asks Ivo', async () => {
  const { convoy } = await fresh();
  run(convoy, [1, 5, 6, 6]);
  const before = convoy.snapshot();
  const after = run(convoy, [6]);
  assert.deepEqual(after.journal, before.journal);
  assert.equal(after.dialogue, before.dialogue);
  assert.doesNotMatch(JSON.stringify(after), /Service restricted/);
  assert.match(run(convoy, [5]).dialogue, /Service is restricted/);
});

test('receipt through Sela restores at hour four; regular correction takes until six', async () => {
  const { convoy: fast } = await fresh();
  run(fast, incident);
  assert.match(run(fast, [4, 5]).dialogue, /Service is restricted/);
  const fastView = run(fast, [6, 5]);
  assert.equal(fastView.time, 4);
  assert.match(fastView.dialogue, /Service is restored/);
  assert.match(fastView.dialogue, /remember refusing you/);
  assert.ok(fastView.journal.some(entry => /Service is restricted/.test(entry)));

  const { convoy: slow } = await fresh();
  run(slow, incident);
  assert.match(run(slow, [6, 6, 5]).dialogue, /Service is restricted/);
  const slowView = run(slow, [6, 5]);
  assert.equal(slowView.time, 6);
  assert.match(slowView.dialogue, /Service is restored/);
  assert.match(slowView.dialogue, /remember refusing you/);
});

test('questions preserve testimony and clock, and restart resets the fixture', async () => {
  const { convoy } = await fresh();
  const initial = convoy.snapshot();
  const testimony = run(convoy, [1, 2]).dialogue;
  assert.match(testimony, /reason and later events were out of view/);
  assert.equal(run(convoy, [3, 4, 2, 5]).time, 0);
  assert.equal(run(convoy, [6, 6, 6, 2]).dialogue, testimony);
  assert.deepEqual(run(convoy, [0]), initial);
});

test('player snapshot has no hidden trace and never reads its field', async () => {
  const { instance } = await fresh();
  const reads: number[] = [];
  // Instrument only the real instance boundary; every simulation call still runs Rust.
  const exports = { ...instance.exports };
  const realLength = exports.dw_len as (field: number) => number;
  exports.dw_len = (field: number) => { reads.push(field); return realLength(field); };
  const convoy = new Convoy({ exports } as WebAssembly.Instance);
  run(convoy, incident);
  const view = convoy.snapshot();
  assert.deepEqual(Object.keys(view).sort(), ['choices', 'dialogue', 'journal', 'time']);
  assert.ok(reads.length > 0);
  assert.ok(reads.every(field => field === 0 || field === 1));
  assert.ok(convoy.developerTrace().length > 0);
  assert.equal(reads.at(-1), 2);
});

test('ordinary HTML does not opt into developer trace access', async () => {
  const html = await readFile(new URL('../index.html', import.meta.url), 'utf8');
  assert.doesNotMatch(html, /developer|trace/i);
});

test('real binary reaches its time limit without mutation, and restart remains available', async () => {
  const { convoy } = await fresh();
  for (let hour = 0; hour < 1000; hour += 1) assert.equal(convoy.act(6), 0);
  const before = convoy.snapshot();
  assert.equal(before.time, 1000);
  assert.equal(before.choices.find(choice => choice.id === 6)?.enabled, false);
  assert.equal(convoy.act(6), 2);
  assert.deepEqual(convoy.snapshot(), before);
  assert.equal(run(convoy, [0]).time, 0);
});

test('loader fetches and instantiates the real binary and reports fetch/compile failures', async () => {
  const url = URL.createObjectURL(new Blob([bytes], { type: 'application/wasm' }));
  try {
    const convoy = await loadConvoy(url);
    assert.equal(convoy.snapshot().time, 0);
    assert.equal(run(convoy, [1]).journal.length, 1);
  } finally {
    URL.revokeObjectURL(url);
  }
  await assert.rejects(loadConvoy('unsupported:convoy.wasm'), /Could not fetch convoy.wasm/);
  await assert.rejects(loadConvoy('data:application/wasm,not-a-wasm-binary'), /Could not start convoy.wasm/);
});

test('host rejects an incompatible or corrupt ABI before displaying its output', async () => {
  const { instance } = await fresh();
  // Corrupt the boundary for validation; all unmodified functions remain the real Wasm exports.
  const bind = (overrides: Record<string, (...args: number[]) => number>) =>
    new Convoy({ exports: { ...instance.exports, ...overrides } } as WebAssembly.Instance);
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
