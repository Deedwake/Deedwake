// @ts-ignore Node built-ins are supplied by the test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { Repair } from '../web/repair.ts';
import type { Action } from '../web/fixture.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url));
type Inspect = (section: number, row: number, column: number) => number;

async function fresh() {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, repair: new Repair(instance), inspect: instance.exports.dw_repair_inspect as Inspect };
}

function run(repair: Repair, actions: readonly Action[]): void {
  for (const action of actions) assert.equal(repair.act(action), 0);
}

test('actual Wasm inspection describes pre-arrival, typed WHY and creation times without changing player knowledge', async () => {
  const { repair } = await fresh();
  const initial = repair.developerInspection();
  assert.equal(initial.version, 1);
  assert.equal(initial.time, 0);
  assert.deepEqual(initial.configuration, {
    schema_version: 1, policy_set: 'repair_cafe_v1', trace_capacity: 128,
    report_to_intake_ticks: 1, report_to_technician_ticks: 2,
    technician_to_intake_ticks: 1, technician_to_collection_ticks: 2, cable_receipt_at_tick: 4,
  });
  assert.deepEqual(initial.decisions.map(({ action, disposition, reason, causes }) => [action, disposition, reason, causes]), [
    ['inspect', 'no_proposal', 'report_not_arrived', []],
    ['repair', 'no_proposal', 'diagnosis_not_arrived', []],
    ['collect', 'no_proposal', 'ready_not_arrived', []],
  ]);
  assert.deepEqual(initial.actions, []);
  run(repair, [1, 6]);
  const beforeArrival = repair.snapshot();
  assert.equal(repair.developerInspection().decisions[0]!.reason, 'report_not_arrived');
  run(repair, [6]);
  const arrived = repair.developerInspection();
  assert.deepEqual(arrived.decisions[0], {
    action: 'inspect', disposition: 'ready_to_execute', reason: 'prerequisites_met',
    causes: [{ fact: 'donor_report', source: 'donor', time: 0 }],
  });
  assert.deepEqual(repair.snapshot().journal, beforeArrival.journal);
  assert.deepEqual(arrived.actions, []);
  run(repair, [2]);
  const diagnosed = repair.developerInspection();
  assert.equal(diagnosed.decisions[0]!.reason, 'already_diagnosed');
  assert.deepEqual(diagnosed.decisions[1], {
    action: 'repair', disposition: 'would_reject', reason: 'cable_receipt_not_arrived',
    causes: [{ fact: 'diagnosis', source: 'technician', time: 2 }],
  });
  assert.deepEqual(diagnosed.actions, [{ action: 'inspect', status: 'confirmed', rejection: null, confirmed: 'diagnosis' }]);
});

test('actual Wasm keeps a prior rejection through reuse, later repair, delayed return and reset', async () => {
  const { repair } = await fresh();
  const initial = repair.developerInspection();
  run(repair, [1, 6, 6, 2, 3]);
  const rejected = repair.developerInspection();
  const rejection = { action: 'repair', status: 'host_rejected', rejection: 'cable_receipt_not_arrived', confirmed: null };
  assert.deepEqual(rejected.actions[1], rejection);
  assert.deepEqual(rejected.decisions[1], { action: 'repair', disposition: 'no_proposal', reason: 'prior_rejection_unchanged', causes: [] });
  run(repair, [3, 3]);
  assert.deepEqual(repair.developerInspection(), rejected);
  run(repair, [6, 6]);
  const ready = repair.developerInspection();
  assert.deepEqual(ready.decisions[1], {
    action: 'repair', disposition: 'ready_to_execute', reason: 'prerequisites_met',
    causes: [{ fact: 'diagnosis', source: 'technician', time: 2 }, { fact: 'cable_receipt', source: 'stock', time: 0 }],
  });
  run(repair, [3]);
  const repaired = repair.developerInspection();
  assert.equal(repaired.decisions[1]!.reason, 'already_repaired');
  assert.equal(repaired.decisions[2]!.reason, 'ready_not_arrived');
  assert.deepEqual(repaired.actions[1], rejection);
  assert.deepEqual(repaired.actions[2], { action: 'repair', status: 'confirmed', rejection: null, confirmed: 'ready' });
  run(repair, [4, 6, 4]);
  assert.equal(repair.developerInspection().decisions[2]!.reason, 'ready_not_arrived');
  run(repair, [6]);
  assert.deepEqual(repair.developerInspection().decisions[2], {
    action: 'collect', disposition: 'ready_to_execute', reason: 'prerequisites_met',
    causes: [{ fact: 'ready', source: 'technician', time: 4 }],
  });
  run(repair, [4]);
  const complete = repair.developerInspection();
  assert.equal(complete.decisions[2]!.reason, 'already_collected');
  assert.equal(complete.actions.length, 4);
  assert.deepEqual(complete.actions[1], rejection);
  assert.deepEqual(complete.actions[3], { action: 'collect', status: 'confirmed', rejection: null, confirmed: 'collected' });
  run(repair, [0]);
  assert.deepEqual(repair.developerInspection(), initial);
});

test('repeated actual Wasm inspection is read-only at every lifecycle stage', async () => {
  const { repair } = await fresh();
  for (const actions of [[], [1, 6], [6, 2], [3], [6, 6], [3], [6, 6, 4]] as const) {
    run(repair, actions);
    const player = repair.snapshot();
    const trace = repair.developerTrace();
    const inspection = repair.developerInspection();
    for (let repeat = 0; repeat < 10; repeat += 1) assert.deepEqual(repair.developerInspection(), inspection);
    assert.deepEqual(repair.snapshot(), player);
    assert.equal(repair.developerTrace(), trace);
  }
});

