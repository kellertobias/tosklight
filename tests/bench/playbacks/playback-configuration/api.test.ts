import {describe, expect, it, vi} from "vitest";
import {activePlayback, definition, installPlaybacks, playbackAt, poolAction} from "./api";
import {hasTemporaryRuntime, hasSwapRuntime} from "./observations";
import type {ApiDriver} from "../../core/api";

describe("canonical Playback benchmark helpers", () => {
 it("stores sparse assignment definitions on the page and reads their exact address", async () => {
  let page = {id:"1",revision:3,body:{number:1,name:"Main",assignments:{}}};
  const seedShowObject = vi.fn(async (_show:string,kind:string,id:string,body:any) => { expect(kind).toBe("playback_page"); expect(id).toBe("1"); page={...page,body}; });
  const api={request:vi.fn(async()=>({active_show:{id:"show"}})),showObjects:vi.fn(async()=>[page]),seedShowObject} as unknown as ApiDriver;
  const sequence=definition("Sequence",{type:"cue_list",cue_list_id:"cue"});
  const group=definition("Group",{type:"group",group_id:"front"});
  await installPlaybacks(api,[sequence,group]);
  expect(seedShowObject).toHaveBeenCalledTimes(1);
  expect(page.body.assignments).toEqual({"1":sequence,"2":group});
  expect(sequence).not.toHaveProperty("number");
  expect((await playbackAt(api,1,2)).body).toEqual(group);
 });
 it("executes repeated live GO independently and keeps held press/release source stable", async () => {
  const request=vi.fn(async(_method:string,path:string)=>path==="/api/v2/bootstrap"?{active_show:{id:"show"}}:{});
  const api={session:{desk:{id:"desk"}},request} as unknown as ApiDriver;
  await poolAction(api,2,"go"); await poolAction(api,2,"go");
  await poolAction(api,2,"button",{button:3,pressed:true});
  await poolAction(api,2,"button",{button:3,pressed:false});
  const actions=request.mock.calls.filter(call=>call[1]==="/api/v2/playback-actions") as unknown as any[][];
  expect(actions).toHaveLength(4);
  expect(actions[0][2]).toEqual(actions[1][2]);
  for(const action of actions) {
   expect(action[2].address).toEqual({kind:"playback",page:1,position:2});
   expect(action[2]).not.toHaveProperty("request_id");
   expect(action[2]).not.toHaveProperty("surface");
   expect(action[5]).toEqual({showId:"show",deskId:"desk"});
  }
  expect(actions[2][2].control_source).toEqual({surface_id:"main",control_id:19});
  expect(actions[3][2].control_source).toEqual(actions[2][2].control_source);
  expect(actions[2][2].action).toEqual({type:"configured_button",number:3,pressed:true});
  expect(actions[3][2].action.pressed).toBe(false);
 });
 it("reads runtime ownership by full page and position across same-position aliases", async () => {
  const active = [{playback_address:{page:2,position:2},temporary_active:true,swap_active:true},{playback_address:{page:1,position:2},temporary_active:false,swap_active:false}];
  const api={request:vi.fn(async()=>({active}))} as unknown as ApiDriver;
  expect(await activePlayback(api,2)).toBe(active[1]);
  expect(hasTemporaryRuntime({active},2)).toBe(false);
  expect(hasSwapRuntime({active},2)).toBe(false);
 });

});
