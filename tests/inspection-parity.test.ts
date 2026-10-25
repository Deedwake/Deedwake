// @ts-ignore Node built-ins come from the existing test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import { execFileSync } from 'node:child_process';
// @ts-ignore See above.
import { fileURLToPath } from 'node:url';
import { Repair } from '../web/repair.ts';
import type { Action } from '../web/fixture.ts';

type Cause = { fact: string; source: string; time: number };
type Decision = { action: string; disposition: string; reason: string; causes: Cause[] };
type NativeReport = {
  steps: { time: number; action: string; result: number; decision: Decision | null }[];
  actions: { action: string; status: string; rejection: string | null; confirmed: Cause | null }[];
  dropped_trace: number;
};
const ids: Record<string, Action> = {
  reset: 0, submit: 1, inspect: 2, repair: 3, collect: 4, ask_intake: 5, wait: 6,
};

test('browser inspection agrees with native CLI decisions and actual outcomes from shared Rust', async () => {
  const binary = fileURLToPath(new URL('../target/debug/repair-tool', import.meta.url));
  const config = fileURLToPath(new URL('../repair-web/repair.toml', import.meta.url));
  const native: NativeReport = JSON.parse(execFileSync(binary,
    ['explain', config, '--case', 'rejection-then-success', '--json'], { encoding: 'utf8' }));
  const bytes = readFileSync(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url));
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const repair = new Repair(instance);
  for (const step of native.steps) {
    const before = repair.snapshot();
    const trace = repair.developerTrace();
    const inspection = repair.developerInspection();
    assert.deepEqual(repair.developerInspection(), inspection);
    assert.deepEqual(repair.snapshot(), before);
    assert.equal(repair.developerTrace(), trace);
    assert.equal(inspection.time, step.time);
    if (step.decision) {
      const actual = inspection.decisions.find(d => d.action === step.decision!.action);
      assert.deepEqual(actual, {
        ...step.decision,
        causes: step.decision.causes.map(({ fact, source, time }) => ({ fact, source, time })),
      });
    }
    const action = ids[step.action];
    assert.notEqual(action, undefined, 'native case only uses the compiled action allowlist');
    assert.equal(repair.act(action!), step.result);
  }
  const final = repair.developerInspection();
  assert.deepEqual(final.actions, native.actions.map(a => ({
    action: a.action, status: a.status, rejection: a.rejection, confirmed: a.confirmed?.fact ?? null,
  })));
  assert.equal(final.droppedTrace, native.dropped_trace);
});
