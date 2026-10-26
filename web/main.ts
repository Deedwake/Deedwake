import { ACTIONS as CONVOY_ACTIONS, loadConvoy } from './convoy.ts';
import { STUDY_CONDITIONS, StudyConvoy, loadStudyConvoy } from './study.ts';
import type { StudyCondition } from './study.ts';
import { CheckpointSession, loadCheckpointConvoy } from './checkpoint.ts';
import { ACTIONS as REPAIR_ACTIONS, Repair, loadRepair } from './repair.ts';
import { renderRepairInspection } from './inspection.ts';
import { DEVELOPER_CASES, renderDeveloperCheck } from './checks.ts';
import type { Action, Fixture } from './fixture.ts';
import './style.css';

const root = document.querySelector('main');
if (!root) throw new Error('The page requires a main element.');
const repair = document.body.dataset.example === 'repair';
const trial = document.body.dataset.trial;
const study = document.body.dataset.study;
const studyPage = study !== undefined;
const studyTrial = document.body.dataset.studyTrial;
if (studyTrial !== undefined && !['A', 'B', 'C', 'D'].includes(studyTrial)) throw new Error('Invalid study trial label.');
const developer = document.body.dataset.developer === 'true';
const checkpointPage = !repair && !trial && !developer && !studyPage;
const actions = repair ? REPAIR_ACTIONS : CONVOY_ACTIONS;
const scene = repair
  ? '<div class="scene" role="img" aria-label="Intake receives reports. The technician inspects and repairs. Collection receives ready notices."><span><b>01 / INTAKE</b><i aria-hidden="true">▤<br>↓<br>○</i>Reports &amp; questions</span><span><b>02 / WORKBENCH</b><i aria-hidden="true">♧<br>─────────<br>⚒</i>Inspection &amp; repair</span><span><b>03 / COLLECTION</b><i aria-hidden="true">▣<br>↓<br>⌂</i>Ready notices</span></div>'
  : '<div class="scene" role="img" aria-label="Mara watches the gate. Sela travels the convoy road. Ivo runs the depot."><span><b>01 / GATE</b><i aria-hidden="true">○<br>╱│╲<br>╱ ╲</i>Mara</span><span><b>02 / ROAD</b><i aria-hidden="true">▱ ▱ ▱<br>─────────<br>○</i>Sela</span><span><b>03 / DEPOT</b><i aria-hidden="true">⌂<br>○<br>╱│╲</i>Ivo</span></div>';
root.innerHTML = `<header><p class="eyebrow">DEEDWAKE · ${repair ? 'THE REPAIR CAFÉ' : 'THE GATE RECEIPT'}${studyTrial ? ` · STUDY ${studyTrial}` : trial === 'A' || trial === 'B' ? ` · TRIAL ${trial}` : ''}</p><h1>${repair ? 'A second chance.' : studyPage ? 'The gate receipt.' : 'An incomplete account.'}</h1><p class="intro">${repair ? 'One donated appliance. Three desks. Reports travel before decisions change.' : 'One convoy. Three people. A chance to set the record right.'}</p>${trial || studyPage ? '' : `<a class="example-link" href="${repair ? '/' : '/repair.html'}">${repair ? 'Play the gate receipt' : 'Visit the repair café'} →</a>`}</header>
${scene}
<div class="time"><p id="clock">Fictional time: loading…</p><p class="hint">Only “Advance fictional time” moves the clock. Reading and other choices take no time.</p></div>
<p id="status" role="status" aria-live="polite">Loading the Rust simulation…</p>
<section aria-labelledby="encounter-heading"><h2 id="encounter-heading">Your encounter</h2><p id="response" aria-live="polite" aria-atomic="true"></p></section>
<nav id="choices" aria-label="Your choices"></nav>
<section aria-labelledby="journal-heading"><h2 id="journal-heading">Your journal</h2><p class="hint">Only things you have done or learned. Earlier entries remain a record of that moment.</p><ul id="journal"></ul><p id="empty-journal" class="hint">${repair ? 'No appliance report recorded yet.' : 'No incident recorded yet.'}</p></section>
${checkpointPage ? '<section aria-labelledby="checkpoint-heading"><h2 id="checkpoint-heading">Save and resume</h2><p class="hint">Download a checkpoint to keep this session. Loading a compatible file replaces your current session. Files stay on your device; nothing is uploaded to a service.</p><button id="save-checkpoint" class="secondary" disabled>Save · download checkpoint</button><p id="checkpoint-limit" class="hint">Checkpoints are loading…</p><label for="checkpoint-file">Choose a checkpoint file</label><input id="checkpoint-file" type="file" accept=".json,application/json" disabled><button id="load-checkpoint" class="secondary" disabled>Load file · replace session</button><p id="checkpoint-status" role="status" aria-live="polite"></p></section>' : ''}
<footer><button id="restart" class="secondary" disabled>Restart fixture</button><p class="hint">${checkpointPage ? 'Restart clears this session. A downloaded checkpoint remains on your device.' : 'Restart clears this session. Nothing is saved or sent outside this page.'}</p></footer>`;

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
const checkpointSave = checkpointPage ? element<HTMLButtonElement>('#save-checkpoint') : undefined;
const checkpointLoad = checkpointPage ? element<HTMLButtonElement>('#load-checkpoint') : undefined;
const checkpointFile = checkpointPage ? element<HTMLInputElement>('#checkpoint-file') : undefined;
const checkpointStatus = checkpointPage ? element('#checkpoint-status') : undefined;
const checkpointLimit = checkpointPage ? element('#checkpoint-limit') : undefined;
let failed = false;
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

