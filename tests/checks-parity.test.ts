// @ts-ignore Node built-ins come from the existing test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import { spawnSync } from 'node:child_process';
// @ts-ignore See above.
import { fileURLToPath } from 'node:url';
import { Repair } from '../web/repair.ts';

const cases = [
  [1, 'delayed-recipients'], [2, 'rejection-then-success'], [3, 'equal-time'],
  [4, 'repeated-actions'], [5, 'reset'],
] as const;
const actions: Record<string, number> = {
  reset: 0, submit: 1, inspect: 2, repair: 3, collect: 4, ask_intake: 5, wait: 6,
};

test('all browser case reports match native CLI assertions, sessions and actual shared Rust steps', async () => {
  const bytes = readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const repair = new Repair(instance);
  for (const action of [1, 6, 6, 2, 3] as const) assert.equal(repair.act(action), 0);
  const player = repair.snapshot();
  const trace = repair.developerTrace();
  const inspection = repair.developerInspection();
  const binary = fileURLToPath(new URL('../target/debug/repair-tool', import.meta.url));
  const config = fileURLToPath(new URL('../repair-web/repair.toml', import.meta.url));
  for (const [id, name] of cases) {
    const native = spawnSync(binary, ['explain', config, '--case', name, '--json'], { encoding: 'utf8' });
    assert.equal(native.error, undefined);
    assert.equal(native.stderr, '');
    const expected = JSON.parse(native.stdout);
    assert.equal(native.status, expected.ok ? 0 : 1);
    const actual = repair.runDeveloperCheck(id);
    assert.equal(actual.version, 1);
    assert.equal(actual.caseId, id);
    assert.equal(actual.ok, expected.ok);
    assert.equal(actual.session, expected.session);
    assert.equal(actual.droppedTrace, expected.dropped_trace);
    assert.equal(actual.traceCapacity, inspection.configuration.trace_capacity);
    assert.equal(actual.notChecked, expected.assertions.filter((a: { status: string }) => a.status === 'not_checked').length);
    assert.deepEqual(actual.assertions, expected.assertions.map((a: {
      step: number; time: number; status: string; code: string; expected: string; actual: string;
    }) => ({ step: a.step, time: a.time, status: a.status, code: a.code, expected: a.expected, actual: a.actual })));
    assert.deepEqual(actual.steps, expected.steps.map((s: {
      session: number; index: number; time: number; action: string; result: number;
    }) => ({ session: s.session, index: s.index, time: s.time, action: actions[s.action], result: s.result })));
    assert.deepEqual(repair.developerCheckReport(), actual);
    assert.deepEqual(repair.runDeveloperCheck(id), actual);
    assert.deepEqual(repair.snapshot(), player);
    assert.equal(repair.developerTrace(), trace);
    assert.deepEqual(repair.developerInspection(), inspection);
  }
});
