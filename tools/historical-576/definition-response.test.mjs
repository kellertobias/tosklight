import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import path from 'node:path';
import {bindCreatedDefinition,validateDefinitionBindings,installDefinitions} from './definition-response.mjs';
import {translateLegacyPlan} from './legacy-plan.mjs';
assert(process.env.LIGHT_HISTORICAL_INPUT&&path.isAbsolute(process.env.LIGHT_HISTORICAL_INPUT));
const bytes=await readFile(process.env.LIGHT_HISTORICAL_INPUT);assert.equal(createHash('sha256').update(bytes).digest('hex'),'0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f');
const input=JSON.parse(bytes),typed=input.plan,legacy=translateLegacyPlan(typed),clone=structuredClone;
const id=i=>`99000000-0000-4000-8000-${String(i+1).padStart(12,'0')}`;
// Independent mock of the frozen source fields' f32 -> serde_json::Value wire promotion.
// Do not use the production expected-body normalizer to manufacture its oracle.
function wireDefinition(d) {
  const result=clone(d);
  for(const lane of result.lanes){
    if(lane.programming){
      const c=lane.programming.configuration.configuration;
      c.minimum.value.value=Math.fround(c.minimum.value.value);
      c.maximum.value.value=Math.fround(c.maximum.value.value);
    }else{
      for(const point of lane.keyframes.points)point.source.value=Math.fround(point.source.value);
      lane.max_min.minimum.value=Math.fround(lane.max_min.minimum.value);
      lane.max_min.maximum.value=Math.fround(lane.max_min.maximum.value);
      lane.middle_amplitude.amplitude=Math.fround(lane.middle_amplitude.amplitude);
    }
  }
  return result;
}
const response=(d,i=0)=>({request_id:'fixed-request',replayed:false,show_id:id(99),show_revision:1,object:{kind:'dynamic',id:id(i),revision:1,updated_at:'fixed',body:{...wireDefinition(d),id:id(i),revision:1}}});
function all(plan){let bindings=[];const definitions=plan.definitions.map((d,i)=>{const next=bindCreatedDefinition(d,response(d,i),bindings);bindings=next.bindings;return next.definition;});validateDefinitionBindings(bindings,plan.definitions);return {bindings,definitions};}
test('all20 typed definitions preserve complete f32 wire bodies except server ID',()=>{const x=all(typed);assert.equal(x.definitions.length,20);x.definitions.forEach((d,i)=>assert.deepEqual({...d,id:typed.definitions[i].id},wireDefinition(typed.definitions[i])));});
test('all20 legacy definitions preserve complete f32 wire bodies except server ID',()=>{const x=all(legacy);assert.equal(x.bindings.length,20);x.definitions.forEach((d,i)=>assert.deepEqual({...d,id:legacy.definitions[i].id},wireDefinition(legacy.definitions[i])));});
test('canonical plan and response and previous bindings are never mutated',()=>{const d=typed.definitions[0],r=response(d),previous=[],snapshot=JSON.stringify([typed,r,previous]);bindCreatedDefinition(d,r,previous);assert.equal(JSON.stringify([typed,r,previous]),snapshot);});
test('runtime plan can use returned IDs while preserving every activation target and lane order',()=>{for(const plan of [typed,legacy]){const x=all(plan),runtime={...clone(plan),definitions:x.definitions,activations:plan.activations.map((a,i)=>({...clone(a),definition:x.definitions[i]}))};assert.deepEqual(runtime.activations.map(a=>a.targets),plan.activations.map(a=>a.targets));assert.deepEqual(runtime.definitions.map(d=>d.lanes),plan.definitions.map(d=>wireDefinition(d).lanes));assert.equal(new Set(runtime.definitions.map(d=>d.id)).size,20);}});
test('frozen typed Angle pair is authored MaxMin on both axes, no automatic Current partner',()=>{assert.equal(typed.definitions.length,20);const axes=typed.definitions.filter(d=>d.lanes.some(l=>l.programming?.address?.representation.kind==='angles'));assert.equal(axes.length,2);for(const d of axes){assert.deepEqual(d.lanes.map(l=>l.programming.address.component.kind),['pan','tilt']);for(const l of d.lanes){assert.equal(l.programming.configuration.mode,'max_min');const c=l.programming.configuration.configuration;assert.equal(c.minimum.kind,'value');assert.equal(c.maximum.kind,'value');}}});
test('server must assign a distinct valid UUID',()=>{const d=typed.definitions[0];for(const bad of [d.id,'not-uuid',null]){const r=response(d);r.object.id=bad;r.object.body.id=bad;assert.throws(()=>bindCreatedDefinition(d,r));}});
test('object identity/revision/body and validation status must agree',()=>{const d=typed.definitions[0];for(const edit of [r=>r.object.kind='cue',r=>r.object.revision=2,r=>r.object.body.revision=2,r=>r.object.body.id=id(2),r=>r.object.validation_error='invalid',r=>delete r.object.body]){const r=response(d);edit(r);assert.throws(()=>bindCreatedDefinition(d,r));}});
test('duplicate actual and canonical IDs refuse before accepting another definition',()=>{const a=bindCreatedDefinition(typed.definitions[0],response(typed.definitions[0]));assert.throws(()=>bindCreatedDefinition(typed.definitions[1],response(typed.definitions[1]),a.bindings));assert.throws(()=>bindCreatedDefinition(typed.definitions[0],response(typed.definitions[0],1),a.bindings));});
test('all lane ID/order/count and source configuration changes refuse',()=>{const d=typed.definitions[0];for(const edit of [r=>r.object.body.lanes[0].id=id(5),r=>r.object.body.lanes.reverse(),r=>r.object.body.lanes.pop(),r=>r.object.body.lanes[0].programming.configuration.configuration.minimum.value.value=0.2]){const r=response(d);edit(r);assert.throws(()=>bindCreatedDefinition(d,r));}});
test('speed phase and arbitrary omitted or extra definition fields refuse',()=>{const d=typed.definitions[0];for(const edit of [r=>r.object.body.speed.duration_millis++,r=>r.object.body.phase.offset_degrees++,r=>delete r.object.body.name,r=>r.object.body.extra=true]){const r=response(d);edit(r);assert.throws(()=>bindCreatedDefinition(d,r));}});
test('final mappings require all20 exact canonical IDs in authored order',()=>{const x=all(typed);assert.throws(()=>validateDefinitionBindings(x.bindings.slice(1),typed.definitions));assert.throws(()=>validateDefinitionBindings(x.bindings.slice().reverse(),typed.definitions));const bad=clone(x.bindings);bad[1].actual_id=bad[0].actual_id;assert.throws(()=>validateDefinitionBindings(bad,typed.definitions));});
test('mapping state and candidate revision are closed and invalid state is refused',()=>{const d=typed.definitions[0];for(const prior of [[{canonical_id:id(2),actual_id:id(3),extra:1}],[{canonical_id:id(2),actual_id:id(2)}],{}])assert.throws(()=>bindCreatedDefinition(d,response(d),prior));assert.throws(()=>bindCreatedDefinition({...d,revision:2},response(d)));});
test('async fake API installs all20 typed and legacy definitions using only returned IDs',async()=>{
  for(const plan of [typed,legacy]){
    const before=JSON.stringify(plan);let calls=0,requests=0;const started=[];
    const api={async request(method,route,payload){const index=Math.floor(calls/2);assert.equal(method,'POST');assert.equal(payload.request_id,`request-${++requests}`);
      if(calls++%2===0){assert.equal(route,'/api/v2/dynamics/create');assert.deepEqual(payload.definition,plan.definitions[index]);return response(payload.definition,index);}
      assert.equal(route,`/api/v2/dynamics/${id(index)}/start`);assert.deepEqual(payload,{request_id:`request-${requests}`,targets:plan.activations[index].targets,overrides:{size:1,speed_multiplier:{numerator:1,denominator:1},phase_offset_degrees:0},timing:{},undo_group:'fixture-sheet-performance'});started.push(id(index));return {runtime_instance_id:id(index+100),controller_id:id(index+200),started:true,targets:clone(payload.targets)};
    }};
    let request=0;const result=await installDefinitions(api,plan,()=>`request-${++request}`);assert.equal(calls,40);assert.equal(JSON.stringify(plan),before);assert.deepEqual(started,result.runtimePlan.definitions.map(d=>d.id));assert.deepEqual(result.starts.map(s=>s.dynamic_id),started);assert.deepEqual(result.runtimePlan.activations.map(a=>a.definition),result.runtimePlan.definitions);assert.deepEqual(result.runtimePlan.activations.map(a=>a.targets),plan.activations.map(a=>a.targets));assert.deepEqual(result.runtimePlan.definitions.map(d=>d.lanes),plan.definitions.map(d=>wireDefinition(d).lanes));assert.equal(result.definitionBindings.length,20);
  }
});
test('async fake API rejects changed returned body before starting it or creating later definitions',async()=>{
  let calls=0;const api={async request(){calls++;const r=response(typed.definitions[0]);r.object.body.lanes[0].id=id(50);return r;}};
  await assert.rejects(installDefinitions(api,typed,()=> 'request'));assert.equal(calls,1);
});

