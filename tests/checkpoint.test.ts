// @ts-ignore Node built-ins are provided by the test runtime; no Node typings dependency.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { CheckpointSession, loadCheckpointConvoy } from '../web/checkpoint.ts';
import { Convoy, loadConvoy } from '../web/convoy.ts';
import { Repair, loadRepair } from '../web/repair.ts';
import type { Action, ConvoyCondition } from '../web/convoy.ts';

const bytes = new Uint8Array(await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_example_web.wasm', import.meta.url)));
const referenceModule = await WebAssembly.compile(bytes);
const conditions = ['current', 'control'] as const;
type Player = Convoy | CheckpointSession;
function run(player: Player, actions: readonly Action[]) {
  for (const action of actions) assert.equal(player.act(action), 0, `action ${action}`);
  return player.snapshot();
}
async function fresh(condition?: ConvoyCondition): Promise<Convoy> {
  const instance = await WebAssembly.instantiate(referenceModule, {});
  return new Convoy(instance, condition);
}
function equal(actual: Player, expected: Player) {
  assert.deepEqual(actual.snapshot(), expected.snapshot());
  assert.equal(actual.developerTrace(), expected.developerTrace());
}
function blob(value: unknown): Blob {
  return new Blob([JSON.stringify(value)]);
}
function saved(session: CheckpointSession) {
  return JSON.parse(session.saveCheckpoint());
}

test('all four actual Wasm routes replay every prefix, repeated restore and continued decisions exactly', async () => {
  for (const condition of conditions) {
    for (const intermediary of [false, true]) {
      const route: Action[] = [1, 2, 2, 5, 5, 6, 6, 6, 5, 5, 3];
      if (intermediary) route.push(4);
      const arrival = condition === 'current' && intermediary ? 4 : 6;
      for (let hour = 4; hour <= arrival; hour += 1) route.push(6, 5, 2, 5);
      // Verify a late intermediary account cannot rewrite the confirmed receipt cause.
      if (!intermediary) route.push(4, 6, 5);
      for (let prefix = 0; prefix <= route.length; prefix += 1) {
        const session = await CheckpointSession.fromBytes(bytes, condition);
        const reference = await fresh(condition);
        run(session, route.slice(0, prefix));
        run(reference, route.slice(0, prefix));
        const file = new Blob([session.saveCheckpoint()]);
        run(session, [2, 5]);
        for (let repeat = 0; repeat < 2; repeat += 1) {
          await session.loadCheckpoint(file);
          equal(session, reference);
        }
        for (const action of route.slice(prefix)) {
          run(session, [action]);
          run(reference, [action]);
          equal(session, reference);
        }
        assert.match(session.snapshot().dialogue, /Service is restored.*remember refusing you/);
        assert.ok(session.snapshot().journal.some(entry => /Service is restricted/.test(entry)));
      }
    }
  }
});

test('replay retains repeated questions/visits, trace eviction, no-decision states and honest refusal memory', async () => {
  for (const condition of conditions) {
    const paths: Action[][] = [
      [2, 5, 5, 1, 3, 6, 6, 6, 5], // Report and receipt tie: no restriction decision.
      [1, 6, 6, 6, 3, 4, 6, 6, 6, 5], // Restriction happened without an encounter refusal.
      [1, 6, 6, 6, 5, 3, 4, ...Array<Action>(180).fill(2), 6, 6, 6, 5],
    ];
    for (const [index, actions] of paths.entries()) {
      const reference = await fresh(condition);
      const session = await CheckpointSession.fromBytes(bytes, condition);
      run(reference, actions);
      run(session, actions);
      assert.deepEqual(saved(session).actions, actions);
      await session.loadCheckpoint(new Blob([session.saveCheckpoint()]));
      equal(session, reference);
      if (index === 0) {
        assert.equal(session.snapshot().dialogue, 'Ivo: Loading service is available.');
        assert.doesNotMatch(session.developerTrace(), /request: "(?:restrict|restore) loading"/);
      }
      if (index < 2) assert.doesNotMatch(session.snapshot().dialogue, /remember refusing/);
      if (index === 2) assert.match(session.developerTrace(), /Dropped entries: [1-9]/);
      run(session, [5, 2, 5]);
      run(reference, [5, 2, 5]);
      equal(session, reference);
    }
  }
});

test('default presentation survives loading explicit-current files without changing the selected condition', async () => {
  const legacy = await CheckpointSession.fromBytes(bytes);
  const explicit = await CheckpointSession.fromBytes(bytes, 'current');
  run(explicit, [1, 6, 5]);
  await legacy.loadCheckpoint(new Blob([explicit.saveCheckpoint()]));
  const reference = await fresh();
  run(reference, [1, 6, 5]);
  equal(legacy, reference);
  assert.equal(saved(legacy).condition, 'current');
  assert.equal(legacy.snapshot().choices[2]!.label, 'Release convoy · take receipt');
  assert.match(explicit.snapshot().choices[2]!.label, /routine pouch/);
  run(legacy, [0]);
  equal(legacy, await fresh());
});

test('invalid files and unsuccessful sequences reject atomically without changing output, trace or log', async () => {
  const session = await CheckpointSession.fromBytes(bytes, 'control');
  run(session, [1, 2, 6, 5]);
  const base = saved(session);
  const before = session.snapshot();
  const trace = session.developerTrace();
  const checkpoint = session.saveCheckpoint();
  const malformed: Blob[] = [
    new Blob(['{']), new Blob(['{} trailing']), new Blob([new Uint8Array([0xc3, 0x28])]),
    new Blob(['\uFEFF' + checkpoint]), new Blob([' '.repeat(65537)]),
    ...[null, [], true, 1, 'checkpoint', {}, { ...base, extra: true },
      { ...base, magic: 'other' }, { ...base, schema: 2 }, { ...base, schema: '1' },
      { ...base, exactWasmSHA256: base.exactWasmSHA256.toUpperCase() },
      { ...base, exactWasmSHA256: '0'.repeat(64) }, { ...base, exactWasmSHA256: null },
      { ...base, condition: 'current' }, { ...base, condition: null },
      { ...base, actions: {} }, { ...base, actions: null }, { ...base, actions: [null] },
      { ...base, actions: [true] }, { ...base, actions: ['1'] }, { ...base, actions: [1.5] },
      { ...base, actions: [0] }, { ...base, actions: [7] }, { ...base, actions: [-1] },
      { ...base, actions: Array(4097).fill(2) },
      { ...base, actions: [3] }, { ...base, actions: [1, 1] },
      { ...base, actions: Array(1001).fill(6) },
    ].map(blob),
  ];
  for (const key of Object.keys(base)) {
    const missing = { ...base };
    delete missing[key];
    malformed.push(blob(missing));
  }
  for (const file of malformed) {
    await assert.rejects(session.loadCheckpoint(file));
    assert.equal(session.busy, false);
    assert.equal(session.canSave, true);
    assert.deepEqual(session.snapshot(), before);
    assert.equal(session.developerTrace(), trace);
    assert.equal(session.saveCheckpoint(), checkpoint);
  }
  run(session, [6, 5]);
  const reference = await fresh('control');
  run(reference, [1, 2, 6, 5, 6, 5]);
  equal(session, reference);
});

test('file size is checked before reading and actual bytes are also bounded', async () => {
  const session = await CheckpointSession.fromBytes(bytes);
  let reads = 0;
  class Oversize extends Blob {
    override async arrayBuffer(): Promise<ArrayBuffer> { reads += 1; return super.arrayBuffer(); }
  }
  await assert.rejects(session.loadCheckpoint(new Oversize(['x'.repeat(65537)])), /64 KiB/);
  assert.equal(reads, 0);
  class Understated extends Oversize { override get size(): number { return 1; } }
  await assert.rejects(session.loadCheckpoint(new Understated(['x'.repeat(65537)])), /64 KiB/);
  assert.equal(reads, 1);
  const exact = session.saveCheckpoint().padEnd(65536, ' ');
  await session.loadCheckpoint(new Blob([exact]));
  assert.equal(session.busy, false);
});

test('4096 commands remain replayable; 4097 keeps playing but disables saving until restart or load', async () => {
  const session = await CheckpointSession.fromBytes(bytes);
  const reference = await fresh();
  const actions: Action[] = [1, ...Array<Action>(4095).fill(2)];
  run(session, actions);
  run(reference, actions);
  const last = session.saveCheckpoint();
  assert.equal(saved(session).actions.length, 4096);
  assert.ok(new TextEncoder().encode(last).byteLength <= 65536);
  await session.loadCheckpoint(new Blob([last]));
  equal(session, reference);
  assert.equal(session.act(1), 1);
  assert.equal(session.saveCheckpoint(), last);
  assert.throws(() => session.act(99 as Action), /Invalid Wasm action/);
  assert.equal(session.saveCheckpoint(), last);
  run(session, [2, 5]);
  run(reference, [2, 5]);
  equal(session, reference);
  assert.equal(session.canSave, false);
  assert.match(session.checkpointMessage, /4096.*Restart/);
  assert.throws(() => session.saveCheckpoint(), /4096/);
  const overflowView = session.snapshot();
  const overflowTrace = session.developerTrace();
  await assert.rejects(session.loadCheckpoint(new Blob(['{'])));
  assert.equal(session.canSave, false);
  assert.deepEqual(session.snapshot(), overflowView);
  assert.equal(session.developerTrace(), overflowTrace);
  await session.loadCheckpoint(new Blob([last]));
  assert.equal(session.canSave, true);
  assert.equal(session.saveCheckpoint(), last);
  run(session, [2, 0]);
  assert.equal(session.canSave, true);
  assert.deepEqual(saved(session).actions, []);
  equal(session, await fresh());
  run(session, Array<Action>(1000).fill(6));
  const limit = session.saveCheckpoint();
  assert.equal(session.act(6), 2);
  assert.equal(session.saveCheckpoint(), limit);
  run(session, [0]);
  assert.deepEqual(saved(session).actions, []);
});

test('load synchronously locks actions, restart and overlapping loads; errors unlock and preserve the session', async () => {
  const session = await CheckpointSession.fromBytes(bytes);
  run(session, [1, 2, 6]);
  const before = session.snapshot();
  const trace = session.developerTrace();
  const checkpoint = session.saveCheckpoint();
  for (const fail of [true, false]) {
    let release!: (bytes: ArrayBuffer) => void;
    class Deferred extends Blob {
      override arrayBuffer(): Promise<ArrayBuffer> { return new Promise(resolve => { release = resolve; }); }
    }
    const pending = session.loadCheckpoint(new Deferred(['x']));
    assert.equal(session.busy, true);
    assert.equal(session.canSave, false);
    assert.match(session.checkpointMessage, /Loading/);
    assert.deepEqual(session.snapshot(), before);
    for (const action of [0, 2, 6] as const) assert.throws(() => session.act(action), /loading/);
    assert.throws(() => session.saveCheckpoint(), /Loading/);
    await assert.rejects(session.loadCheckpoint(new Blob([checkpoint])), /already loading/);
    release(new TextEncoder().encode(fail ? '{' : checkpoint).buffer);
    if (fail) await assert.rejects(pending);
    else await pending;
    assert.equal(session.busy, false);
    assert.equal(session.canSave, true);
    assert.deepEqual(session.snapshot(), before);
    assert.equal(session.developerTrace(), trace);
    assert.equal(session.saveCheckpoint(), checkpoint);
  }
  run(session, [6]);
  assert.equal(session.snapshot().time, before.time + 1);
});

test('fresh candidate output is validated before replacing the active instance', async () => {
  const session = await CheckpointSession.fromBytes(bytes);
  run(session, [1, 2]);
  const before = session.snapshot();
  const trace = session.developerTrace();
  const checkpoint = session.saveCheckpoint();
  const instantiate = WebAssembly.instantiate;
  const mock = test.mock.method(WebAssembly, 'instantiate', async (module: WebAssembly.Module, imports: WebAssembly.Imports) => {
    const instance = await instantiate(module, imports);
    return { exports: { ...instance.exports, dw_len: (_field: number) => 1024 * 1024 + 1 } } as WebAssembly.Instance;
  });
  try {
    await assert.rejects(session.loadCheckpoint(new Blob([checkpoint])), /Invalid Wasm text length/);
    assert.deepEqual(session.snapshot(), before);
    assert.equal(session.developerTrace(), trace);
    assert.equal(session.saveCheckpoint(), checkpoint);
  } finally { mock.mock.restore(); }
  await session.loadCheckpoint(new Blob([checkpoint]));
  assert.equal(session.busy, false);
});

test('save is stable metadata plus exact commands and never reads output or developer trace', async () => {
  const instantiate = WebAssembly.instantiate;
  const fields: number[] = [];
  const mock = test.mock.method(WebAssembly, 'instantiate', async (module: WebAssembly.Module, imports: WebAssembly.Imports) => {
    const instance = await instantiate(module, imports);
    const length = instance.exports.dw_len as (field: number) => number;
    return { exports: { ...instance.exports, dw_len: (field: number) => { fields.push(field); return length(field); } } } as WebAssembly.Instance;
  });
  try {
    const session = await CheckpointSession.fromBytes(bytes);
    run(session, [1, 2, 2, 5, 5]);
    fields.length = 0;
    const text = session.saveCheckpoint();
    assert.equal(session.saveCheckpoint(), text);
    assert.deepEqual(fields, []);
    assert.deepEqual(Object.keys(JSON.parse(text)), ['magic', 'schema', 'exactWasmSHA256', 'condition', 'actions']);
    assert.deepEqual(JSON.parse(text).actions, [1, 2, 2, 5, 5]);
    assert.doesNotMatch(text, /dialogue|journal|trace|refusal|restored/i);
    session.snapshot();
    assert.ok(fields.length > 0 && fields.every(field => field === 0 || field === 1));
  } finally { mock.mock.restore(); }
});

test('the exact input byte range is hashed once and retained module restores without fetch, hash or compile', async () => {
  const expected = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), byte => byte.toString(16).padStart(2, '0')).join('');
  const digest = crypto.subtle.digest;
  const compile = WebAssembly.compile;
  let hashes = 0;
  let compiles = 0;
  const hashMock = test.mock.method(crypto.subtle, 'digest', (algorithm: AlgorithmIdentifier, data: BufferSource) => {
    hashes += 1;
    return digest.call(crypto.subtle, algorithm, data);
  });
  const compileMock = test.mock.method(WebAssembly, 'compile', (source: BufferSource) => { compiles += 1; return compile(source); });
  try {
    const padded = new Uint8Array(bytes.length + 8);
    padded.set(bytes, 4);
    const creating = CheckpointSession.fromBytes(padded.subarray(4, padded.length - 4));
    padded.fill(0);
    const session = await creating;
    assert.equal(saved(session).exactWasmSHA256, expected);
    run(session, [1, 2, 6]);
    const file = new Blob([session.saveCheckpoint()]);
    for (let repeat = 0; repeat < 2; repeat += 1) await session.loadCheckpoint(file);
    assert.equal(hashes, 1);
    assert.equal(compiles, 1);
  } finally { hashMock.mock.restore(); compileMock.mock.restore(); }
  const url = URL.createObjectURL(new Blob([bytes]));
  const session = await loadCheckpointConvoy(url);
  URL.revokeObjectURL(url);
  const file = new Blob([session.saveCheckpoint()]);
  await session.loadCheckpoint(file);
  // Valid identical-behavior Wasm with an additional custom section is a foreign artifact.
  const foreign = new Uint8Array([...bytes, 0, 2, 1, 120]);
  const other = await CheckpointSession.fromBytes(foreign);
  await assert.rejects(other.loadCheckpoint(file), /different Wasm build/);
  await assert.rejects(loadCheckpointConvoy('unsupported:convoy.wasm'), /Could not fetch/);
  await assert.rejects(loadCheckpointConvoy('data:application/wasm,invalid'), /Could not start/);
  await assert.rejects(loadCheckpointConvoy('data:application/wasm,invalid', 'other' as ConvoyCondition));
});

