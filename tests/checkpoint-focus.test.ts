// @ts-ignore Node built-ins are provided by the test runtime; no Node typings dependency.
import test from 'node:test';
// @ts-ignore Node built-ins are provided by the test runtime.
import assert from 'node:assert/strict';
import { restoreCheckpointFocus } from '../web/checkpoint-focus.ts';

test('a load restores displaced focus but does not take focus from another control', () => {
  const body = {} as HTMLElement;
  const load = {} as HTMLButtonElement;
  const other = {} as HTMLElement;
  let active: Element | null = body;
  const documentFocus = { get activeElement() { return active; }, body };
  const choice = { focus() { active = choice; } } as HTMLElement;
  const picker = { focus() { active = picker; } } as HTMLElement;

  restoreCheckpointFocus(true, documentFocus, load, choice);
  assert.equal(active, choice, 'successful load resumes at the story choice');
  active = load;
  restoreCheckpointFocus(true, documentFocus, load, picker);
  assert.equal(active, picker, 'failed load returns to file input');
  active = other;
  restoreCheckpointFocus(true, documentFocus, load, choice);
  assert.equal(active, other, 'intentional focus elsewhere survives load');
  active = body;
  restoreCheckpointFocus(false, documentFocus, load, picker);
  assert.equal(active, body, 'a load that did not start focused does not steal focus');
});
