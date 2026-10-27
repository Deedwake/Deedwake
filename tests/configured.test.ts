// @ts-ignore Node built-ins are supplied by the test runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFile } from 'node:fs/promises';
import { CapacityError, ConfiguredConvoy } from '../web/configured.ts';
import { CheckpointSession } from '../web/checkpoint.ts';
import type { Action } from '../web/fixture.ts';

const bytes = new Uint8Array(await readFile(new URL('../public/convoy.wasm', import.meta.url)));
const module = await WebAssembly.compile(bytes);
const fresh = async () => new ConfiguredConvoy(await WebAssembly.instantiate(module, {}));
const checkpoint = () => CheckpointSession.fromConfiguredBytes(bytes);
type Player = ConfiguredConvoy | CheckpointSession;
function run(player: Player, actions: readonly Action[]) {
  for (const action of actions) assert.equal(player.act(action), 0, `action ${action}`);
}
function equal(a: Player, b: Player) {
  assert.deepEqual(a.snapshot(), b.snapshot());
  assert.equal(a.developerTrace(), b.developerTrace());
}

test('configured actual Wasm metadata is bounded and labels reflect generated configuration', async () => {
  const scene = await fresh();
  assert.deepEqual(scene.configuration, { profileId: 'configured-convoy-v1', observer: 'Mara', retention: 'incomplete', reaction: 'restrict_until_correction', selaConfidence: 'trusted', initial: 3, routine: 3, intermediary: 1, traceCapacity: 128 });
  assert.match(scene.snapshot().choices[1]!.label, /Ask Mara/);
  assert.match(scene.snapshot().choices[2]!.label, /3 fictional hours/);
  assert.match(scene.snapshot().choices[3]!.label, /1 fictional hour/);
  assert.ok(Object.isFrozen(scene.configuration));
  run(scene, [1, 6, 0]);
  equal(scene, await fresh());
});

test('configured checkpoints replay every prefix and continuation with separate schema/profile identity', async () => {
  for (const intermediary of [true, false]) {
    const path: Action[] = [1, 2, 2, 5, 6, 6, 6, 5, 3];
    if (intermediary) path.push(4);
    path.push(6, 5, 6, 6, 5, 2, 5);
    for (let prefix = 0; prefix <= path.length; prefix += 1) {
      const saved = await checkpoint(), reference = await fresh();
      run(saved, path.slice(0, prefix));
      run(reference, path.slice(0, prefix));
      const json = saved.saveCheckpoint();
      const file = JSON.parse(json);
      assert.equal(file.schema, 2);
      assert.equal(file.profileId, 'configured-convoy-v1');
      assert.equal(Object.hasOwn(file, 'condition'), false);
      run(saved, [2, 5]);
      for (let repeat = 0; repeat < 2; repeat += 1) { await saved.loadCheckpoint(new Blob([json])); equal(saved, reference); }
      for (const action of path.slice(prefix)) { run(saved, [action]); run(reference, [action]); equal(saved, reference); }
      assert.match(saved.snapshot().dialogue, /restored/);
    }
  }
});

test('configured load rejects malformed, duplicate, foreign-profile/build and unsuccessful inputs atomically', async () => {
  const scene = await checkpoint();
  run(scene, [1, 6, 5]);
  const json = scene.saveCheckpoint(), valid = JSON.parse(json), view = scene.snapshot(), trace = scene.developerTrace();
  const invalid = [
    '{', '\uFEFF' + json, ' '.repeat(65537),
    json.replace('"schema":2', '"schema":2,"schema":2'),
    json.replace('"schema":2', '"schema":2,"sch\\u0065ma":2'),
    ...[null, [], { ...valid, schema: 1 }, { ...valid, schema: 3 }, { ...valid, profileId: 'other' },
      { ...valid, exactWasmSHA256: '0'.repeat(64) }, { ...valid, condition: 'current' },
      { ...valid, actions: [3] }, { ...valid, actions: [1, 1] }, { ...valid, actions: [0] },
      { ...valid, actions: [7] }, { ...valid, actions: [1.5] }, { ...valid, actions: ['2'] },
      { ...valid, actions: Array(4097).fill(2) }, { ...valid, actions: Array(1001).fill(6) },
    ].map(value => JSON.stringify(value)),
  ];
  for (const source of invalid) {
    await assert.rejects(scene.loadCheckpoint(new Blob([source])));
    assert.deepEqual(scene.snapshot(), view);
    assert.equal(scene.developerTrace(), trace);
    assert.equal(scene.saveCheckpoint(), json);
    assert.equal(scene.busy, false);
  }
  const legacy = await CheckpointSession.fromBytes(bytes);
  await assert.rejects(legacy.loadCheckpoint(new Blob([json])));
  await assert.rejects(scene.loadCheckpoint(new Blob([legacy.saveCheckpoint()])));
});

