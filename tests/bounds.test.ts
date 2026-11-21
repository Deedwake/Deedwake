// @ts-ignore Node built-ins are supplied by the runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
import { CapacityError, ConfiguredConvoy } from '../web/configured.ts';
import { CheckpointSession } from '../web/checkpoint.ts';
import type { Action } from '../web/fixture.ts';
const bytes=readFileSync(new URL('../public/convoy.wasm',import.meta.url));
const module=await WebAssembly.compile(bytes);
const fresh=async()=>new ConfiguredConvoy(await WebAssembly.instantiate(module,{}));
const run=(scene:ConfiguredConvoy|CheckpointSession,actions:readonly Action[])=>{for(const action of actions)assert.equal(scene.act(action),0);};

test('actual Wasm capacity refusal preserves output, trace and clock and protected story still completes',async()=>{
 const scene=await fresh();let refused=false;
 for(let attempt=0;attempt<4096;attempt++){
  const before=scene.snapshot(),trace=scene.developerTrace();
  try{scene.act(5);}catch(error){if(!(error instanceof CapacityError))throw error;assert.equal(error.resource,'records');assert.deepEqual(scene.snapshot(),before);assert.equal(scene.developerTrace(),trace);refused=true;break;}
 }
 assert.equal(refused,true);
 run(scene,[1,6,6,6,3,4,6,5]);
 assert.match(scene.snapshot().dialogue,/Service is restored/);
 const complete=scene.snapshot();assert.ok(complete.journal.some(entry=>/You released/.test(entry)));
 run(scene,[0]);assert.deepEqual(scene.snapshot(),(await fresh()).snapshot());
});

test('configured replay-log reserve rejects optional repetition while preserving hold, correction, return and save',async()=>{
 const scene=await CheckpointSession.fromConfiguredBytes(bytes);let refused=false;
 for(let attempt=0;attempt<4096;attempt++){
  const before=scene.snapshot(),save=scene.saveCheckpoint();
  try{scene.act(2);}catch(error){if(!(error instanceof CapacityError))throw error;assert.equal(error.resource,'checkpoint_commands');assert.deepEqual(scene.snapshot(),before);assert.equal(scene.saveCheckpoint(),save);assert.equal(scene.canSave,true);refused=true;break;}
 }
 assert.equal(refused,true);
 run(scene,[1,6,6,6,5,3,4,6,5]);
 assert.match(scene.snapshot().dialogue,/Service is restored/);
 const save=scene.saveCheckpoint(),view=scene.snapshot(),trace=scene.developerTrace();
 await scene.loadCheckpoint(new Blob([save]));assert.deepEqual(scene.snapshot(),view);assert.equal(scene.developerTrace(),trace);
 run(scene,[0]);assert.equal(scene.canSave,true);assert.equal(JSON.parse(scene.saveCheckpoint()).actions.length,0);
});

test('typed capacity codes are separate from corruption errors and ordinary snapshots contain no usage totals',async()=>{
 for(let code=17;code<=24;code++){
  const instance=await WebAssembly.instantiate(module,{}),original=instance.exports.dw_convoy_command as(action:number)=>number;
  const exports={...instance.exports,dw_convoy_command:(action:number)=>action===0?original(action):code};
  const scene=new ConfiguredConvoy({exports} as unknown as WebAssembly.Instance),before=scene.snapshot();
  assert.throws(()=>scene.act(5),(error:unknown)=>error instanceof CapacityError);
  assert.deepEqual(scene.snapshot(),before);assert.deepEqual(Object.keys(before).sort(),['choices','dialogue','journal','time']);
  assert.equal(scene.act(0),0);
 }
 for(const code of [3,16,25,-1,1.5]){
  const instance=await WebAssembly.instantiate(module,{});
  const scene=new ConfiguredConvoy({exports:{...instance.exports,dw_convoy_command:(_action:number)=>code}} as unknown as WebAssembly.Instance);
  assert.throws(()=>scene.act(5),(error:unknown)=>error instanceof Error&&!(error instanceof CapacityError));
 }
});
