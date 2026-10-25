// @ts-ignore Node built-ins are supplied by the test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { Repair } from '../web/repair.ts';
import { DEVELOPER_CASES, readDeveloperCheck, runDeveloperCheck } from '../web/checks.ts';
import type { Action } from '../web/fixture.ts';

const bytes = await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url));
const INVALID = 0xffff_ffff;
type Value = (section: number, row: number, column: number) => number;
type Length = (field: number, row: number) => number;
type Byte = (field: number, row: number, index: number) => number;

async function fresh() {
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return { instance, repair: new Repair(instance) };
}

function modified(instance: WebAssembly.Instance, overrides: Record<string, unknown>): WebAssembly.Exports {
  return { ...instance.exports, ...overrides } as WebAssembly.Exports;
}

test('all five actual Wasm cases return typed Rust outcomes and repeat from fresh fixtures', async () => {
  const { repair } = await fresh();
  assert.equal(repair.developerCheckReport(), null);
  assert.deepEqual(DEVELOPER_CASES.map(c => c.id), [1, 2, 3, 4, 5]);
  for (const choice of DEVELOPER_CASES) {
    const report = repair.runDeveloperCheck(choice.id);
    assert.equal(report.version, 1);
    assert.equal(report.caseId, choice.id);
    assert.equal(report.ok, choice.id !== 3);
    assert.equal(report.notChecked, 0);
    assert.equal(report.traceCapacity, 128);
    assert.equal(report.droppedTrace, 0);
    assert.ok(report.assertions.length > 0);
    assert.deepEqual(repair.developerCheckReport(), report);
    for (let repeat = 0; repeat < 3; repeat += 1) assert.deepEqual(repair.runDeveloperCheck(choice.id), report);
    const other = await fresh();
    assert.deepEqual(other.repair.runDeveloperCheck(choice.id), report);
    if (choice.id === 3) {
      assert.deepEqual(report.steps, []);
      assert.deepEqual(report.assertions.map(a => [a.step, a.time, a.status, a.code, a.expected, a.actual]), [
        [0, 0, 'failed', 'equal_report_delays', '1', '2'],
        [0, 0, 'failed', 'equal_ready_delays', '1', '2'],
      ]);
    } else {
      assert.ok(report.assertions.every(a => a.status === 'passed'));
      assert.ok(report.steps.length > 0);
    }
    if (choice.id === 5) {
      assert.equal(report.session, 1);
      const reset = report.steps.find(step => step.action === 0)!;
      assert.equal(reset.session, 1);
      assert.equal(reset.time, 0);
      assert.ok(report.steps.slice(0, reset.index - 1).every(step => step.session === 0));
      assert.ok(report.steps.slice(reset.index - 1).every(step => step.session === 1));
    }
  }
});

test('checks and report reads leave the complete interactive fixture unchanged, and reports survive interactive reset', async () => {
  const { repair } = await fresh();
  const { repair: control } = await fresh();
  for (const action of [1, 6, 6, 2, 3, 5] as const) assert.equal(repair.act(action), control.act(action));
  const player = repair.snapshot();
  const inspection = repair.developerInspection();
  const trace = repair.developerTrace();
  for (const choice of DEVELOPER_CASES) {
    const report = repair.runDeveloperCheck(choice.id);
    for (let repeat = 0; repeat < 3; repeat += 1) assert.deepEqual(repair.developerCheckReport(), report);
    assert.deepEqual(repair.snapshot(), player);
    assert.deepEqual(repair.developerInspection(), inspection);
    assert.equal(repair.developerTrace(), trace);
  }
  const last = repair.developerCheckReport();
  for (const action of [3, 6, 6, 3, 6, 6, 4, 0, 1, 6] as const) {
    assert.equal(repair.act(action), control.act(action));
    assert.deepEqual(repair.snapshot(), control.snapshot());
    assert.deepEqual(repair.developerInspection(), control.developerInspection());
    assert.equal(repair.developerTrace(), control.developerTrace());
    assert.deepEqual(repair.developerCheckReport(), last);
  }
});

test('invalid case IDs are rejected without replacing the stored report or interactive fixture', async () => {
  const { repair, instance } = await fresh();
  const rawRun = instance.exports.dw_repair_check_run as (id: number) => number;
  const report = repair.runDeveloperCheck(2);
  const player = repair.snapshot();
  for (const id of [0, 6, -1, INVALID]) {
    assert.equal(rawRun(id), 1);
    assert.deepEqual(repair.developerCheckReport(), report);
  }
  for (const id of [0, 6, -1, INVALID, NaN, Infinity, 1.5, '1', true, null]) {
    assert.throws(() => repair.runDeveloperCheck(id as number), /Wasm repair check case ID/);
    assert.deepEqual(repair.developerCheckReport(), report);
  }
  assert.deepEqual(repair.snapshot(), player);
});