test('without Web Crypto ordinary play and plain Convoy/Repair loading remain available', async () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');
  Object.defineProperty(globalThis, 'crypto', { configurable: true, value: undefined });
  try {
    const session = await CheckpointSession.fromBytes(bytes);
    const reference = await fresh();
    run(session, [1, 2, 6, 5]);
    run(reference, [1, 2, 6, 5]);
    equal(session, reference);
    assert.equal(session.canSave, false);
    assert.match(session.checkpointMessage, /Web Crypto/);
    assert.throws(() => session.saveCheckpoint(), /Web Crypto/);
    await assert.rejects(session.loadCheckpoint(new Blob(['{}'])), /Web Crypto/);
    assert.equal(session.busy, false);
    run(session, [6, 0]);
    const url = URL.createObjectURL(new Blob([bytes]));
    try {
      const plain = await loadConvoy(url);
      assert.ok(plain instanceof Convoy);
      equal(plain, await fresh());
    } finally { URL.revokeObjectURL(url); }
    const repairBytes = new Uint8Array(await readFile(new URL('../target/wasm32-unknown-unknown/release/deedwake_repair_web.wasm', import.meta.url)));
    const repairUrl = URL.createObjectURL(new Blob([repairBytes]));
    try {
      const plain = await loadRepair(repairUrl);
      const module = await WebAssembly.compile(repairBytes);
      const reference = new Repair(await WebAssembly.instantiate(module, {}));
      for (const action of [1, 6, 6, 2, 3, 5, 6, 6, 3] as const) {
        assert.equal(plain.act(action), reference.act(action));
        assert.deepEqual(plain.snapshot(), reference.snapshot());
        assert.equal(plain.developerTrace(), reference.developerTrace());
        assert.deepEqual(plain.developerInspection(), reference.developerInspection());
      }
      assert.ok(plain instanceof Repair);
    } finally { URL.revokeObjectURL(repairUrl); }
  } finally {
    if (descriptor) Object.defineProperty(globalThis, 'crypto', descriptor);
    else Reflect.deleteProperty(globalThis, 'crypto');
  }
});
