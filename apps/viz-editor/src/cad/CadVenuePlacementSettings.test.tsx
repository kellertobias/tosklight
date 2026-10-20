import type { PatchFixtureProjection } from "@tosklight/patch";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { CadVenuePlacementSettings } from "./CadVenuePlacementSettings";

const windowId = "window-stable-id";
const doorId = "door-stable-id";
function wall(): PatchFixtureProjection {
	return {
		fixtureId: "wall-id", name: "Rear wall", profileId: "profile", profileRevision: 4,
		location: {x:1000,y:2000,z:0}, rotation: {x:0,y:0,z:30},
		scenerySizeMetres: {x:8000,y:4000,z:200},
		sceneryOptions: {
			colourSrgb: "#808080", chainTop: "direct",
			architecture: {kind:"wall",ceiling_height_millimetres:4000,openings:[
				{id:windowId,offset_millimetres:500,sill_millimetres:900,width_millimetres:1000,height_millimetres:1400,opening:{kind:"window",shape:"rectangular"}},
				{id:doorId,offset_millimetres:3500,sill_millimetres:0,width_millimetres:900,height_millimetres:2100,opening:{kind:"door",leaves:"single",swing:"inward",hinge:"left"}},
			]},
		},
	} as unknown as PatchFixtureProjection;
}
function mount(selectedOpeningId?: string) {
	const written = vi.fn();
	function Harness() {
		const [fixture,setFixture] = useState(wall);
		return <CadVenuePlacementSettings fixture={fixture} selectedOpeningId={selectedOpeningId} onWrite={async(next)=>{written(next);setFixture(next);}} />;
	}
	render(<Harness />);
	return written;
}
function commit(label:string,value:string) {
	const input=screen.getByLabelText(label);
	fireEvent.change(input,{target:{value}});
	fireEvent.keyDown(input,{key:"Enter"});
}
async function settled() {
	await waitFor(()=>expect(screen.getByRole("group",{name:"Venue construction"})).not.toBeDisabled());
}
afterEach(cleanup);

it("edits wall colour directly in Placement and preserves the complete wall assembly", async()=>{
	const written=mount();
	expect(screen.queryByRole("button",{name:"Edit venue element"})).toBeNull();
	expect(screen.queryByLabelText("Position X")).toBeNull();
	fireEvent.change(screen.getByLabelText("Wall colour"),{target:{value:"#214365"}});
	await settled();
	expect(written).toHaveBeenCalledTimes(1);
	expect(written.mock.calls[0][0]).toEqual({...wall(),sceneryOptions:{...wall().sceneryOptions,colourSrgb:"#214365"}});
	fireEvent.click(screen.getByRole("button",{name:"Door 2"}));
	expect(screen.getByRole("heading",{name:"Door settings"})).toBeTruthy();
});

it("edits the clicked door dimensions, leaves, swing and hinge without changing its window sibling", async()=>{
	const written=mount(doorId);
	expect(screen.getByRole("heading",{name:"Door settings"})).toBeTruthy();
	commit("Opening 2 width","1.2"); await settled();
	commit("Opening 2 height","2.4"); await settled();
	for (const [key,value] of [["leaves","double"],["swing","outward"],["hinge","right"]]) {
		fireEvent.change(screen.getByLabelText(`Door 2 ${key}`),{target:{value}}); await settled();
	}
	const last=written.mock.lastCall?.[0] as PatchFixtureProjection;
	const architecture=last.sceneryOptions?.architecture;
	if(architecture?.kind!=="wall") throw Error("wall missing");
	expect(architecture.openings[0]).toEqual((wall().sceneryOptions?.architecture as typeof architecture).openings[0]);
	expect(architecture.openings[1]).toMatchObject({id:doorId,width_millimetres:1200,height_millimetres:2400,opening:{kind:"door",leaves:"double",swing:"outward",hinge:"right"}});
	expect(last.location).toEqual(wall().location);
	expect(last.rotation).toEqual(wall().rotation);
});

it("keeps the clicked round window physically valid when its size changes", async()=>{
	const written=mount(windowId);
	fireEvent.change(screen.getByLabelText("Window 1 shape"),{target:{value:"round"}}); await settled();
	commit("Opening 1 width","1.6"); await settled();
	const last=written.mock.lastCall?.[0] as PatchFixtureProjection;
	const architecture=last.sceneryOptions?.architecture;
	if(architecture?.kind!=="wall") throw Error("wall missing");
	expect(architecture.openings[0]).toMatchObject({id:windowId,width_millimetres:1600,height_millimetres:1600,sill_millimetres:900,opening:{kind:"window",shape:"round"}});
});

it("removes only the selected door and preserves the rest of its wall",async()=>{
	const written=mount(doorId);
	fireEvent.click(screen.getByRole("button",{name:"Remove opening"}));
	await settled();
	const original=wall();
	const architecture=original.sceneryOptions!.architecture!;
	if(architecture.kind!=="wall") throw Error("wall missing");
	expect(written).toHaveBeenCalledExactlyOnceWith({...original,sceneryOptions:{...original.sceneryOptions,architecture:{...architecture,openings:[architecture.openings[0]]}}});
});
