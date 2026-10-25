import { table } from './inspection.ts';
import type { RepairInspection } from './repair.ts';

export const DEVELOPER_CASES = [
  { id: 1, label: 'Delayed recipients', name: 'delayed-recipients' },
  { id: 2, label: 'Rejection then success', name: 'rejection-then-success' },
  { id: 3, label: 'Equal-time deliveries', name: 'equal-time' },
  { id: 4, label: 'Repeated actions', name: 'repeated-actions' },
  { id: 5, label: 'Reset', name: 'reset' },
] as const;

export type RepairCheckReport = {
  version: 1;
  caseId: number;
  ok: boolean;
  session: number;
  notChecked: number;
  time: number;
  droppedTrace: number;
  traceCapacity: number;
  assertions: { step: number; time: number; status: 'passed' | 'failed' | 'not_checked'; code: string; expected: string; actual: string }[];
  steps: { session: number; index: number; time: number; action: number; result: number }[];
};

const INVALID = 0xffff_ffff;
const ARITY = { dw_repair_check_run: 1, dw_repair_check_value: 3, dw_repair_check_len: 2, dw_repair_check_byte: 3 } as const;

function uint(value: number, max: number, field: string): number {
  if (!Number.isInteger(value) || value < 0 || value > max) throw new Error(`Invalid Wasm repair check ${field}: ${String(value)}.`);
  return value;
}

function capability(exports: WebAssembly.Exports) {
  for (const [name, arity] of Object.entries(ARITY)) {
    const value = exports[name];
    if (typeof value !== 'function' || value.length !== arity) throw new Error(`Missing or incompatible Wasm export: ${name}.`);
  }
  const call = (name: keyof typeof ARITY, ...args: number[]): number => {
    const value = (exports[name] as CallableFunction)(...args) as number;
    return Number.isInteger(value) && value >= -0x8000_0000 && value < 0 ? value + 0x1_0000_0000 : value;
  };
  return {
    run: (id: number) => call('dw_repair_check_run', id),
    value: (section: number, row: number, column = 0) => call('dw_repair_check_value', section, row, column),
    length: (field: number, row: number) => call('dw_repair_check_len', field, row),
    byte: (field: number, row: number, index: number) => call('dw_repair_check_byte', field, row, index),
  };
}

export function readDeveloperCheck(exports: WebAssembly.Exports): RepairCheckReport | null {
  const abi = capability(exports);
  const absent = (section: number, row: number, column = 0) => {
    if (abi.value(section, row, column) !== INVALID) throw new Error('Invalid Wasm repair check bounds.');
  };
  const absentText = (field: number, row: number) => {
    if (abi.length(field, row) !== INVALID || abi.byte(field, row, 0) !== 256) throw new Error('Invalid Wasm repair check text bounds.');
  };
  if (abi.value(0, 0) !== 1) throw new Error('Unsupported Wasm repair check version; expected 1.');
  const hasReport = uint(abi.value(0, 1), 1, 'report presence');
  for (const [s, r, c] of [[0, 11, 0], [0, 0, 1], [1, 128, 0], [1, 0, 3], [2, 2128, 0], [2, 0, 5], [3, 0, 0]]) absent(s!, r!, c!);
  absentText(3, 0);
  if (!hasReport) {
    for (let row = 2; row <= 10; row += 1) absent(0, row);
    for (let column = 0; column < 3; column += 1) absent(1, 0, column);
    for (let column = 0; column < 5; column += 1) absent(2, 0, column);
    for (let field = 0; field < 3; field += 1) absentText(field, 0);
    return null;
  }
  const caseId = uint(abi.value(0, 2), 5, 'case ID');
  if (caseId === 0) throw new Error('Invalid Wasm repair check case ID.');
  const ok = uint(abi.value(0, 3), 1, 'verdict') === 1;
  const session = uint(abi.value(0, 4), 2128, 'session');
  const stepCount = uint(abi.value(0, 5), 2128, 'step count');
  const assertionCount = uint(abi.value(0, 6), 128, 'assertion count');
  const notChecked = uint(abi.value(0, 7), assertionCount, 'not checked count');
  const time = uint(abi.value(0, 8), 1000, 'final tick');
  const droppedTrace = uint(abi.value(0, 9), INVALID - 1, 'dropped trace count');
  const traceCapacity = uint(abi.value(0, 10), 1024, 'trace capacity');
  const text = (field: number, row: number) => {
    const length = uint(abi.length(field, row), 1024, 'text length');
    const bytes = new Uint8Array(length);
    for (let index = 0; index < length; index += 1) bytes[index] = uint(abi.byte(field, row, index), 255, 'text byte');
    if (abi.byte(field, row, length) !== 256) throw new Error('Invalid Wasm repair check text boundary.');
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  };
  const assertions = Array.from({ length: assertionCount }, (_, row) => {
    const step = uint(abi.value(1, row, 0), stepCount, 'assertion step');
    const tick = uint(abi.value(1, row, 1), 1000, 'assertion tick');
    const status = uint(abi.value(1, row, 2), 3, 'assertion status');
    if (status === 0) throw new Error('Invalid Wasm repair check assertion status.');
    return { step, time: tick, status: (['passed', 'failed', 'not_checked'] as const)[status - 1]!, code: text(0, row), expected: text(1, row), actual: text(2, row) };
  });
  const steps = Array.from({ length: stepCount }, (_, row) => {
    const entry = {
      session: uint(abi.value(2, row, 0), session, 'step session'),
      index: uint(abi.value(2, row, 1), stepCount, 'step index'),
      time: uint(abi.value(2, row, 2), 1000, 'step tick'),
      action: uint(abi.value(2, row, 3), 6, 'step action'),
      result: uint(abi.value(2, row, 4), 2, 'step result'),
    };
    if (entry.index !== row + 1) throw new Error('Invalid Wasm repair check step order.');
    return entry;
  });
  for (let column = 0; column < 3; column += 1) absent(1, assertionCount, column);
  for (let column = 0; column < 5; column += 1) absent(2, stepCount, column);
  for (let field = 0; field < 3; field += 1) absentText(field, assertionCount);
  if (notChecked !== assertions.filter(a => a.status === 'not_checked').length || ok !== !assertions.some(a => a.status === 'failed')) {
    throw new Error('Invalid Wasm repair check verdict or assertion counts.');
  }
  return { version: 1, caseId, ok, session, notChecked, time, droppedTrace, traceCapacity, assertions, steps };
}

