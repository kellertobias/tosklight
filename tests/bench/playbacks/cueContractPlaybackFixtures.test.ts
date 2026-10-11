import {describe,expect,it,vi} from "vitest";
import type {ApiDriver} from "../core/api";
vi.mock("../core/fixtures",async()=>({expect:(await import("vitest")).expect}));
import {assignPlayback,emptyPlaybackPage,installPlaybackSequence,playbackAtSlot,runtime} from "../../cueSemanticContracts/playbackFixtures";
function apiFixture(page:number=99) {
 const definition={name:"retained",target:{type:"cue_list",cue_list_id:"retained"}};
 const current={id:String(page),revision:8,body:{number:page,name:"Retained page",assignments:{7:definition}}};
 const seedShowObject=vi.fn();
 const api={session:{desk:{id:"desk"}},request:vi.fn().mockResolvedValue({active_show:{id:"show"}}),showObjects:vi.fn().mockResolvedValue([current]),seedShowObject} as unknown as ApiDriver;
 return {api,current,seedShowObject};
}
describe("cue contract canonical sparse fixtures",()=>{
 it("writes a sequence at page99 position1000 while retaining local presentation and revision",async()=>{
 const {api,seedShowObject}=apiFixture();
 const installed=await installPlaybackSequence(api,{page:99,position:1000},[],{name:"Last position",auto_off:true});
 expect(installed.address).toEqual({page:99,position:1000});
 expect(seedShowObject.mock.calls.map(call=>call[1])).toEqual(["cue_list","playback_page"]);
 const page=seedShowObject.mock.calls[1];
 expect(page[2]).toBe("99"); expect(page[4]).toBe(8);
 expect(page[3].assignments[7].name).toBe("retained");
 expect(page[3].assignments[1000]).toMatchObject({name:"Last position",auto_off:true,target:{type:"cue_list",cue_list_id:installed.id}});
 expect(page[3].assignments[1000]).not.toHaveProperty("number");
 expect(page[3]).not.toHaveProperty("slots");
 });
 it("assigns another shared alias without writing a standalone Playback",async()=>{
 const {api,seedShowObject}=apiFixture(2);
 await assignPlayback(api,{page:2,position:7},{name:"replacement",target:{type:"cue_list",cue_list_id:"retained"}});
 expect(seedShowObject).toHaveBeenCalledExactlyOnceWith("show","playback_page","2",{number:2,name:"Retained page",assignments:{7:{name:"replacement",target:{type:"cue_list",cue_list_id:"retained"}}}},8);
 });
 it("clears page assignments with no retired bank fields",async()=>{
 const {api,seedShowObject}=apiFixture(1);
 await emptyPlaybackPage(api);
 expect(seedShowObject.mock.calls[0][3]).toEqual({number:1,name:"Retained page",assignments:{}});
 expect(seedShowObject.mock.calls[0][4]).toBe(8);
 });
 it("returns an exact recorded address, then reads shared runtime by requested identity",async()=>{
 const {api}=apiFixture(1);
 expect(await playbackAtSlot(api,7)).toEqual({page:1,position:7});
 vi.mocked(api.request).mockImplementation(async(_method,path)=>path==="/api/v2/bootstrap"?{active_show:{id:"show"}}:{projections:[{identity:{kind:"playback",page:2,position:7},target:"cue_list",runtime:{owner:{kind:"playback",page:1,position:7},enabled:true,current:{id:"cue",number:"2"},loaded:{id:"next",number:"3"},effective_next:{id:"next",number:"3"}}}]} as never);
 expect(await runtime(api,{page:2,position:7})).toMatchObject({identity:{kind:"playback",page:2,position:7},owner:{kind:"playback",page:1,position:7},current_cue_id:"cue",current_cue_number:"2",loaded_cue_id:"next",effective_next_cue_number:"3"});
 expect(api.request).toHaveBeenLastCalledWith("POST","/api/v2/playback-runtime/snapshot",{identities:[{kind:"playback",page:2,position:7}]},true,undefined,{showId:"show",deskId:"desk"});
 });
});
