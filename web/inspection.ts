import type { RepairInspection } from './repair.ts';

function table(caption: string, headings: readonly string[], rows: readonly (readonly string[])[]): HTMLDivElement {
  const wrapper = document.createElement('div');
  wrapper.className = 'inspection-table';
  wrapper.tabIndex = 0;
  wrapper.setAttribute('role', 'region');
  wrapper.setAttribute('aria-label', caption);
  const result = document.createElement('table');
  const title = result.createCaption();
  title.textContent = caption;
  const header = result.createTHead().insertRow();
  for (const heading of headings) {
    const cell = document.createElement('th');
    cell.scope = 'col';
    cell.textContent = heading;
    header.append(cell);
  }
  const body = result.createTBody();
  for (const row of rows) {
    const line = body.insertRow();
    row.forEach((value, index) => {
      const cell = index === 0 ? document.createElement('th') : document.createElement('td');
      if (index === 0) cell.scope = 'row';
      cell.textContent = value;
      line.append(cell);
    });
  }
  wrapper.append(result);
  return wrapper;
}

function section(id: string, title: string, hint: string, contents: HTMLElement): HTMLElement {
  const result = document.createElement('section');
  result.setAttribute('aria-labelledby', id);
  const heading = document.createElement('h3');
  heading.id = id;
  heading.textContent = title;
  const explanation = document.createElement('p');
  explanation.className = 'hint';
  explanation.textContent = hint;
  result.append(heading, explanation, contents);
  return result;
}

export function renderRepairInspection(container: HTMLElement, view: RepairInspection): void {
  const dispositions = {
    no_proposal: 'No proposal',
    would_reject: 'Would reject',
    ready_to_execute: 'Prerequisites met (no action executed by inspection)',
  };
  const configuration = section('inspection-config', 'Compiled configuration',
    'Read-only values compiled into this Rust fixture. Timing values are fictional ticks.',
    table('Compiled repair configuration', ['Field', 'Value'], Object.entries(view.configuration).map(([key, value]) => [key, String(value)])));
  const why = section('inspection-why', 'WHY · current prospective decisions',
    `Predictions at tick ${view.time}, returned by Rust. Reading this inspection does not propose or execute an action. Cause times are record creation times, not arrival times.`,
    table('Prospective repair decisions', ['Action', 'Prospective disposition', 'Typed reason', 'Permitted causes'], view.decisions.map(decision => [
      decision.action,
      `${dispositions[decision.disposition]} · ${decision.disposition}`,
      decision.reason,
      decision.causes.map(cause => `${cause.fact} · source: ${cause.source} · recorded at tick ${cause.time}`).join('\n') || 'None',
    ])));
  const history = section('inspection-history', 'Actual action history',
    'Recorded lifecycle statuses from Rust. Earlier host rejections remain here after a later repair succeeds.',
    table('Actual repair action history', ['Action', 'Actual status', 'Rejection reason', 'Confirmed fact'], view.actions.map(action => [action.action, action.status, action.rejection ?? 'None', action.confirmed ?? 'None'])));
  if (view.actions.length === 0) {
    const empty = document.createElement('p');
    empty.textContent = 'No actions proposed yet.';
    history.append(empty);
  }
  const retention = document.createElement('p');
  retention.className = 'hint';
  retention.textContent = `Trace capacity: ${view.configuration.trace_capacity} entries. Dropped trace entries: ${view.droppedTrace}. ${view.droppedTrace > 0 ? 'Trace truncated; earlier entries were dropped.' : 'Trace not truncated; no entries dropped.'} Action history above is separate from trace retention.`;
  container.replaceChildren(configuration, why, history, retention);
}
