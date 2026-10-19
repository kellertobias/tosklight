// Private comparison translator; no renderer, I/O, mutation or feature-equivalence claim.
import assert from 'node:assert/strict';
const copy = (value) => structuredClone(value);
const keys = (value, names) => {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), names.split(' ').sort());
};
const uuid = (value) => assert.match(value, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
const rational = (value) => {
  keys(value, 'numerator denominator');
  for (const n of Object.values(value)) assert.ok(Number.isSafeInteger(n) && n > 0);
};
const PWM = {attack:0,on:0.5,decay:0,off:0.5,attack_interpolation:'linear',decay_interpolation:'linear'};
const PHASE = {ordering:{type:'selection'},offset_degrees:0,span_degrees:360,block_size:1,repeats:1,wings:false,anchors_degrees:[]};
export const LEGACY_MAPPING = Object.freeze({
  'color.red':'color.red', 'color.green':'color.green', 'color.blue':'color.blue',
  'color.white_blend':'color.white', 'position.pan':'pan', 'position.tilt':'tilt', intensity:'intensity',
});
const EXPECTED = {intensity:1184,'color.red':1184,'color.green':1184,'color.blue':1184,'color.white_blend':224,'position.pan':224,'position.tilt':224};
function validateDefinition(d,index) {
  keys(d, 'id pool_number revision name color icon target_binding lanes random_groups phase_mode spatial_mapping phase speed overall_speed_multiplier run_mode default_activation activation_boundary');
  uuid(d.id); assert.equal(d.pool_number,9001+index); assert.equal(d.revision,1);
  assert.equal(d.name,`Stage capacity Dynamic ${String(index+1).padStart(2,'0')}`); assert.equal(d.color,null); assert.equal(d.icon,null);
  assert.deepEqual(d.target_binding,{type:'targetless'}); assert.deepEqual(d.random_groups,[]);
  assert.equal(d.phase_mode,'uniform'); assert.deepEqual(d.phase,{...PHASE,offset_degrees:(index*19)%360});
  assert.deepEqual(d.spatial_mapping,{projection:{type:'inherit'},shape:{type:'inherit'}});
  keys(d.speed,'type duration_millis'); assert.equal(d.speed.type,'fixed');
  assert.equal(d.speed.duration_millis,2400+(index%5)*350);
  assert.deepEqual(d.overall_speed_multiplier,{numerator:1,denominator:1}); assert.equal(d.run_mode,'loop');
  assert.equal(d.default_activation,'start_now'); assert.equal(d.activation_boundary,'beat');
  assert.ok(Array.isArray(d.lanes) && d.lanes.length > 0);
}
function legacyConfigurations(low, high, functionName, size, pwm) {
  return {
    mode:'max_min',
    keyframes:{points:[{position:0,source:{type:'value',value:low},interpolation:'linear'},{position:0.5,source:{type:'value',value:high},interpolation:'linear'}],size},
    max_min:{minimum:{type:'value',value:low},maximum:{type:'value',value:high},function:functionName,size,pwm:copy(pwm)},
    middle_amplitude:{middle:{type:'current'},amplitude:(high-low)/2,function:functionName,size,pwm:copy(pwm)},
  };
}
function translateLane(lane) {
  const common = ['id','speed_multiplier','width','random_group_id','phase'];
  uuid(lane.id); rational(lane.speed_multiplier); assert.equal(lane.width,1);
  assert.equal(lane.phase,null); assert.equal(lane.random_group_id,null);
  let attribute, configuration;
  if (!Object.hasOwn(lane,'programming')) {
    keys(lane,[...common,'attribute','mode','keyframes','max_min','middle_amplitude'].join(' '));
    assert.equal(lane.attribute,'intensity');
    configuration=legacyConfigurations(0.25,0.9,'sinus',1,PWM);
    for (const key of Object.keys(configuration)) assert.deepEqual(lane[key],configuration[key]);
    return {lane:copy(lane), original:'intensity'};
  }
  keys(lane,[...common,'programming'].join(' '));
  const p=lane.programming; keys(p,'address configuration'); keys(p.address,'representation component');
  const r=p.address.representation, c=p.address.component; let original;
  if (r.kind==='semantic_color') {
    assert.deepEqual(r,{kind:'semantic_color',basis:'recipe'}); keys(c,'kind component');
    assert.equal(c.kind,'color'); assert.ok(['red','green','blue','white_blend'].includes(c.component));
    original=`color.${c.component}`;
  } else {
    assert.deepEqual(r,{kind:'angles'}); keys(c,'kind'); assert.ok(['pan','tilt'].includes(c.kind));
    original=`position.${c.kind}`;
  }
  keys(p.configuration,'mode configuration'); assert.equal(p.configuration.mode,'max_min');
  const v=p.configuration.configuration; keys(v,'minimum maximum function size pwm');
  const scalar = (s) => {keys(s,'kind value'); assert.equal(s.kind,'value'); keys(s.value,'kind value'); assert.equal(s.value.kind,'scalar'); assert.ok(Number.isFinite(s.value.value)); return s.value.value;};
  let low=scalar(v.minimum), high=scalar(v.maximum);
  assert.ok(['sinus','cosinus'].includes(v.function)); assert.equal(v.size,1); assert.deepEqual(v.pwm,PWM);
  if (original.startsWith('position.')) {
    const extent=original==='position.pan'?162:81;
    assert.equal(low,-extent); assert.equal(high,extent);
    // Explicit common legacy show: normalized travel, not typed Angle equivalence.
    low=0.2; high=0.8;
  } else { assert.equal(low,0.1); assert.equal(high,1); }
  attribute=LEGACY_MAPPING[original]; configuration=legacyConfigurations(low,high,v.function,v.size,v.pwm);
  return {lane:{id:lane.id,attribute,...configuration,speed_multiplier:copy(lane.speed_multiplier),width:lane.width,random_group_id:null,phase:null},original};
}
/** Translate only this admitted frozen576 plan. Metadata and target order are copied exactly.
 * Inactive legacy configurations are explicit comparison defaults; typed mode has none.
 * WhiteBlend->native White and Angle->normalized travel are deliberate semantic translations.
 */
export function translateLegacy576Plan(plan) {
  keys(plan,'definitions activations targetDescriptors dynamicTargetCount staticControlFixtureIds laneCoverage');
  assert.equal(plan.definitions.length,20); assert.equal(plan.activations.length,20);
  assert.equal(plan.dynamicTargetCount,5408); assert.deepEqual(plan.laneCoverage,EXPECTED);
  const result=copy(plan), coverage={}, seen=new Set();
  result.definitions=plan.definitions.map((d,index) => {
    validateDefinition(d,index); keys(plan.activations[index],'definition targets');
    assert.deepEqual(plan.activations[index].definition,d);
    const targets=plan.activations[index].targets; assert.ok(Array.isArray(targets) && targets.length>0);
    assert.equal(new Set(targets).size,targets.length); targets.forEach(uuid);
    const lanes=d.lanes.map((l) => {
      const t=translateLane(l); coverage[t.original]=(coverage[t.original]??0)+targets.length;
      for (const target of targets) {const k=`${target}:${t.lane.attribute}`; assert.ok(!seen.has(k)); seen.add(k);}
      return t.lane;
    });
    return {...copy(d),lanes};
  });
  assert.deepEqual(coverage,EXPECTED); assert.equal(seen.size,5408);
  result.activations=plan.activations.map((a,i)=>({definition:copy(result.definitions[i]),targets:copy(a.targets)}));
  result.targetDescriptors=plan.targetDescriptors.map((d)=>{
    keys(d,'target fixtureId headId attributes signature'); uuid(d.target);uuid(d.fixtureId);uuid(d.headId);
    assert.equal(d.signature,d.attributes.join('|'));
    const attributes=d.attributes.map(a=>{assert.ok(Object.hasOwn(LEGACY_MAPPING,a));return LEGACY_MAPPING[a];});
    return {...copy(d),attributes,signature:attributes.join('|')};
  });
  plan.staticControlFixtureIds.forEach(uuid);
  result.laneCoverage=Object.fromEntries(Object.entries(EXPECTED).map(([k,v])=>[LEGACY_MAPPING[k],v]));
  return result;
}

export const translateLegacyPlan = translateLegacy576Plan;
