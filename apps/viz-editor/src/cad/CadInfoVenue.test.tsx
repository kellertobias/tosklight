import type { PatchFixtureProjection } from "@tosklight/patch";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { CadInfoPanel } from "./CadInfoPanel";
import { CadSidePanels } from "./CadSidePanels";
import { CadToolContext, type CadTools } from "./cadTools";
import type { CadEntity } from "./types";

const mocks=vi.hoisted(()=>({snapshot:vi.fn(),venue:vi.fn(),patch:vi.fn(),numeric:vi.fn()}));
vi.mock("../document/session",()=>({documentSession:{patchSnapshot:mocks.snapshot,fixtureNotes:async()=>[],fixtureProfileUpdate:async()=>null}}));
vi.mock("../document/transport",()=>({TauriPatchTransport:class{patchFixtures=mocks.patch;}}));
vi.mock("./session",()=>({cadSession:{patchVenue:mocks.venue,setNumericTransforms:mocks.numeric}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
const wallId="11111111-1111-4111-8111-111111111111";
const doorId="22222222-2222-4222-8222-222222222222";
function fixture():PatchFixtureProjection {
	return {fixtureId:wallId,name:"Wall",profileId:"profile",profileRevision:1,location:{x:0,y:0,z:0},rotation:{x:0,y:0,z:0},splitPatches:[],multipatch:[],scenerySizeMetres:{x:6000,y:4000,z:200},sceneryOptions:{colourSrgb:"#808080",architecture:{kind:"wall",openings:[{id:doorId,offset_millimetres:1000,sill_millimetres:0,width_millimetres:900,height_millimetres:2100,opening:{kind:"door",leaves:"single",swing:"inward",hinge:"left"}}]}}} as unknown as PatchFixtureProjection;
}
function mount(selectedOpening=false, selectOpening=vi.fn()) {
	mocks.snapshot.mockResolvedValue({fixtures:[fixture()],profileRevisions:[]});
	const onError=vi.fn();
	const info=<CadInfoPanel entity={{id:wallId,logicalFixtureId:wallId,kind:"venue",name:"Wall",positionMillimetres:[0,0,0],rotationDegrees:[0,0,0],scenery:{architecture:fixture().sceneryOptions?.architecture}} as CadEntity} tab="placement" selectionCount={1} sceneRevision={42} onError={onError} />;
	render(selectedOpening?<CadToolContext.Provider value={{selectedOpening:{fixtureId:wallId,openingId:doorId},selectOpening} as unknown as CadTools}>{info}</CadToolContext.Provider>:info);
	return onError;
}
it("commits wall colour from Placement as one venue history edit",async()=>{
	mocks.venue.mockResolvedValue({changed:true});
	mount();
	const input=await screen.findByLabelText("Wall colour");
	fireEvent.change(input,{target:{value:"#135724"}});
	await waitFor(()=>expect(mocks.venue).toHaveBeenCalledTimes(1));
	expect(mocks.venue).toHaveBeenCalledWith(42,[{...fixture(),sceneryOptions:{...fixture().sceneryOptions,colourSrgb:"#135724"}}]);
	expect(mocks.patch).not.toHaveBeenCalled();
	expect(mocks.numeric).not.toHaveBeenCalled();
	expect(screen.queryByRole("dialog")).toBeNull();
});
it("edits the opening clicked in the viewport directly in Placement",async()=>{
	mocks.venue.mockResolvedValue({changed:true});
	mount(true);
	const input=await screen.findByLabelText("Opening 1 width");
	fireEvent.change(input,{target:{value:"1.4"}});
	fireEvent.keyDown(input,{key:"Enter"});
	await waitFor(()=>expect(mocks.venue).toHaveBeenCalledTimes(1));
	expect(mocks.venue.mock.calls[0][1][0].sceneryOptions.architecture.openings[0]).toMatchObject({id:doorId,width_millimetres:1400});
	expect(mocks.patch).not.toHaveBeenCalled();
});
it("restores the authoritative field and reports a rejected venue edit",async()=>{
	const error=Error("Opening extends outside its wall");
	mocks.venue.mockRejectedValue(error);
	const onError=mount();
	const input=await screen.findByLabelText("Wall colour");
	fireEvent.change(input,{target:{value:"#123456"}});
	await waitFor(()=>expect(onError).toHaveBeenCalledWith(error));
	expect(screen.getByLabelText("Wall colour")).toHaveValue("#808080");
});

it("opens Placement for venue selection and returns there when another opening is clicked", async()=>{
    mocks.snapshot.mockResolvedValue({fixtures:[fixture()],profileRevisions:[]});
    const entity={id:wallId,logicalFixtureId:wallId,kind:"venue",name:"Wall",positionMillimetres:[0,0,0],rotationDegrees:[0,0,0],scenery:{architecture:fixture().sceneryOptions?.architecture}} as CadEntity;
    const tools={annotations:[],selectedTextId:null,selectedOpening:null,selectOpening:vi.fn()} as unknown as CadTools;
    const panel=(context:CadTools)=><CadToolContext.Provider value={context}><CadSidePanels panel={null} scene={{sceneRevision:42,entities:[entity],selectedIds:[wallId]} as never} tools={context} underlayState={{} as never} defaultView="top_down" documentKey="show" printPages={{} as never} exporting={false} onExport={vi.fn()} onSelect={vi.fn()} focusedEntityId={wallId} onFocusEntity={vi.fn()} onError={vi.fn()} onNotice={vi.fn()} /></CadToolContext.Provider>;
    const view=render(panel(tools));
    await screen.findByLabelText("Wall colour");
    expect(screen.getByRole("tab",{name:"Placement"})).toHaveAttribute("aria-selected","true");
    fireEvent.click(screen.getByRole("tab",{name:"Generic"}));
    await screen.findByLabelText("Name");
    view.rerender(panel({...tools,selectedOpening:{fixtureId:wallId,openingId:doorId}}));
    await screen.findByLabelText("Opening 1 width");
    expect(screen.getByRole("tab",{name:"Placement"})).toHaveAttribute("aria-selected","true");
});

it("removes the selected opening with one venue edit and clears selection after success",async()=>{
	let accept!: (value: unknown) => void;
	mocks.venue.mockReturnValue(new Promise(resolve=>{accept=resolve;}));
	const selectOpening=vi.fn();
	mount(true,selectOpening);
	fireEvent.click(await screen.findByRole("button",{name:"Remove opening"}));
	await waitFor(()=>expect(mocks.venue).toHaveBeenCalledTimes(1));
	expect(mocks.venue.mock.calls[0][1][0].sceneryOptions.architecture.openings).toEqual([]);
	expect(selectOpening).not.toHaveBeenCalled();
	accept({changed:true});
	await waitFor(()=>expect(selectOpening).toHaveBeenCalledWith(null));
	expect(mocks.patch).not.toHaveBeenCalled();
});
it("retains the opening and selection when removal is rejected",async()=>{
	const error=Error("Wall changed; retry");
	mocks.venue.mockRejectedValue(error);
	const selectOpening=vi.fn();
	const onError=mount(true,selectOpening);
	fireEvent.click(await screen.findByRole("button",{name:"Remove opening"}));
	await waitFor(()=>expect(onError).toHaveBeenCalledWith(error));
	expect(selectOpening).not.toHaveBeenCalled();
	expect(screen.getByLabelText("Opening 1 width")).toHaveValue("0.9");
	expect(screen.getByRole("button",{name:"Remove opening"})).toBeEnabled();
});
