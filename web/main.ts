import { ACTIONS as CONVOY_ACTIONS, loadConvoy } from './convoy.ts';
import { ACTIONS as REPAIR_ACTIONS, Repair, loadRepair } from './repair.ts';
import { renderRepairInspection } from './inspection.ts';
import { DEVELOPER_CASES, renderDeveloperCheck } from './checks.ts';
import type { Action, Fixture } from './fixture.ts';
import './style.css';

const root = document.querySelector('main');
if (!root) throw new Error('The page requires a main element.');
const repair = document.body.dataset.example === 'repair';
const actions = repair ? REPAIR_ACTIONS : CONVOY_ACTIONS;
const scene = repair
  ? '<div class="scene" role="img" aria-label="Intake receives reports. The technician inspects and repairs. Collection receives ready notices."><span><b>01 / INTAKE</b><i aria-hidden="true">▤<br>↓<br>○</i>Reports &amp; questions</span><span><b>02 / WORKBENCH</b><i aria-hidden="true">♧<br>─────────<br>⚒</i>Inspection &amp; repair</span><span><b>03 / COLLECTION</b><i aria-hidden="true">▣<br>↓<br>⌂</i>Ready notices</span></div>'
  : '<div class="scene" role="img" aria-label="Mara watches the gate. Sela travels the convoy road. Ivo runs the depot."><span><b>01 / GATE</b><i aria-hidden="true">○<br>╱│╲<br>╱ ╲</i>Mara</span><span><b>02 / ROAD</b><i aria-hidden="true">▱ ▱ ▱<br>─────────<br>○</i>Sela</span><span><b>03 / DEPOT</b><i aria-hidden="true">⌂<br>○<br>╱│╲</i>Ivo</span></div>';
root.innerHTML = `<header><p class="eyebrow">DEEDWAKE · ${repair ? 'THE REPAIR CAFÉ' : 'THE GATE RECEIPT'}</p><h1>${repair ? 'A second chance.' : 'An incomplete account.'}</h1><p class="intro">${repair ? 'One donated appliance. Three desks. Reports travel before decisions change.' : 'One convoy. Three people. A chance to set the record right.'}</p><a class="example-link" href="${repair ? '/' : '/repair.html'}">${repair ? 'Play the gate receipt' : 'Visit the repair café'} →</a></header>
${scene}
<div class="time"><p id="clock">Fictional time: loading…</p><p class="hint">Only “Advance fictional time” moves the clock. Reading and other choices take no time.</p></div>
<p id="status" role="status" aria-live="polite">Loading the Rust simulation…</p>
<section aria-labelledby="encounter-heading"><h2 id="encounter-heading">Your encounter</h2><p id="response" aria-live="polite" aria-atomic="true"></p></section>
<nav id="choices" aria-label="Your choices"></nav>
<section aria-labelledby="journal-heading"><h2 id="journal-heading">Your journal</h2><p class="hint">Only things you have done or learned. Earlier entries remain a record of that moment.</p><ul id="journal"></ul><p id="empty-journal" class="hint">${repair ? 'No appliance report recorded yet.' : 'No incident recorded yet.'}</p></section>
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
let inspection: HTMLDivElement | undefined;
let checkRun: HTMLButtonElement | undefined;
if (developer) {
  const section = document.createElement('aside');
  section.className = 'developer';
  const title = document.createElement('h2');
  title.textContent = repair ? 'Developer fixture · inspection and case checks' : 'Developer fixture · read-only causal trace';
  const hint = document.createElement('p');
  hint.textContent = `${repair ? 'Prospective decisions, actual action history, and causal trace.' : 'Executed outcomes and causes.'} This page exposes information outside the player’s perspective; use the ordinary page for playtests.`;
  if (repair) {
    inspection = document.createElement('div');
    section.append(title, hint, inspection);
    const traceTitle = document.createElement('h3');
    traceTitle.textContent = 'Read-only causal trace';
    const checks = document.createElement('section');
    checks.setAttribute('aria-labelledby', 'developer-checks-title');
    const checkTitle = document.createElement('h3');
    checkTitle.id = 'developer-checks-title';
    checkTitle.textContent = 'Run a developer case';
    const explanation = document.createElement('p');
    explanation.className = 'hint';
    explanation.textContent = 'Run one of five existing Rust cases using the compiled configuration in a fresh, separate validation fixture. The interactive encounter, journal, time, inspection and trace are untouched. No configuration is edited.';
    const label = document.createElement('label');
    label.htmlFor = 'developer-check-case';
    label.textContent = 'Developer case';
    const select = document.createElement('select');
    select.id = label.htmlFor;
    for (const choice of DEVELOPER_CASES) {
      const option = document.createElement('option');
      option.value = String(choice.id);
      option.textContent = choice.label;
      select.append(option);
    }
    checkRun = document.createElement('button');
    checkRun.type = 'button';
    checkRun.textContent = 'Run check';
    checkRun.disabled = true;
    const result = document.createElement('div');
    result.setAttribute('aria-live', 'polite');
    checkRun.addEventListener('click', () => {
      if (!(fixture instanceof Repair)) return;
      try {
        const report = fixture.runDeveloperCheck(Number(select.value));
        renderDeveloperCheck(result, report, fixture.developerInspection().configuration);
      } catch (error) {
        const failure = document.createElement('p');
        failure.setAttribute('role', 'alert');
        failure.textContent = `Developer check unavailable. ${error instanceof Error ? error.message : String(error)}`;
        result.replaceChildren(failure);
      }
    });
    checks.append(checkTitle, explanation, label, select, checkRun, result);
    section.append(checks, traceTitle);
  } else {
    section.append(title, hint);
  }
  trace = document.createElement('pre');
  trace.setAttribute('aria-label', 'Read-only developer trace');
  section.append(trace);
  root.after(section);
}

let fixture: Fixture | undefined;
const buttons = actions.map(action => {
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
  if (!fixture) return;
  const view = fixture.snapshot();
  const developerText = developer ? fixture.developerTrace() : undefined;
  const developerInspection = inspection && fixture instanceof Repair ? fixture.developerInspection() : undefined;
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
  if (inspection && developerInspection) renderRepairInspection(inspection, developerInspection);
}

function choose(action: Action, button: HTMLButtonElement): void {
  if (!fixture) return;
  try {
    const result = fixture.act(action);
    render();
    status.textContent = result === 2 ? 'The fictional time limit has been reached. You can restart the fixture.' : result === 1 ? 'That choice is currently unavailable.' : '';
    if (button.disabled) (buttons.find(candidate => !candidate.disabled) ?? restart).focus();
  } catch (error) {
    fail(error);
  }
}

restart.addEventListener('click', () => choose(0, restart));
try {
  fixture = repair
    ? await loadRepair(new URL('/repair.wasm', window.location.href))
    : await loadConvoy(new URL('/convoy.wasm', window.location.href));
  render();
  if (checkRun) checkRun.disabled = false;
  status.textContent = '';
} catch (error) {
  fail(error);
}