let fixture: Fixture | CheckpointSession | undefined;
let studySelect: HTMLSelectElement | undefined;
let studyLoading = false;
if (studyPage && developer) {
  const controls = document.createElement('section');
  const label = document.createElement('label');
  label.htmlFor = 'study-condition';
  label.textContent = 'Compiled study condition';
  studySelect = document.createElement('select');
  studySelect.id = label.htmlFor;
  for (const condition of STUDY_CONDITIONS) {
    const option = document.createElement('option');
    option.value = condition.key;
    option.textContent = condition.label;
    option.selected = condition.key === study;
    studySelect.append(option);
  }
  studySelect.disabled = true;
  const select = studySelect;
  const note = document.createElement('p');
  note.className = 'hint';
  note.textContent = 'Changing the compiled condition creates a fresh session. Ordinary study pages do not show this selector or trace.';
  select.addEventListener('change', async () => {
    if (studyLoading || failed) return;
    studyLoading = true;
    const previous = fixture;
    select.disabled = true;
    for (const button of [...buttons, restart]) button.disabled = true;
    try {
      const candidate = await loadStudyConvoy(new URL('/convoy.wasm', window.location.href), select.value as StudyCondition);
      candidate.snapshot();
      candidate.developerTrace();
      fixture = candidate;
      status.textContent = 'Fresh study session selected.';
      render();
    } catch (error) {
      fixture = previous;
      if (previous instanceof StudyConvoy) select.value = previous.studyConfiguration().key;
      status.textContent = `Study selection failed; previous session preserved. ${error instanceof Error ? error.message : String(error)}`;
      render();
    } finally {
      studyLoading = false;
      select.disabled = failed;
      render();
    }
  });
  controls.append(label, select, note);
  document.querySelector('aside.developer')!.prepend(controls);
}
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
  failed = true;
  status.setAttribute('role', 'alert');
  status.textContent = `Simulation unavailable. ${error instanceof Error ? error.message : String(error)} Reload the page to try again.`;
  for (const button of [...buttons, restart]) button.disabled = true;
  for (const control of [checkpointSave, checkpointLoad, checkpointFile, studySelect]) if (control) control.disabled = true;
}