test('ordinary construction and interaction never access optional inspection or check capabilities', async () => {
  const { instance } = await fresh();
  const exports = { ...instance.exports };
  for (const name of ['dw_repair_inspect', 'dw_repair_check_run', 'dw_repair_check_value', 'dw_repair_check_len', 'dw_repair_check_byte']) {
    Object.defineProperty(exports, name, { get() { throw new Error(`optional getter ${name}`); } });
  }
  const repair = new Repair({ exports } as WebAssembly.Instance);
  for (const action of [1, 6, 6, 2, 3] as readonly Action[]) repair.act(action);
  assert.equal(repair.snapshot().time, 2);
  assert.ok(repair.developerTrace().length > 0);
  assert.throws(() => repair.developerCheckReport(), /optional getter/);
});

test('every check capability is optional until requested and its full arity is checked', async () => {
  const { instance } = await fresh();
  for (const name of ['dw_repair_check_run', 'dw_repair_check_value', 'dw_repair_check_len', 'dw_repair_check_byte']) {
    for (const replacement of [undefined, 1, () => 0, (_a: number, _b: number, _c: number, _d: number) => 0]) {
      const exports = modified(instance, { [name]: replacement });
      const repair = new Repair({ exports } as WebAssembly.Instance);
      assert.equal(repair.snapshot().time, 0);
      assert.throws(() => repair.developerCheckReport(), /Missing or incompatible Wasm export/);
      assert.throws(() => repair.runDeveloperCheck(1), /Missing or incompatible Wasm export/);
    }
  }
  for (const status of [1, 2, -1, NaN, '0', false, undefined]) {
    const exports = modified(instance, { dw_repair_check_run: (_id: number) => status });
    assert.throws(() => runDeveloperCheck(exports, 1), /Wasm repair check/);
  }
  assert.throws(() => runDeveloperCheck(modified(instance, { dw_repair_check_run: (_id: number) => 0 }), 1), /returned case/);
  let calls = 0;
  const value = instance.exports.dw_repair_check_value as Value;
  assert.throws(() => runDeveloperCheck(modified(instance, {
    dw_repair_check_run: (_id: number) => { calls += 1; return 0; },
    dw_repair_check_value: (s: number, r: number, c: number) => s === 0 && r === 0 && c === 0 ? 2 : value(s, r, c),
  }), 1), /version/);
  assert.equal(calls, 0);
});

test('actual Wasm scalar and string coordinates are bounded before and after a stored report', async () => {
  const { repair, instance } = await fresh();
  const value = instance.exports.dw_repair_check_value as Value;
  const length = instance.exports.dw_repair_check_len as Length;
  const byte = instance.exports.dw_repair_check_byte as Byte;
  const invalid = (s: number, r: number, c: number) => assert.equal(value(s, r, c) >>> 0, INVALID);
  for (let row = 2; row <= 10; row += 1) invalid(0, row, 0);
  for (const [section, count] of [[1, 3], [2, 5]]) for (let column = 0; column < count!; column += 1) invalid(section!, 0, column);
  for (let field = 0; field < 3; field += 1) {
    assert.equal(length(field, 0) >>> 0, INVALID);
    assert.equal(byte(field, 0, 0), 256);
  }
  const report = repair.runDeveloperCheck(5);
  for (const [s, r, c] of [[0, 11, 0], [0, 0, 1], [1, report.assertions.length, 0], [1, 128, 0], [1, 0, 3], [2, report.steps.length, 0], [2, 2128, 0], [2, 0, 5], [3, 0, 0], [-1, 0, 0], [0, -1, 0], [0, 0, -1]]) invalid(s!, r!, c!);
  for (const field of [0, 1, 2, 3, -1]) {
    assert.equal(length(field, report.assertions.length) >>> 0, INVALID);
    assert.equal(byte(field, report.assertions.length, 0), 256);
  }
  for (let field = 0; field < 3; field += 1) {
    assert.equal(byte(field, 0, length(field, 0)), 256);
    assert.equal(byte(field, 0, -1), 256);
  }
});

