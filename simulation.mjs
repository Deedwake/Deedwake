const ACCOUNT = 'I saw you hold the convoy at the gate. I could not see why, or what happened afterward.';
const freeze = value => {
  if (value && typeof value === 'object') {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
};
const record = (s, outcome, cause, rule) => s.trace.push({ time: s.time, outcome, cause, rule });
const learn = (s, fact) => { if (!s.journal.includes(fact)) s.journal.push(fact); };
const send = (s, kind, content, delay, cause) => {
  const message = { id: s.sent.length + 1, kind, content, sentAt: s.time, due: s.time + delay, route: 'gate–depot pouch' };
  s.sent.push(message);
  s.pending.push(message.id);
  record(s, `Sent ${kind} #${message.id}, due ${message.due}`, cause, 'Fixed contents; delivery by due time then message ID');
};
export function start() {
  return freeze({ time: 0, truth: { delayed: false, released: false, reason: 'You check a loose load strap.' },
    observation: null, retained: null, sent: [], pending: [], received: [], restricted: false,
    evidenceSent: false, refusals: [], visits: 0, journal: [], trace: [],
    dialogue: 'At the gate, a load strap looks loose. You can hold the convoy to check it. Mara watches from across the road. Ivo runs the depot; Sela is a trusted convoy intermediary.' });
}
export function act(previous, action) {
  if (action === 'restart') return start();
  if (!['delay', 'question', 'release', 'evidence', 'visit', 'wait'].includes(action)) throw new Error('Unknown action');
  const s = structuredClone(previous);
  switch (action) {
    case 'delay':
      if (s.truth.delayed) break;
      s.truth.delayed = true;
      s.observation = { saw: 'Player held convoy at gate', omitted: 'Reason and later release', certainty: 'Limited view' };
      s.retained = ACCOUNT;
      send(s, 'account', s.retained, 3, 'Mara witnessed the hold');
      learn(s, 'You held the convoy to check a loose strap.');
      s.dialogue = 'You hold the convoy and secure the strap. Mara leaves a note in the gate–depot pouch. Delivery takes three fictional hours. Reading and choosing do not advance time.';
      record(s, 'Convoy held', 'Player delay choice', 'One incident per fixture');
      break;
    case 'question':
      s.dialogue = s.retained ? `Mara: “${s.retained}” My note travels to Ivo in the gate–depot pouch.` : 'Mara: “I have not seen an incident here.”';
      if (s.retained) learn(s, `Mara’s testimony: ${s.retained}`);
      break;
    case 'release':
      if (!s.truth.delayed || s.truth.released) break;
      s.truth.released = true;
      send(s, 'correction', 'The convoy has been released after a strap check.', 3, 'Gate release receipt');
      learn(s, 'Release receipt: convoy released after a strap check.');
      s.dialogue = 'You release the convoy and receive a gate receipt. A correction enters the same pouch route, due in three fictional hours. Earlier reports and refusals remain history. Sela can send your receipt by the next priority pouch in one hour.';
      record(s, 'Convoy released', 'Player release choice', 'Release creates a new correction');
      break;
    case 'evidence':
      if (!s.truth.released || s.evidenceSent) break;
      s.evidenceSent = true;
      send(s, 'evidence', 'Sela verified the gate receipt: convoy released after a strap check.', 1, 'Player gave learned receipt to Sela');
      learn(s, 'Sela sent your verified receipt by priority pouch; arrival in one hour.');
      s.dialogue = 'Sela: “I checked your receipt and sent it along the gate–depot route by priority pouch. Ivo accepts my verified evidence. It still needs one fictional hour to reach him.”';
      break;
    case 'wait':
      s.time += 1;
      for (const message of s.sent.filter(m => s.pending.includes(m.id) && m.due <= s.time).sort((a, b) => a.due - b.due || a.id - b.id)) {
        s.pending = s.pending.filter(id => id !== message.id);
        s.received.push(message);
        record(s, `Ivo received ${message.kind} #${message.id}`, `Delivery #${message.id}`, 'Only arrived information can affect access');
        const wasRestricted = s.restricted;
        s.restricted = s.received.some(m => m.kind === 'account') && !s.received.some(m => m.kind === 'correction' || m.kind === 'evidence');
        if (s.restricted !== wasRestricted) record(s, s.restricted ? 'Service restricted' : 'Service restored', `Received #${message.id}`, 'Hold report restricts; gate correction or Sela’s verified receipt restores');
      }
      s.dialogue = 'One fictional hour passes. Return to Ivo to learn his current response.';
      break;
    case 'visit': {
      s.visits += 1;
      const returning = s.visits > 1 ? 'Ivo: “Welcome back. ' : 'Ivo: “';
      if (s.restricted) {
        s.refusals.push({ time: s.time, cause: s.received.find(m => m.kind === 'account').id });
        s.dialogue = `${returning}Mara’s note says you held the convoy, but does not say why. I’m temporarily withholding loading service. A gate correction or Sela’s verified release receipt will restore it.”`;
        learn(s, 'Ivo received Mara’s incomplete report and refused loading service. He accepts a gate correction or Sela’s verified release receipt.');
        record(s, 'Loading request refused', 'Player visited while restriction active', 'Service restriction enforced');
      } else if (s.received.some(m => m.kind !== 'account')) {
        s.dialogue = `${returning}I received confirmation that you released the convoy after a strap check. Loading service is available.${s.refusals.length ? ' I remember refusing you earlier; that delay still happened. Thank you for clearing this up.' : ' Thank you for clearing this up.'}”`;
        learn(s, 'Ivo received release confirmation. Loading service is available.');
        record(s, 'Loading service offered after confirmation', 'Player returned after received correction/evidence', 'Recognize shared history without erasing it');
      } else {
        s.dialogue = `${returning}Loading service is available. I have received no report requiring a restriction.”`;
        learn(s, 'At your visit, Ivo had received no report requiring a restriction.');
        record(s, 'Loading service offered', 'Player visited before any restricting report', 'No reaction to unseen facts');
      }
      break;
    }
  }
  return freeze(s);
}
// Player projection deliberately excludes queue, received facts, policy state and causal trace.
export function playerView(s) {
  return { time: s.time, dialogue: s.dialogue, journal: [...s.journal],
    choices: [
      { action: 'delay', label: 'Hold convoy · check strap', enabled: !s.truth.delayed },
      { action: 'question', label: 'Ask Mara what she saw', enabled: true },
      { action: 'release', label: 'Release convoy · take receipt', enabled: s.truth.delayed && !s.truth.released },
      { action: 'evidence', label: 'Give receipt to Sela · priority pouch', enabled: s.truth.released && !s.evidenceSent },
      { action: 'visit', label: 'Visit / return to Ivo at depot', enabled: true },
      { action: 'wait', label: 'Advance fictional time · 1 hour', enabled: true }
    ] };
}