function render(): void {
  if (!fixture) return;
  const view = fixture.snapshot();
  const checkpoint = fixture instanceof CheckpointSession ? fixture : undefined;
  const developerText = developer ? fixture.developerTrace() : undefined;
  const developerInspection = inspection && fixture instanceof Repair ? fixture.developerInspection() : undefined;
  clock.textContent = `Fictional time: hour ${view.time}`;
  response.textContent = view.dialogue;
  view.choices.forEach((choice, index) => {
    buttons[index]!.textContent = choice.label;
    buttons[index]!.disabled = failed || studyLoading || Boolean(checkpoint?.busy) || !choice.enabled;
  });
  journal.replaceChildren(...view.journal.map(fact => {
    const item = document.createElement('li');
    item.textContent = fact;
    return item;
  }));
  emptyJournal.hidden = view.journal.length > 0;
  restart.disabled = failed || studyLoading || Boolean(checkpoint?.busy);
  if (checkpointSave && checkpointLoad && checkpointFile && checkpointLimit) {
    checkpointSave.disabled = failed || !checkpoint?.canSave;
    checkpointFile.disabled = failed || !checkpoint || checkpoint.busy;
    checkpointLoad.disabled = failed || !checkpoint || checkpoint.busy || checkpointFile.files?.length !== 1;
    checkpointLimit.textContent = checkpoint?.checkpointMessage ?? 'Checkpoints are unavailable.';
  }
  if (trace && developerText !== undefined) trace.textContent = developerText;
  if (inspection && developerInspection) renderRepairInspection(inspection, developerInspection);
}

function choose(action: Action, button: HTMLButtonElement): void {
  if (!fixture || failed || studyLoading || (fixture instanceof CheckpointSession && fixture.busy)) return;
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
checkpointFile?.addEventListener('change', () => render());
checkpointSave?.addEventListener('click', () => {
  if (!(fixture instanceof CheckpointSession) || fixture.busy || failed || !checkpointStatus) return;
  let url: string | undefined;
  try {
    const contents = fixture.saveCheckpoint();
    url = URL.createObjectURL(new Blob([contents], { type: 'application/json' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = 'deedwake-convoy-checkpoint.json';
    document.body.append(link);
    link.click();
    link.remove();
    checkpointStatus.textContent = 'Checkpoint download requested. Keep the file to resume this session in the same build.';
  } catch (error) {
    checkpointStatus.textContent = `Could not save: ${error instanceof Error ? error.message : String(error)}`;
  } finally {
    if (url) { const completed = url; setTimeout(() => URL.revokeObjectURL(completed), 0); }
  }
});
checkpointLoad?.addEventListener('click', async () => {
  if (!(fixture instanceof CheckpointSession) || fixture.busy || failed || !checkpointStatus || !checkpointFile) return;
  const file = checkpointFile.files?.[0];
  if (!file) return;
  const session = fixture;
  const loading = session.loadCheckpoint(file);
  checkpointStatus.textContent = 'Checking the checkpoint. Your current session stays intact until loading succeeds.';
  render();
  try {
    await loading;
    checkpointFile.value = '';
    checkpointStatus.textContent = 'Checkpoint loaded. Continue from the restored fictional time.';
    status.textContent = '';
  } catch (error) {
    checkpointStatus.textContent = `Checkpoint not loaded. Current session preserved. ${error instanceof Error ? error.message : String(error)}`;
  } finally {
    render();
  }
});
try {
  const condition = document.body.dataset.convoyCondition;
  if (condition !== undefined && condition !== 'current' && condition !== 'control') throw new Error('Invalid convoy condition.');
  if (studyPage && !STUDY_CONDITIONS.some(condition => condition.key === study)) throw new Error('Invalid study condition.');
  if (studyPage && (repair || condition !== undefined || trial !== undefined)) throw new Error('Study and legacy scene settings cannot be combined.');
  fixture = studyPage
    ? await loadStudyConvoy(new URL('/convoy.wasm', window.location.href), study as StudyCondition)
    : repair
      ? await loadRepair(new URL('/repair.wasm', window.location.href))
      : checkpointPage
        ? await loadCheckpointConvoy(new URL('/convoy.wasm', window.location.href), condition)
        : await loadConvoy(new URL('/convoy.wasm', window.location.href), condition);
  render();
  if (checkRun) checkRun.disabled = false;
  if (studySelect) studySelect.disabled = false;
  status.textContent = '';
} catch (error) {
  fail(error);
}
