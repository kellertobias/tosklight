import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import test from 'node:test';
import {createLargeStageDynamicsPlan} from '../stage-dynamics-scene.mjs';
import {LEGACY_MAPPING,translateLegacy576Plan} from './legacy-plan.mjs';
assert.ok(process.env.LIGHT_HISTORICAL_INPUT && path.isAbsolute(process.env.LIGHT_HISTORICAL_INPUT), 'explicit absolute frozen input required');
const inputBytes=await readFile(process.env.LIGHT_HISTORICAL_INPUT);
assert.equal(createHash('sha256').update(inputBytes).digest('hex'),'0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f');
const input=JSON.parse(inputBytes);
const plan=createLargeStageDynamicsPlan(input.patch,input.scene);
const clone=()=>structuredClone(plan);
test('current constructor reproduces frozen typed plan and translator does not mutate it',()=>{
  assert.deepEqual(plan,input.plan);const before=JSON.stringify(plan);translateLegacy576Plan(plan);assert.equal(JSON.stringify(plan),before);
});
test('exact20 partitions, all IDs and original target/lane order survive',()=>{
  const out=translateLegacy576Plan(plan);assert.equal(out.definitions.length,20);
  out.definitions.forEach((d,i)=>{
    assert.deepEqual({...d,lanes:[]},{...plan.definitions[i],lanes:[]});
    assert.deepEqual(out.activations[i].targets,plan.activations[i].targets);
    assert.deepEqual(d.lanes.map(x=>x.id),plan.definitions[i].lanes.map(x=>x.id));
    assert.deepEqual(out.activations[i].definition,d);
  });
});
test('all5408 addresses and per-attribute coverage are preserved under explicit mapping',()=>{
  const out=translateLegacy576Plan(plan),seen=new Set(),counts={};
  for(const a of out.activations)for(const target of a.targets)for(const l of a.definition.lanes){seen.add(`${target}:${l.attribute}`);counts[l.attribute]=(counts[l.attribute]??0)+1;}
  assert.equal(seen.size,5408);assert.deepEqual(counts,{'color.blue':1184,'color.green':1184,'color.red':1184,'color.white':224,intensity:1184,pan:224,tilt:224});
  assert.deepEqual(out.laneCoverage,counts);
});
test('literal mappings and every speed/phase/timing field are pinned',()=>{
  const out=translateLegacy576Plan(plan);
  for(let i=0;i<20;i++)out.definitions[i].lanes.forEach((l,j)=>{
    const old=plan.definitions[i].lanes[j];
    for(const k of ['id','speed_multiplier','phase','width','random_group_id'])assert.deepEqual(l[k],old[k]);
    if(!old.programming){assert.deepEqual(l,old);return;}
    const c=old.programming.configuration.configuration;
    assert.equal(l.max_min.function,c.function);assert.deepEqual(l.max_min.pwm,c.pwm);assert.equal(l.max_min.size,c.size);
    const angle=['pan','tilt'].includes(l.attribute);
    assert.deepEqual([l.max_min.minimum.value,l.max_min.maximum.value],angle?[0.2,0.8]:[0.1,1]);
    assert.equal(l.max_min.minimum.type,'value');assert.equal(l.mode,'max_min');
    assert.equal(l.keyframes.points[0].source.value,l.max_min.minimum.value);
    assert.equal(l.keyframes.points[1].source.value,l.max_min.maximum.value);
  });
});
test('whole descriptor mapping is explicit and leaves physical target identity unchanged',()=>{
  const out=translateLegacy576Plan(plan);
  assert.deepEqual(out.staticControlFixtureIds,plan.staticControlFixtureIds);
  out.targetDescriptors.forEach((d,i)=>{
    const old=plan.targetDescriptors[i];for(const k of ['target','fixtureId','headId'])assert.equal(d[k],old[k]);
    assert.deepEqual(d.attributes,old.attributes.map(x=>LEGACY_MAPPING[x]));assert.equal(d.signature,d.attributes.join('|'));
  });
  const used=new Set(input.scene.fixtures.map(x=>x.profile_id));
  const dls=input.package_bindings.find(x=>x.file.includes('dls-profile'));assert.ok(!used.has(dls.profile_id));
  assert.deepEqual(input.scene.categoryCounts,{sunstrip:96,moving:224,static:256,venue:0});
});
test('unexpected definition fields and altered phase/spatial forms are rejected',()=>{
  for(const edit of [d=>d.unknown=true,d=>d.phase.offset_degrees=10,d=>d.spatial_mapping.extra=1,d=>d.target_binding={type:'live_selection'}]){
    const p=clone();edit(p.definitions[0]);p.activations[0].definition=structuredClone(p.definitions[0]);assert.throws(()=>translateLegacy576Plan(p));
  }
});
test('Current/Preset, wrappers, unexpected modes/components and scalar bounds refuse translation',()=>{
  for(const edit of [p=>p.address.component.component='amber',p=>p.extra=1,p=>p.configuration.mode='random',p=>p.configuration.configuration.minimum={kind:'current'},p=>p.configuration.configuration.maximum.value.value=0.99,p=>p.configuration.configuration.size=0.5]){
    const p=clone();edit(p.definitions[0].lanes[0].programming);p.activations[0].definition=structuredClone(p.definitions[0]);assert.throws(()=>translateLegacy576Plan(p));
  }
});
test('mismatched activation bodies, duplicate targets and missing partitions fail closed',()=>{
  for(const edit of [p=>p.activations[0].definition={...p.activations[0].definition,name:'wrong'},p=>p.activations[0].targets.push(p.activations[0].targets[0]),p=>p.definitions.pop()]){const p=clone();edit(p);assert.throws(()=>translateLegacy576Plan(p));}
});
