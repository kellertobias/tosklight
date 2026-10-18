// Comparison-only normal show lifecycle; never changes runtime capability or bypasses recovery.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
export const EMPTY_SHOW_NAME='Historical 576 Comparison Empty';
const uuid=value=>assert(typeof value==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(value));
const digest=value=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
function outcome(response,requestId){
  assert(response&&typeof response==='object');assert.equal(response.request_id,requestId);assert.equal(response.replayed,false);
  assert.equal(response.result?.type,'show');const show=response.result.show;assert(show&&typeof show==='object');
  uuid(show.id);assert.equal(show.name,EMPTY_SHOW_NAME);assert.equal(show.is_base_show,false);return show;
}
export async function prepareEmptyShow(api,requestId){
  assert.equal(typeof api.request,'function');assert.equal(typeof requestId,'function');
  const previous=api.showId;let complete=false;api.showId=null;
  try{
    const createAction={type:'create',name:EMPTY_SHOW_NAME,data_base64:null,overwrite:false};
    const createId=requestId();assert(typeof createId==='string'&&createId.length>0);
    const created=outcome(await api.request('POST','/api/v2/shows',{request_id:createId,action:createAction}),createId);
    const openAction={type:'open',show_id:created.id,transition:'hold_current',transition_millis:null};
    const openId=requestId();assert(typeof openId==='string'&&openId.length>0&&openId!==createId);
    const opened=outcome(await api.request('POST','/api/v2/shows',{request_id:openId,action:openAction}),openId);
    assert.equal(opened.id,created.id);api.showId=created.id;
    const bootstrap=await api.request('GET','/api/v2/bootstrap');assert.equal(bootstrap.active_show?.id,created.id);
    if(Object.hasOwn(bootstrap,'active_show_error'))assert.equal(bootstrap.active_show_error,null);
    const patch=await api.request('GET','/api/v2/patch');assert.deepEqual(patch.fixtures,[]);assert.deepEqual(patch.profile_revisions,[]);assert.equal(patch.patch_revision,0);
    const runtime=await api.request('GET','/api/v2/dynamics/runtime');assert.deepEqual(runtime.definitions,[]);assert.deepEqual(runtime.instances,[]);
    complete=true;
    return {showId:created.id,setupBinding:{schema_version:1,setup_code:1,show_id:created.id,create_action_sha256:digest(createAction),open_action_sha256:digest(openAction),patch_revision:0,fixture_count:0,profile_count:0,dynamic_definition_count:0,dynamic_instance_count:0}};
  }finally{if(!complete)api.showId=previous;}
}