test('decoder rejects corrupt versions, types, counts, statuses, rows and step bounds before returning a report', async () => {
  const { repair, instance } = await fresh();
  const report = repair.runDeveloperCheck(1);
  const value = instance.exports.dw_repair_check_value as Value;
  const corruptions: readonly (readonly [number, number, number, unknown])[] = [
    [0, 0, 0, 2], [0, 1, 0, 2], [0, 2, 0, 0], [0, 2, 0, 6], [0, 3, 0, 2], [0, 3, 0, 0],
    [0, 4, 0, 2129], [0, 5, 0, 2129], [0, 5, 0, report.steps.length - 1],
    [0, 6, 0, 129], [0, 6, 0, report.assertions.length - 1], [0, 7, 0, 1],
    [0, 8, 0, 1001], [0, 9, 0, -1], [0, 9, 0, 0x1_0000_0000], [0, 10, 0, 1025],
    ...[NaN, Infinity, 0.5, '1', true, null, undefined].map(v => [0, 8, 0, v] as const),
    [0, 11, 0, 0], [0, 0, 1, 1], [1, 0, 0, report.steps.length + 1], [1, 0, 1, 1001],
    [1, 0, 2, 0], [1, 0, 2, 4], [1, 0, 2, 2], [1, 0, 2, 3], [1, 0, 3, 0],
    [1, report.assertions.length, 0, 0], [2, 0, 0, 1], [2, 0, 1, 0], [2, 0, 1, 2],
    [2, 0, 2, 1001], [2, 0, 3, 7], [2, 0, 4, 3], [2, report.steps.length, 0, 0], [3, 0, 0, 0],
  ];
  for (const [section, row, column, replacement] of corruptions) {
    const exports = modified(instance, { dw_repair_check_value: (s: number, r: number, c: number) => s === section && r === row && c === column ? replacement : value(s, r, c) });
    assert.throws(() => readDeveloperCheck(exports), /Wasm repair check/, `${section}/${row}/${column}: ${String(replacement)}`);
  }
  repair.runDeveloperCheck(3);
  assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_value: (s: number, r: number, c: number) => s === 0 && r === 3 && c === 0 ? 1 : value(s, r, c) })), /verdict/);
});

test('before-first-report decoder rejects inconsistent presence and absent rows', async () => {
  const { instance } = await fresh();
  const value = instance.exports.dw_repair_check_value as Value;
  for (const [section, row, column] of [[0, 2, 0], [0, 10, 0], [1, 0, 0], [2, 0, 4]]) {
    assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_value: (s: number, r: number, c: number) => s === section && r === row && c === column ? 0 : value(s, r, c) })), /bounds/);
  }
});

test('text decoding is strict UTF-8 with byte, length, field and row bounds', async () => {
  const { repair, instance } = await fresh();
  const report = repair.runDeveloperCheck(1);
  const length = instance.exports.dw_repair_check_len as Length;
  const byte = instance.exports.dw_repair_check_byte as Byte;
  for (const replacement of [-1, 1025, NaN, 0.5, '1', null]) {
    assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_len: (f: number, r: number) => f === 0 && r === 0 ? replacement : length(f, r) })), /text length/);
  }
  for (const replacement of [-1, 256, NaN, 0.5, '65']) {
    assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_byte: (f: number, r: number, i: number) => f === 0 && r === 0 && i === 0 ? replacement : byte(f, r, i) })), /text byte/);
  }
  for (const [field, row] of [[3, 0], [0, report.assertions.length]]) {
    assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_len: (f: number, r: number) => f === field && r === row ? 0 : length(f, r) })), /text bounds/);
  }
  assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_byte: (f: number, r: number, i: number) => f === 0 && r === 0 && i === length(0, 0) ? 0 : byte(f, r, i) })), /text boundary/);
  assert.throws(() => readDeveloperCheck(modified(instance, { dw_repair_check_byte: (f: number, r: number, i: number) => f === 0 && r === 0 && i === 0 ? 0xff : byte(f, r, i) })));
  const utf8 = new TextEncoder().encode('é'.repeat(512));
  const exports = modified(instance, {
    dw_repair_check_len: (f: number, r: number) => f === 2 && r === 0 ? utf8.length : length(f, r),
    dw_repair_check_byte: (f: number, r: number, i: number) => f === 2 && r === 0 ? utf8[i] ?? 256 : byte(f, r, i),
  });
  assert.equal(readDeveloperCheck(exports)!.assertions[0]!.actual, 'é'.repeat(512));
});

test('not checked, zero capacity, truncation and large unsigned counts remain explicit data', async () => {
  const { repair, instance } = await fresh();
  repair.runDeveloperCheck(1);
  const value = instance.exports.dw_repair_check_value as Value;
  const exports = modified(instance, { dw_repair_check_value: (s: number, r: number, c: number) => {
    if (s === 0 && c === 0) {
      if (r === 7) return 1;
      if (r === 9) return -2;
      if (r === 10) return 0;
    }
    return s === 1 && r === 0 && c === 2 ? 3 : value(s, r, c);
  } });
  const report = readDeveloperCheck(exports)!;
  assert.equal(report.ok, true);
  assert.equal(report.notChecked, 1);
  assert.equal(report.assertions[0]!.status, 'not_checked');
  assert.equal(report.traceCapacity, 0);
  assert.equal(report.droppedTrace, INVALID - 1);
});
