import { describe, expect, it, vi } from "vitest";
import { BrowserVirtualPlaybacks, panePlaybackAddress } from "./virtualPlaybackScenario";
import type { ApiDriver } from "../core/api";

describe("Playback pane canonical addresses", () => {
    it("uses the same position on every page instead of deriving a page bank", () => {
        expect(panePlaybackAddress(1, 1)).toEqual({ page: 1, position: 401 });
        expect(panePlaybackAddress(1, 99)).toEqual({ page: 99, position: 401 });
        expect(panePlaybackAddress(600, 99)).toEqual({ page: 99, position: 1000 });
    });
    it.each([[0, 1], [1, 0], [1, 100], [601, 1], [1.5, 1]])("rejects invalid cell/page %s/%s", (cell, page) => {
        expect(() => panePlaybackAddress(cell, page)).toThrow();
    });
    it("reads the requested exact-address shared runtime using the canonical protocol", async () => {
        const identity = { kind: "playback", page: 99, position: 1000 };
        const projection = { identity, target: "cue_list", runtime: { enabled: true, current: { number: "1" } } };
        const request = vi.fn().mockResolvedValue({ projections: [projection] });
        const api = { request, session: { desk: { id: "desk" } } } as unknown as ApiDriver;
        const helper = new BrowserVirtualPlaybacks(api, {} as never, {} as never, {} as never, {} as never, () => "show");
        const runtime = await (helper as unknown as { runtime(address: {page:number;position:number}):Promise<unknown> }).runtime({page:99,position:1000});
        expect(runtime).toEqual(projection);
        expect(request).toHaveBeenCalledWith("POST", "/api/v2/playback-runtime/snapshot", { identities: [identity] }, true, undefined, { showId: "show", deskId: "desk" });
    });
});

describe("Solo management renderer contract",()=>{
 it("deletes the named region without obsolete per-region name inputs",async()=>{
  class Locator { readonly _apiName="Locator"; constructor(readonly name:string){} async _expect(){return {matches:true};} }
  const permitted=new Set(["Settings","Solo Regions","Delete Touring pair","Close settings"]);
  const getByRole=vi.fn((_role:string,options:{name:string})=>{
   if(!permitted.has(options.name)) throw new Error(`No rendered control ${options.name}`);
   return new Locator(options.name);
  });
  const settings={getByRole,getByLabel:()=>{throw new Error("Renderer has only shared Region name input")}};
  const click=vi.fn();
  const api={request:vi.fn().mockResolvedValue({regions:[]}),session:{desk:{id:"desk"}}} as unknown as ApiDriver;
  const helper=new BrowserVirtualPlaybacks(api,{getByRole:()=>settings} as never,{click} as never,{locatorFor:()=>({getByRole})} as never,{} as never,()=>"show");
  await helper.deleteSoloRegion({} as never,"Touring pair");
  expect(getByRole).toHaveBeenCalledWith("button",{name:"Delete Touring pair",exact:true});
  expect(click).toHaveBeenCalledTimes(4);
 });
});
