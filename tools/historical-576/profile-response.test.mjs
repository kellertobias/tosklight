import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {bindInstalledProfiles,expectedInstalledProfile} from './profile-response.mjs';
assert(process.env.LIGHT_HISTORICAL_INPUT&&path.isAbsolute(process.env.LIGHT_HISTORICAL_INPUT));
const bytes=await readFile(process.env.LIGHT_HISTORICAL_INPUT);
assert.equal(createHash('sha256').update(bytes).digest('hex'),'0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f');
const input=JSON.parse(bytes),clone=structuredClone;
// Deliberately independent fixture builder: exact Rust defaults observed in the two frozen sources.
function installed(arm){return input.patch.profile_revisions.map(({profile_snapshot:p})=>{
 const r=clone(p);r.revision=1;
 for(const m of r.modes){for(const c of m.color_systems)c.calibration={status:'nominal',revision:0};
 if(arm===1)for(const c of m.channels)Object.assign(c,{reacts_to_sequence_master:false,reacts_to_group_master:false,reacts_to_grand_master:false});}return r;
});}
const bind=(arm=2,profiles=installed(arm),source=input)=>bindInstalledProfiles(source,profiles,arm);
test('all three arms bind all3 profiles and only rebind576 fixture revisions',()=>{
 for(const arm of [1,2,3]){const r=bind(arm);assert.equal(r.profileBindings.length,3);
 for(const [actual,canonical] of [[r.runtimeScene,input.scene],[r.runtimePatch,input.patch]]){
  assert.equal(actual.fixtures.length,576);actual.fixtures.forEach((f,i)=>assert.deepEqual({...f,profile_revision:canonical.fixtures[i].profile_revision},canonical.fixtures[i]));
 }
 assert.deepEqual(r.runtimePatch.profile_revisions.map(x=>x.profile_snapshot),installed(arm));
 assert(r.profileBindings.every(b=>b.actual_revision===1&&/^[0-9a-f]{64}$/.test(b.installed_content_sha256)));
 }});
test('canonical input and installed responses remain immutable and returned data are owned',()=>{const rows=installed(1),before=JSON.stringify([input,rows]);const r=bind(1,rows);r.runtimeScene.fixtures[0].fixture_number=-1;r.runtimePatch.profile_revisions[0].profile_snapshot.name='changed';assert.equal(JSON.stringify([input,rows]),before);});
test('fresh library requires exactly revision1 and exactly one match per used id',()=>{for(const rev of [0,2,3]){const p=installed(2);p[0].revision=rev;assert.throws(()=>bind(2,p));}assert.throws(()=>bind(2,installed(2).slice(1)));assert.throws(()=>bind(2,[...installed(2),installed(2)[0]]));});
test('physical figures and channel calibration are exact, no f32 tolerance or field ignoring',()=>{for(const mutate of [p=>p[0].physical.weight_kilograms+=0.000001,p=>p[0].mounting.centre_millimetres.z=Math.fround(p[0].mounting.centre_millimetres.z),p=>p[0].modes[0].channels[0].default_raw++,p=>p[0].modes[0].color_systems[0].system.emitters[0].xyz.x+=0.00001]){const p=installed(2);mutate(p);assert.throws(()=>bind(2,p));}});
test('mode channel head and fixture ordering retained and alterations rejected',()=>{for(const key of ['channels','heads']){const p=installed(2);p[0].modes[0][key].reverse();assert.throws(()=>bind(2,p));}const p=installed(2);p[0].modes[0].id='changed';assert.throws(()=>bind(2,p));});
test('only explicit nominal zero calibration default is accepted in both builds',()=>{for(const arm of [1,2,3])for(const action of [c=>delete c.calibration,c=>c.calibration.revision++,c=>c.calibration.status='measured',c=>c.calibration.source=null]){const p=installed(arm);action(p[0].modes[0].color_systems[0]);assert.throws(()=>bind(arm,p));}});
test('old false master flags are not broadly ignored or admitted to current response',()=>{const p=installed(1);p[0].modes[0].channels[0].reacts_to_sequence_master=true;assert.throws(()=>bind(1,p));assert.throws(()=>bind(2,installed(1)));assert.throws(()=>bind(1,installed(2)));});
test('unknown fields and absent authored fields fail complete content equality',()=>{for(const mutate of [p=>p[0].new_field=null,p=>delete p[0].notes,p=>p[1].modes[0].channels[0].new_field=false]){const p=installed(2);mutate(p);assert.throws(()=>bind(2,p));}});
test('canonical missing duplicate or unsupported profile references fail closed',()=>{for(const mutate of [s=>s.patch.profile_revisions.pop(),s=>s.patch.profile_revisions[1]=clone(s.patch.profile_revisions[0]),s=>s.scene.fixtures[0].profile_revision=1,s=>s.patch.fixtures[0].profile_id='unknown']){const s=clone(input);mutate(s);assert.throws(()=>bind(2,installed(2),s));}});
test('invalid arm and modified raw defaults fail rather than generic normalizing',()=>{for(const arm of [0,4,'1'])assert.throws(()=>bind(arm,installed(2)));const p=clone(input.patch.profile_revisions[0].profile_snapshot);p.modes[0].color_systems[0].calibration={status:'nominal',revision:0};assert.throws(()=>expectedInstalledProfile(p,2));});
test('all profile content binding hashes are deterministic and build-specific when content differs',()=>{assert.deepEqual(bind(2),bind(3));const a=bind(1),b=bind(2);assert(a.profileBindings.every((x,i)=>x.installed_content_sha256!==b.profileBindings[i].installed_content_sha256));});
test('definitions and all twenty activations remain outside revision rebinding',()=>{const before=JSON.stringify(input.plan);for(const arm of [1,2,3]){bind(arm);assert.equal(JSON.stringify(input.plan),before);}assert.equal(input.plan.activations.length,20);});
