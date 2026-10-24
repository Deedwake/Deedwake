import { ACTIONS, loadConvoy } from './convoy.ts';
import type { Action, Convoy } from './convoy.ts';
import './style.css';

const root = document.querySelector('main');
if (!root) throw new Error('The page requires a main element.');
root.innerHTML = `<header><p class="eyebrow">DEEDWAKE · THE GATE RECEIPT</p><h1>An incomplete account.</h1><p class="intro">One convoy. Three people. A chance to set the record right.</p></header>
<div class="scene" role="img" aria-label="Mara watches the gate. Sela travels the convoy road. Ivo runs the depot."><span><b>01 / GATE</b><i aria-hidden="true">○<br>╱│╲<br>╱ ╲</i>Mara</span><span><b>02 / ROAD</b><i aria-hidden="true">▱ ▱ ▱<br>─────────<br>○</i>Sela</span><span><b>03 / DEPOT</b><i aria-hidden="true">⌂<br>○<br>╱│╲</i>Ivo</span></div>
<div class="time"><p id="clock">Fictional time: loading…</p><p class="hint">Only “Advance fictional time” moves the clock. Reading and other choices take no time.</p></div>
<p id="status" role="status" aria-live="polite">Loading the Rust simulation…</p>
<section aria-labelledby="encounter-heading"><h2 id="encounter-heading">Your encounter</h2><p id="response" aria-live="polite" aria-atomic="true"></p></section>
<nav id="choices" aria-label="Your choices"></nav>
<section aria-labelledby="journal-heading"><h2 id="journal-heading">Your journal</h2><p class="hint">Only things you have done or learned. Earlier entries remain a record of that moment.</p><ul id="journal"></ul><p id="empty-journal" class="hint">No incident recorded yet.</p></section>
<footer><button id="restart" class="secondary" disabled>Restart fixture</button><p class="hint">Restart clears this session. Nothing is saved or sent outside this page.</p></footer>`;

function element<T extends HTMLElement>(selector: string): T {
  const found = document.querySelector<T>(selector);
  if (!found) throw new Error(`The page requires ${selector}.`);
  return found;
}

const clock = element('#clock');
const response = element('#response');
const status = element('#status');
const journal = element('#journal');
const emptyJournal = element('#empty-journal');
const restart = element<HTMLButtonElement>('#restart');
const developer = document.body.dataset.developer === 'true';
let trace: HTMLPreElement | undefined;
if (developer) {
  const section = document.createElement('aside');
  section.className = 'developer';
  const title = document.createElement('h2');
  title.textContent = 'Developer fixture · read-only causal trace';
  const hint = document.createElement('p');
  hint.textContent = 'Executed outcomes and causes. This page exposes information outside the player’s perspective; use the ordinary page for playtests.';
  trace = document.createElement('pre');
  trace.setAttribute('aria-label', 'Read-only developer trace');
  section.append(title, hint, trace);
  root.after(section);
}

let convoy: Convoy | undefined;
const buttons = ACTIONS.map(action => {
  const button = document.createElement('button');
  button.type = 'button';
  button.textContent = action.label;
  button.disabled = true;
  button.addEventListener('click', () => choose(action.id, button));
  return button;
});
element('#choices').replaceChildren(...buttons);

function fail(error: unknown): void {
  status.setAttribute('role', 'alert');
  status.textContent = `Simulation unavailable. ${error instanceof Error ? error.message : String(error)} Reload the page to try again.`;
  for (const button of [...buttons, restart]) button.disabled = true;
}

function render(): void {
  if (!convoy) return;
  const view = convoy.snapshot();
  const developerText = developer ? convoy.developerTrace() : undefined;
  clock.textContent = `Fictional time: hour ${view.time}`;
  response.textContent = view.dialogue;
  view.choices.forEach((choice, index) => { buttons[index]!.disabled = !choice.enabled; });
  journal.replaceChildren(...view.journal.map(fact => {
    const item = document.createElement('li');
    item.textContent = fact;
    return item;
  }));
  emptyJournal.hidden = view.journal.length > 0;
  restart.disabled = false;
  if (trace && developerText !== undefined) trace.textContent = developerText;
}

function choose(action: Action, button: HTMLButtonElement): void {
  if (!convoy) return;
  try {
    const result = convoy.act(action);
    render();
    status.textContent = result === 2 ? 'The fictional time limit has been reached. You can restart the fixture.' : result === 1 ? 'That choice is currently unavailable.' : '';
    if (button.disabled) (buttons.find(candidate => !candidate.disabled) ?? restart).focus();
  } catch (error) {
    fail(error);
  }
}

restart.addEventListener('click', () => choose(0, restart));
try {
  convoy = await loadConvoy(new URL('/convoy.wasm', window.location.href));
  render();
  status.textContent = '';
} catch (error) {
  fail(error);
}