export function runDeveloperCheck(exports: WebAssembly.Exports, caseId: number): RepairCheckReport {
  uint(caseId, 5, 'case ID');
  if (caseId === 0) throw new Error('Invalid Wasm repair check case ID.');
  const abi = capability(exports);
  if (abi.value(0, 0) !== 1) throw new Error('Unsupported Wasm repair check version; expected 1.');
  if (abi.run(caseId) !== 0) throw new Error('Wasm repair check did not store the requested case.');
  const report = readDeveloperCheck(exports);
  if (!report || report.caseId !== caseId) throw new Error('Invalid Wasm repair check returned case.');
  return report;
}

export function renderDeveloperCheck(container: HTMLElement, report: RepairCheckReport, configuration: RepairInspection['configuration']): void {
  const heading = document.createElement('h4');
  heading.textContent = `Last separate case result · ${DEVELOPER_CASES.find(choice => choice.id === report.caseId)!.label}`;
  const verdict = document.createElement('p');
  verdict.setAttribute('role', 'status');
  const passed = report.assertions.filter(a => a.status === 'passed').length;
  const failed = report.assertions.filter(a => a.status === 'failed').length;
  const summary = failed > 0 ? 'Case expectations failed.' : report.assertions.length === 0 || report.notChecked > 0 ? 'Check incomplete.' : 'All case assertions passed.';
  verdict.textContent = `${summary} ${passed} passed; ${failed} failed; ${report.notChecked} not checked; ${report.assertions.length} total assertions. ${report.steps.length} executed steps; final session ${report.session}, tick ${report.time}.`;
  const hint = document.createElement('p');
  hint.className = 'hint';
  hint.textContent = 'This stored result belongs to a fresh, separate fixture and does not predict the current interactive encounter. A failed case expectation can mean its configuration assumptions do not hold; compare the expected and actual values before treating it as a software bug. Equal-time requires matching report delays and matching technician delivery delays; delayed recipients requires different report delays; rejection then success requires the cable to arrive after the technician’s report.';
  const retention = document.createElement('p');
  retention.className = 'hint';
  retention.textContent = `Case trace capacity: ${report.traceCapacity}; dropped entries: ${report.droppedTrace}. ${report.droppedTrace > 0 ? 'Trace truncated; any unchecked assertion remains explicitly not checked.' : 'No trace entries dropped.'}`;
  container.replaceChildren(heading, verdict, hint,
    table('Case configuration · compiled read-only values', ['Field', 'Value'], Object.entries(configuration).map(([key, value]) => [key, String(value)])),
    table('Separate case assertions', ['Status', 'Step', 'Tick', 'Code', 'Expected', 'Actual'], report.assertions.map(a => [a.status === 'not_checked' ? 'Not checked' : a.status === 'passed' ? 'Passed' : 'Failed', String(a.step), String(a.time), a.code, a.expected, a.actual])), retention);
}
