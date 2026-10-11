import { describe, expect, it, vi } from "vitest";
import type { ApiDriver } from "../core/api";
vi.mock("../core/fixtures", async () => ({expect:(await import("vitest")).expect}));
import { captureMasks, scopeContains } from "../../preloadVirtualPlaybackContracts/scopeCases";
import { activeAssignedPlayback, copyPageAssignments, installOnCurrentShow, paneAction } from "../../preloadVirtualPlaybackContracts/showFixtures";
import { playbackPendingObservation } from "../../preloadVirtualPlaybackContracts/preloadSupport";

describe("Preload executable contracts use one sparse Playback model",()=>{
 it("covers all eight programmer/position eligibility combinations and restricts pages independently",()=>{
  const masks=captureMasks();
  expect(masks).toHaveLength(8);
  expect(masks.map(mask=>[mask.programmer,scopeContains(mask.playbackScope,{page:1,position:51}),scopeContains(mask.playbackScope,{page:1,position:52})])).toEqual([
   [false,false,false],[true,false,false],[false,true,false],[true,true,false],
   [false,false,true],[true,false,true],[false,true,true],[true,true,true]]);
  expect(scopeContains(masks[0].playbackScope,{page:99,position:51})).toBe(true);
 });
 it("preserves repeated action order and distinct held control provenance",()=>{
  const source={surface_id:"main:one",control_id:9};
  const other={surface_id:"pane:two",control_id:9};
  const actions=[{address:{page:1,position:9},action:"temporary_on",control_source:source},{address:{page:99,position:9},action:"go",control_source:other},{address:{page:1,position:9},action:"temporary_off",control_source:source}];
  expect(playbackPendingObservation({preload_playback_pending:actions})).toEqual(actions.map(action=>[action.address,action.action,action.control_source]));
 });
 it("authors definitions only inside sparse page assignments, including configured main aliases",async()=>{
  const seedShowObject=vi.fn();
  const api={request:vi.fn().mockResolvedValue({active_show:{id:"show"}}),showObjects:vi.fn().mockResolvedValue([{id:"1",body:{name:"Existing",assignments:{99:{name:"retained"}}},revision:7}]),seedShowObject} as unknown as ApiDriver;
  await installOnCurrentShow(api,"show",{3:"fixture"},[{position:61,fixture:3,levels:[.6]}],{1:61});
  expect(seedShowObject.mock.calls.map(call=>call[1])).toEqual(["cue_list","playback_page"]);
  const body=seedShowObject.mock.calls[1][3];
  expect(Object.keys(body.assignments)).toEqual(["1","61","99"]);
  expect(body.assignments[1]).toEqual(body.assignments[61]);
  expect(body.assignments[61]).not.toHaveProperty("number");
  expect(body.assignments[99]).toEqual({name:"retained"});
  expect(seedShowObject.mock.calls[1][4]).toBe(7);
  expect(body).not.toHaveProperty("slots");
 });
 it("copies a source into the same position on page99 without a bank-derived identity",async()=>{
  const definition={name:"source",buttons:["go","flash","none"],has_fader:true};
  const seedShowObject=vi.fn();
  const api={request:vi.fn().mockResolvedValue({active_show:{id:"show"}}),showObject:vi.fn().mockResolvedValue({body:{assignments:{61:definition}}}),showObjects:vi.fn().mockResolvedValue([{id:"1",body:{assignments:{61:definition}},revision:1}]),seedShowObject} as unknown as ApiDriver;
  await copyPageAssignments(api,99,{401:61});
  expect(seedShowObject).toHaveBeenCalledWith("show","playback_page","99",{number:99,name:"Page 99",assignments:{401:{...definition,buttons:["go","none","none"],button_count:1,has_fader:false}}},0);
 });
 it("queries the requested alias identity and preserves authoritative owner runtime",async()=>{
  const request=vi.fn().mockImplementation(async(_method:string,path:string)=>path==="/api/v2/bootstrap"?{active_show:{id:"show"}}:{projections:[{identity:{kind:"playback",page:99,position:401},target:"cue_list",runtime:{enabled:true,current:{number:"2"},owner:{kind:"playback",page:1,position:61},activated_at:"now"}}]});
  const api={request,session:{desk:{id:"desk"}}} as unknown as ApiDriver;
  expect(await activeAssignedPlayback(api,{page:99,position:401})).toMatchObject({identity:{kind:"playback",page:99,position:401},owner:{kind:"playback",page:1,position:61},enabled:true,current_cue_number:"2"});
  expect(request).toHaveBeenLastCalledWith("POST","/api/v2/playback-runtime/snapshot",{identities:[{kind:"playback",page:99,position:401}]},true,undefined,{showId:"show",deskId:"desk"});
 });
 it("sends a pane action through the same exact-address action authority",async()=>{
  const playbackAction=vi.fn().mockResolvedValue({outcome:{status:"applied"}});
  await paneAction({playbackAction} as unknown as ApiDriver,{page:99,position:1000},"toggle");
  expect(playbackAction).toHaveBeenCalledExactlyOnceWith({page:99,position:1000},"toggle",{});
 });
});
