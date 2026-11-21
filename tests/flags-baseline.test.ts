// @ts-ignore Node built-ins are supplied by the runtime.
import test from 'node:test';
// @ts-ignore See above.
import assert from 'node:assert/strict';
// @ts-ignore See above.
import { readFileSync } from 'node:fs';
// @ts-ignore See above.
import { spawnSync } from 'node:child_process';
// @ts-ignore See above.
import { fileURLToPath } from 'node:url';
// @ts-ignore See above.
import process from 'node:process';
import { ConfiguredConvoy } from '../web/configured.ts';
import type { Action } from '../web/fixture.ts';
const bytes=readFileSync(process.env.DEEDWAKE_CONFIGURED_WASM ?? new URL('../public/convoy.wasm',import.meta.url));
const binary=process.env.DEEDWAKE_BASELINE_BINARY ?? fileURLToPath(new URL('../target/debug/convoy-baseline',import.meta.url));
const config=process.env.DEEDWAKE_BASELINE_CONFIG ?? fileURLToPath(new URL('../example-web/convoy.toml',import.meta.url));
const paths:readonly (readonly Action[])[]=[
 [0,5,1,2,2,5,6,5,6,5,6,5,3,4,6,5,6,6,5],
 [0,1,6,6,6,5,3,6,6,6,5,4,6,5],
 [0,1,3,4,6,5,6,6,5],
 [0,3,4,1,1,3,3,4,4,6,6,6,5,0,1,2,5],
 [0,1,...Array.from({length:90},()=>[2,5] as const).flat(),6,6,6,3,4,6,5],
 [0,...Array<Action>(1000).fill(6),6,5,0,1,2,5],
];

test('ordinary flags baseline matches controlled player projections of actual Rust/Wasm in its declared all-available scope',async()=>{
 for(const actions of paths){
  const native=spawnSync(binary,['--config',config,'--actions',actions.join(','),'--json'],{encoding:'utf8',maxBuffer:32*1024*1024});
  assert.equal(native.stderr,'');assert.ok(native.status===0||native.status===1,native.stdout);
  const report=JSON.parse(native.stdout);assert.equal(report.schema_version,1);assert.equal(report.implementation,'flags_baseline');
  const {instance}=await WebAssembly.instantiate(bytes,{});const scene=new ConfiguredConvoy(instance);const steps=[];let unavailable=false;
  for(const action of actions){const result=scene.act(action),view=scene.snapshot();unavailable ||= result!==0;steps.push({action,result,time:view.time,dialogue:view.dialogue});}
  assert.equal(native.status,unavailable?1:0);assert.deepEqual(report.steps,steps);
  const view=scene.snapshot(),enabled=instance.exports.dw_enabled as (action:number)=>number;
  assert.deepEqual(report.final,{time:view.time,dialogue:view.dialogue,journal:view.journal,enabled:Array.from({length:7},(_,id)=>enabled(id)===1)});
  assert.equal(Object.hasOwn(report.final,'trace'),false);
 }
});