test('ordinary construction, snapshot and legacy trace do not read the privileged extension', async () => {
  const { instance } = await fresh();
  const exports = { ...instance.exports };
  Object.defineProperty(exports, 'dw_repair_inspect', { get() { throw new Error('privileged extension read'); } });
  const repair = new Repair({ exports } as WebAssembly.Instance);
  run(repair, [1, 6, 6, 2, 3]);
  assert.equal(repair.snapshot().time, 2);
  assert.ok(repair.developerTrace().length > 0);
  assert.throws(() => repair.developerInspection(), /privileged extension read/);
});

test('inspection capability is optional until explicitly requested and checks its arity', async () => {
  const { instance } = await fresh();
  for (const capability of [undefined, 1, () => 1, (_section: number, _row: number) => 1]) {
    const exports = { ...instance.exports, dw_repair_inspect: capability };
    const repair = new Repair({ exports } as unknown as WebAssembly.Instance);
    assert.equal(repair.snapshot().time, 0);
    assert.throws(() => repair.developerInspection(), /Missing or incompatible Wasm export: dw_repair_inspect/);
  }
});

test('actual Wasm returns the invalid sentinel for all absent or out-of-bounds inspection coordinates', async () => {
  const { inspect, repair } = await fresh();
  const invalid = (section: number, row: number, column: number) => assert.equal(inspect(section, row, column) >>> 0, 0xffff_ffff);
  for (const [section, row, column] of [[0, 4, 0], [0, 0, 1], [1, 8, 0], [1, 0, 1], [2, 3, 0], [2, 0, 3], [3, 6, 0], [3, 0, 3], [4, 4, 0], [4, 0, 4], [5, 0, 0], [-1, 0, 0], [0, -1, 0], [0, 0, -1]]) invalid(section!, row!, column!);
  for (let row = 0; row < 6; row += 1) for (let column = 0; column < 3; column += 1) invalid(3, row, column);
  for (let row = 0; row < 4; row += 1) for (let column = 0; column < 4; column += 1) invalid(4, row, column);
  assert.equal(repair.developerInspection().time, 0);
});

test('inspection rejects corrupt versions, bounds, codes, counts and overflow before returning data', async () => {
  const corruptions: readonly (readonly [number, number, number, number, readonly Action[]])[] = [
    [0, 0, 0, 2, []], [0, 1, 0, 1001, []], [0, 1, 0, NaN, []],
    [0, 2, 0, -1, []], [0, 2, 0, 0x1_0000_0000, []], [0, 3, 0, 5, []], [0, 3, 0, -1, []], [0, 3, 0, 1, []],
    [1, 0, 0, 2, []], [1, 1, 0, 2, []], [1, 2, 0, 1025, []],
    ...[3, 4, 5, 6, 7].map(row => [1, row, 0, 1001, []] as const),
    [2, 0, 0, 0, []], [2, 0, 0, 4, []], [2, 0, 1, 11, []], [2, 0, 2, 3, []], [2, 0, 2, 0.5, []],
    [3, 0, 0, 6, [1, 6, 6]], [3, 0, 1, 7, [1, 6, 6]], [3, 0, 2, 3, [1, 6, 6]],
    [3, 1, 0, 0, [1, 6, 6]],
    [4, 0, 0, 4, [1, 6, 6, 2]], [4, 0, 1, 5, [1, 6, 6, 2]],
    [4, 0, 2, 11, [1, 6, 6, 2]], [4, 0, 3, 6, [1, 6, 6, 2]],
    [4, 0, 1, 1, [1, 6, 6, 2]], [4, 0, 1, 2, [1, 6, 6, 2]], [4, 0, 1, 3, [1, 6, 6, 2]],
    [4, 0, 2, 7, [1, 6, 6, 2]], [4, 0, 3, 0, [1, 6, 6, 2]],
    [4, 1, 2, 0, [1, 6, 6, 2, 3]], [4, 1, 3, 4, [1, 6, 6, 2, 3]],
    [4, 1, 0, 1, [1, 6, 6, 2]], [5, 0, 0, 0, []],
  ];
  for (const [section, row, column, value, actions] of corruptions) {
    const { instance, inspect } = await fresh();
    const exports = { ...instance.exports, dw_repair_inspect: (s: number, r: number, c: number) => s === section && r === row && c === column ? value : inspect(s, r, c) };
    const repair = new Repair({ exports } as WebAssembly.Instance);
    run(repair, actions);
    assert.throws(() => repair.developerInspection(), /Wasm repair inspection/, `${section}/${row}/${column}: ${value}`);
  }
});

test('trace dropped count is surfaced independently of action history', async () => {
  const { instance, inspect } = await fresh();
  const exports = { ...instance.exports, dw_repair_inspect: (s: number, r: number, c: number) => s === 0 && r === 2 && c === 0 ? 7 : inspect(s, r, c) };
  const repair = new Repair({ exports } as WebAssembly.Instance);
  run(repair, [1, 6, 6, 2, 3, 6, 6, 3, 6, 6, 4]);
  const view = repair.developerInspection();
  assert.equal(view.droppedTrace, 7);
  assert.equal(view.actions.length, 4);
  assert.equal(view.actions[1]!.status, 'host_rejected');
});

test('inspection decodes valid unsigned dropped counts returned as signed Wasm i32', async () => {
  const { instance, inspect } = await fresh();
  const exports = { ...instance.exports, dw_repair_inspect: (s: number, r: number, c: number) => s === 0 && r === 2 && c === 0 ? -2 : inspect(s, r, c) };
  const repair = new Repair({ exports } as WebAssembly.Instance);
  assert.equal(repair.developerInspection().droppedTrace, 0xffff_fffe);
});
