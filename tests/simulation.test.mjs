import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { start, act, playerView } from '../simulation.mjs';
const run = actions => actions.reduce(act, start());
const incident = ['delay', 'wait', 'wait', 'wait', 'visit', 'release'];

test('provider reacts only after arrival; player sees only learned information', () => {
  let s = run(['delay', 'visit', 'wait', 'wait']);
  assert.equal(s.restricted, false);
  assert.equal(s.received.length, 0);
  assert.match(s.dialogue, /One fictional hour/);
  const before = playerView(s);
  s = act(s, 'wait');
  assert.equal(s.restricted, true);
  assert.deepEqual(playerView(s).journal, before.journal);
  assert.equal(playerView(s).dialogue, before.dialogue);
  assert.equal(JSON.stringify(playerView(s)).includes('Service restricted'), false);
  assert.deepEqual(Object.keys(playerView(s)).sort(), ['choices', 'dialogue', 'journal', 'time']);
  s = act(s, 'visit');
  assert.match(s.dialogue, /temporarily withholding/);
});

test('testimony is stable and sent report is immutable across release and questions', () => {
  let s = run(['delay', 'question']);
  const testimony = s.dialogue;
  const message = structuredClone(s.sent[0]);
  assert.throws(() => { s.sent[0].content = 'changed'; }, TypeError);
  s = act(s, 'release');
  s = act(s, 'question');
  assert.equal(s.dialogue, testimony);
  assert.deepEqual(s.sent[0], message);
  assert.equal(s.sent[1].kind, 'correction');
  assert.match(s.sent[0].content, /could not see why/);
});

test('intermediary restores sooner than waiting; release alone cannot repair remotely', () => {
  let base = run(incident);
  assert.equal(base.time, 3);
  assert.equal(base.restricted, true);
  let fast = act(base, 'evidence');
  assert.equal(fast.restricted, true);
  fast = act(act(fast, 'wait'), 'visit');
  const slowBefore = run([...incident, 'wait', 'wait']);
  assert.equal(slowBefore.restricted, true);
  const slow = run([...incident, 'wait', 'wait', 'wait', 'visit']);
  assert.equal(fast.time, 4);
  assert.equal(slow.time, 6);
  for (const s of [fast, slow]) {
    assert.equal(s.restricted, false);
    assert.deepEqual(s.refusals, [{ time: 3, cause: 1 }]);
    assert.match(s.dialogue, /Welcome back/);
    assert.match(s.dialogue, /remember refusing you earlier/);
    assert.equal(s.trace.filter(t => t.outcome === 'Service restored').length, 1);
  }
  assert.equal(base.restricted, true, 'transitions do not mutate previous state');
});

test('duplicate effect actions are idempotent; questions and visits never advance clock', () => {
  let s = run(['delay']);
  assert.deepEqual(act(s, 'delay'), s);
  s = act(s, 'release');
  assert.deepEqual(act(s, 'release'), s);
  s = act(s, 'evidence');
  assert.deepEqual(act(s, 'evidence'), s);
  s = act(s, 'question');
  assert.deepEqual(act(s, 'question'), s);
  assert.equal(act(s, 'visit').time, 0);
  assert.equal(s.sent.length, 3);
  assert.deepEqual(act(start(), 'release'), start());
  assert.deepEqual(act(start(), 'evidence'), start());
});

test('equal-time delivery uses message IDs; corrected state survives old account arrival', () => {
  const actions = ['delay', 'release', 'wait', 'wait', 'wait'];
  const s = run(actions);
  assert.deepEqual(s.received.map(m => m.id), [1, 2]);
  assert.deepEqual(s.trace.filter(t => t.outcome.startsWith('Service ')).map(t => t.outcome), ['Service restricted', 'Service restored']);
  assert.equal(s.restricted, false);
  assert.deepEqual(run(actions), s);
  const earlyEvidence = run(['delay', 'release', 'evidence', 'wait', 'wait', 'wait']);
  assert.deepEqual(earlyEvidence.received.map(m => m.id), [3, 1, 2]);
  assert.equal(earlyEvidence.restricted, false);
  assert.equal(earlyEvidence.trace.some(t => t.outcome === 'Service restricted'), false);
});

test('trace records executed outcomes with causes; no restoration at send time', () => {
  const s = run([...incident, 'evidence']);
  assert.equal(s.trace.some(t => t.outcome === 'Service restored'), false);
  assert.equal(s.trace.filter(t => t.outcome === 'Loading request refused').length, 1);
  assert.ok(s.trace.every(t => t.cause && t.rule && Number.isInteger(t.time)));
});

test('restart resets complete state and player view; invalid actions rejected', () => {
  const s = run([...incident, 'evidence', 'wait', 'visit', 'restart']);
  assert.deepEqual(s, start());
  assert.deepEqual(playerView(s), playerView(start()));
  assert.throws(() => act(s, 'teleport'), /Unknown action/);
});

test('ordinary host has no trace element or developer link', () => {
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  assert.doesNotMatch(html, /developer|trace/i);
});