test('f32 wire promotion is exact and narrow, with inherited mapping retained',()=>{
  const d=typed.definitions[0],r=response(d),body=r.object.body;
  assert.equal(body.lanes[0].programming.configuration.configuration.minimum.value.value,0.10000000149011612);
  assert.notEqual(body.lanes[0].programming.configuration.configuration.minimum.value.value,0.1);
  assert.deepEqual(body.spatial_mapping,d.spatial_mapping);
  assert.doesNotThrow(()=>bindCreatedDefinition(d,r));
  const unrounded=response(d);unrounded.object.body.lanes[0].programming.configuration.configuration.minimum.value.value=0.1;
  assert.throws(()=>bindCreatedDefinition(d,unrounded));
  for(const edit of [
    b=>delete b.spatial_mapping,
    b=>b.lanes[0].programming.configuration.configuration.minimum.value.value+=Number.EPSILON,
    b=>b.lanes[0].width+=Number.EPSILON,
    b=>b.phase.offset_degrees+=Number.EPSILON,
    b=>b.speed.duration_millis+=0.000001,
    b=>b.name+=' changed',
    b=>b.lanes[0].programming.configuration.configuration.extra=1,
  ]){const changed=response(d);edit(changed.object.body);assert.throws(()=>bindCreatedDefinition(d,changed));}
});
test('all frozen numeric source promotions are present without changing canonical plans',()=>{
  for(const [plan,count] of [[typed,58],[legacy,170]]){
    const before=JSON.stringify(plan);let differences=0;
    function compare(a,b){if(typeof a==='number'&&a!==b){assert.equal(b,Math.fround(a));differences++;}else if(a&&typeof a==='object'){assert.deepEqual(Object.keys(a),Object.keys(b));for(const key of Object.keys(a))compare(a[key],b[key]);}else assert.equal(a,b);}
    for(const d of plan.definitions)compare(d,wireDefinition(d));
    assert.equal(differences,count);all(plan);assert.equal(JSON.stringify(plan),before);
  }
});

test('legacy f32 endpoints and inactive amplitude remain exact wire values',()=>{
  const d=legacy.definitions.find(d=>d.lanes.some(l=>l.attribute==='pan'));
  const r=response(d);const lane=r.object.body.lanes.find(l=>l.attribute==='pan');
  assert.equal(lane.max_min.minimum.value,0.20000000298023224);
  assert.equal(lane.max_min.maximum.value,0.800000011920929);
  assert.equal(lane.middle_amplitude.amplitude,0.30000001192092896);
  assert.doesNotThrow(()=>bindCreatedDefinition(d,r));
  for(const edit of [
    l=>l.max_min.minimum.value=0.2,
    l=>l.middle_amplitude.amplitude=0.3,
    l=>l.keyframes.points[0].source.value+=Number.EPSILON,
    l=>l.keyframes.points[0].position+=Number.EPSILON,
    l=>l.speed_multiplier.numerator+=0.000001,
    l=>l.middle_amplitude.extra=0,
  ]){const changed=response(d);edit(changed.object.body.lanes.find(l=>l.attribute==='pan'));assert.throws(()=>bindCreatedDefinition(d,changed));}
});
