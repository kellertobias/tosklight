// CI comparison only: bind server-owned Dynamic IDs without altering authored lanes or plan hashes.
import assert from 'node:assert/strict';
const uuid=value=>assert(typeof value==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(value));
function entries(bindings){
  assert(Array.isArray(bindings)&&bindings.length<=20);const canonical=new Set(),actual=new Set();
  for(const entry of bindings){assert.deepEqual(Object.keys(entry).sort(),['actual_id','canonical_id']);uuid(entry.canonical_id);uuid(entry.actual_id);assert.notEqual(entry.canonical_id,entry.actual_id);assert(!canonical.has(entry.canonical_id)&&!actual.has(entry.actual_id));canonical.add(entry.canonical_id);actual.add(entry.actual_id);}
  return {canonical,actual};
}
export function bindCreatedDefinition(definition,response,bindings=[]){
  uuid(definition.id);assert.equal(definition.revision,1);const previous=entries(bindings);assert(bindings.length<20&&!previous.canonical.has(definition.id));
  const object=response?.object;assert(object&&typeof object==='object');assert.equal(object.kind,'dynamic');uuid(object.id);assert.notEqual(object.id,definition.id);assert.equal(object.revision,1);assert(object.validation_error==null);assert(!previous.actual.has(object.id));
  const expected={...structuredClone(definition),id:object.id,revision:1};
  // Compare the COMPLETE returned raw definition: no lane-ID, order, config, phase or metadata filtering.
  assert.deepEqual(object.body,expected);
  return {definition:structuredClone(object.body),bindings:[...structuredClone(bindings),{canonical_id:definition.id,actual_id:object.id}]};
}
export function validateDefinitionBindings(bindings,definitions){
  entries(bindings);assert.equal(definitions.length,20);assert.equal(bindings.length,20);
  assert.deepEqual(bindings.map(x=>x.canonical_id),definitions.map(x=>x.id));return bindings;
}
export async function installDefinitions(api,plan,requestId){
  assert.equal(typeof api.request,'function');assert.equal(typeof requestId,'function');assert.equal(plan.definitions.length,20);assert.equal(plan.activations.length,20);
  const runtimePlan=structuredClone(plan),starts=[];let definitionBindings=[];
  for(let i=0;i<runtimePlan.activations.length;i++){
    const activation=runtimePlan.activations[i];assert.deepEqual(activation.definition,plan.definitions[i]);
    const created=await api.request('POST','/api/v2/dynamics/create',{request_id:requestId(),definition:structuredClone(activation.definition)});
    const bound=bindCreatedDefinition(activation.definition,created,definitionBindings);definitionBindings=bound.bindings;
    activation.definition=bound.definition;runtimePlan.definitions[i]=structuredClone(bound.definition);
    const outcome=await api.request('POST',`/api/v2/dynamics/${encodeURIComponent(bound.definition.id)}/start`,{request_id:requestId(),targets:structuredClone(activation.targets),overrides:{size:1,speed_multiplier:{numerator:1,denominator:1},phase_offset_degrees:0},timing:{},undo_group:'fixture-sheet-performance'});
    starts.push({dynamic_id:bound.definition.id,instance_id:outcome.runtime_instance_id,controller_id:outcome.controller_id,started:outcome.started,targets:structuredClone(outcome.targets)});
  }
  validateDefinitionBindings(definitionBindings,plan.definitions);return {runtimePlan,definitionBindings,starts};
}