test('configured load locks overlap; replay cap rejects before mutation and keeps save/reset available', async () => {
  const scene = await checkpoint();
  const completed: Action[] = [1, 6, 6, 6, 5, 3, 4, 6, 5];
  const prefix: Action[] = [...completed, ...Array<Action>(996).fill(6)];
  run(scene, [...prefix, ...Array<Action>(4096 - prefix.length - 1).fill(2), 5]);
  const json = scene.saveCheckpoint();
  assert.equal(JSON.parse(json).actions.length, 4096);
  let release!: (value: ArrayBuffer) => void;
  class Deferred extends Blob { override arrayBuffer(): Promise<ArrayBuffer> { return new Promise(resolve => { release = resolve; }); } }
  const pending = scene.loadCheckpoint(new Deferred(['x']));
  assert.equal(scene.busy, true);
  assert.throws(() => scene.act(0), /loading/);
  assert.throws(() => scene.act(6), /loading/);
  await assert.rejects(scene.loadCheckpoint(new Blob([json])), /already loading/);
  release(new TextEncoder().encode(json).buffer);
  await pending;
  assert.equal(scene.saveCheckpoint(), json);
  const atCapacity = scene.snapshot(), trace = scene.developerTrace();
  assert.throws(() => scene.act(2), (error: unknown) => error instanceof CapacityError && error.resource === 'checkpoint_commands');
  assert.deepEqual(scene.snapshot(), atCapacity);
  assert.equal(scene.developerTrace(), trace);
  assert.equal(scene.saveCheckpoint(), json);
  assert.equal(scene.canSave, true);
  await assert.rejects(scene.loadCheckpoint(new Blob(['{'])));
  assert.equal(scene.canSave, true);
  await scene.loadCheckpoint(new Blob([json]));
  assert.equal(scene.canSave, true);
  run(scene, [0]);
  equal(scene, await fresh());
});

test('configured metadata rejects malformed capability exports before accepting a session', async () => {
  for (const invalid of ['version', 'observer', 'delay', 'trace', 'profile', 'bound', 'missing', 'confidence', 'confidence-arity']) {
    const instance = await WebAssembly.instantiate(module, {});
    const original = instance.exports.dw_convoy_config as (field: number) => number;
    const exports = { ...instance.exports };
    if (invalid === 'missing') delete exports.dw_convoy_config;
    else if (invalid === 'profile') exports.dw_convoy_profile_len = () => 65;
    else if (invalid === 'confidence') exports.dw_convoy_confidence = () => 3;
    else if (invalid === 'confidence-arity') exports.dw_convoy_confidence = (unused: number) => unused;
    else exports.dw_convoy_config = (field: number) => {
      if (invalid === 'version' && field === 0) return 2;
      if (invalid === 'observer' && field === 1) return 3;
      if (invalid === 'delay' && field === 4) return -1;
      if (invalid === 'trace' && field === 7) return 1025;
      if (invalid === 'bound' && field === 8) return 0;
      return original(field);
    };
    assert.throws(() => new ConfiguredConvoy({ exports } as unknown as WebAssembly.Instance));
  }
});

test('older configured capability without confidence metadata retains trusted interpretation', async () => {
  const instance = await WebAssembly.instantiate(module, {});
  const exports = { ...instance.exports };
  delete exports.dw_convoy_confidence;
  const scene = new ConfiguredConvoy({ exports } as unknown as WebAssembly.Instance);
  assert.equal(scene.configuration.selaConfidence, 'trusted');
});
