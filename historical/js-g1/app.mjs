import { start, act, playerView } from './simulation.mjs';
let state = start();
const root = document.querySelector('main');
root.innerHTML = `<header><p class="eyebrow">DEEDWAKE · LOCAL G1 PROOF</p><h1>The gate receipt</h1><p>One convoy. An incomplete account. A chance to set it right.</p></header>
<div class="scene" role="img" aria-label="Gate with Mara, road with Sela, depot with Ivo"><span>☀<br>○<br>╱│╲<br>╱ ╲<br>Mara · gate</span><span>▱ ▱ ▱<br>── convoy road ──<br>○<br>╱│╲<br>Sela · intermediary</span><span>⌂<br>○<br>╱│╲<br>╱ ╲<br>Ivo · depot</span></div>
<p id="clock"></p><p class="hint">Only “Advance fictional time” moves the clock. Reading and other choices take no time.</p>
<section aria-labelledby="response-heading"><h2 id="response-heading">Your encounter</h2><p id="response" aria-live="polite"></p></section>
<nav id="choices" aria-label="Your choices"></nav><section><h2>Your journal</h2><p>Only things you have done or learned. Earlier entries remain a record of that moment.</p><ul id="journal"></ul></section>
<button id="restart" class="secondary">Restart fixture</button><p class="hint">Restart clears this session. Nothing is saved or sent outside this page.</p>`;
const buttons = playerView(state).choices.map(choice => {
  const button = document.createElement('button');
  button.textContent = choice.label;
  button.addEventListener('click', () => {
    state = act(state, choice.action);
    render();
    if (button.disabled) buttons.find(candidate => !candidate.disabled)?.focus();
  });
  return button;
});
document.querySelector('#choices').replaceChildren(...buttons);
function render() {
  const view = playerView(state);
  document.querySelector('#clock').textContent = `Fictional time: hour ${view.time}`;
  document.querySelector('#response').textContent = view.dialogue;
  view.choices.forEach((choice, index) => { buttons[index].disabled = !choice.enabled; });
  document.querySelector('#journal').replaceChildren(...(view.journal.length ? view.journal : ['No incident recorded yet.']).map(fact => {
    const item = document.createElement('li'); item.textContent = fact; return item;
  }));
  const trace = document.querySelector('#developer-trace');
  if (trace) trace.textContent = JSON.stringify(state.trace, null, 2);
}
document.querySelector('#restart').addEventListener('click', () => { state = start(); render(); });
render();
