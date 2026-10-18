// Historical comparison only. Bind fresh-library revision numbers after complete codec-content proof.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const EXPECTED = new Map([
  ['0c7102e2-dc0b-3da6-3fa6-466feeb9043d',3],
  ['7dce29ad-3051-e2be-3004-58d56d4c9a69',3],
  ['9a4e73b4-c14b-ae4c-d444-d98d51dcda0f',2],
]);
const OLD_FLAGS=['reacts_to_sequence_master','reacts_to_group_master','reacts_to_grand_master'];
const clone=structuredClone;
function stable(value){
  if(Array.isArray(value))return `[${value.map(stable).join(',')}]`;
  if(value&&typeof value==='object')return `{${Object.keys(value).sort().map(k=>`${JSON.stringify(k)}:${stable(value[k])}`).join(',')}}`;
  assert(value===null||typeof value!=='number'||Number.isFinite(value));
  return JSON.stringify(value);
}
// Both pinned Rust codecs add the nominal calibration default; only old f67 emits master flags.
// Their HTTP json_values/encode_value uses to_vec -> from_slice (shortest f32), NOT to_value.
// Do not Math.fround arbitrary numbers or discard fields: these frozen authored decimals already
// equal the shortest-f32 wire spelling. A different spelling/value fails full structural equality.
export function expectedInstalledProfile(profile,armCode){
  assert([1,2,3].includes(armCode));assert.equal(EXPECTED.get(profile.id),profile.revision);
  assert.equal(profile.schema_version,3);assert.equal(profile.reserved_source,null);
  const expected=clone(profile);expected.revision=1;
  let calibrations=0;
  for(const mode of expected.modes){
    for(const color of mode.color_systems){
      assert(!Object.hasOwn(color,'calibration'));
      color.calibration={status:'nominal',revision:0};calibrations++;
    }
    for(const channel of mode.channels){
      for(const key of OLD_FLAGS){assert(!Object.hasOwn(channel,key));if(armCode===1)channel[key]=false;}
    }
  }
  assert.equal(calibrations,profile.id==='0c7102e2-dc0b-3da6-3fa6-466feeb9043d'?10:0);
  return expected;
}
export function bindInstalledProfiles(input,installedProfiles,armCode){
  assert([1,2,3].includes(armCode));assert(Array.isArray(installedProfiles));
  assert.equal(input.patch.profile_revisions.length,3);
  const seen=new Set(),profiles=new Map(),profileBindings=[];
  for(const canonical of input.patch.profile_revisions){
    assert(!seen.has(canonical.profile_id));seen.add(canonical.profile_id);
    assert.equal(EXPECTED.get(canonical.profile_id),canonical.profile_revision);
    assert.equal(canonical.profile_snapshot.id,canonical.profile_id);
    assert.equal(canonical.profile_snapshot.revision,canonical.profile_revision);
    const found=installedProfiles.filter(p=>p.id===canonical.profile_id);assert.equal(found.length,1);
    const expected=expectedInstalledProfile(canonical.profile_snapshot,armCode);
    assert.deepEqual(found[0],expected);
    profiles.set(canonical.profile_id,clone(found[0]));
    profileBindings.push({profile_id:canonical.profile_id,canonical_revision:canonical.profile_revision,actual_revision:1,installed_content_sha256:createHash('sha256').update(stable(found[0])).digest('hex')});
  }
  assert.equal(seen.size,EXPECTED.size);
  const runtimeScene=clone(input.scene),runtimePatch=clone(input.patch);
  function rebind(fixtures){for(const fixture of fixtures){assert.equal(EXPECTED.get(fixture.profile_id),fixture.profile_revision);assert(profiles.has(fixture.profile_id));fixture.profile_revision=1;}}
  assert.equal(runtimeScene.fixtures.length,576);assert.equal(runtimePatch.fixtures.length,576);
  rebind(runtimeScene.fixtures);rebind(runtimePatch.fixtures);
  for(const row of runtimePatch.profile_revisions){row.profile_revision=1;row.profile_snapshot=clone(profiles.get(row.profile_id));}
  return {runtimeScene,runtimePatch,profileBindings};
}
