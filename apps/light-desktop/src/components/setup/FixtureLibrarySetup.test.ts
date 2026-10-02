import { describe, expect, it } from "vitest";
import { parseHeadDrafts } from "./FixtureLibrarySetup";
import { fixtureAttributeName } from "./fixtureLibrary/definitions";

describe("fixture library editor", () => {
	it("maps physical strobe into the canonical Shutter / Strobe control", () => {
		expect(fixtureAttributeName("Strobe")).toBe("shutter");
	});

	it("builds sequential multi-head channels with physical and gobo metadata", () => {
		const result = parseHeadDrafts([
			{ name: "Master", master: true, channels: "dimmer,pan:16[-270,270,deg]" },
			{
				name: "Layer 1",
				master: false,
				channels: "gobo{Open=0-31|Dots=32-63},tilt:16[-135,135,deg]",
			},
		]);
		expect(result.footprint).toBe(6);
		expect(result.heads.map((head) => [head.name, head.shared])).toEqual([
			["Master", true],
			["Layer 1", false],
		]);
		expect(
			result.heads[0].parameters[1].components.map(
				(component) => component.offset,
			),
		).toEqual([1, 2]);
		expect(result.heads[0].parameters[1].metadata).toMatchObject({
			physical_min: -270,
			physical_max: 270,
			unit: "deg",
		});
		expect(result.heads[1].parameters[0].capabilities).toEqual([
			{ name: "Open", dmx_from: 0, dmx_to: 31, preset_family: "gobo" },
			{ name: "Dots", dmx_from: 32, dmx_to: 63, preset_family: "gobo" },
		]);
		expect(
			result.heads[1].parameters[1].components.map(
				(component) => component.offset,
			),
		).toEqual([4, 5]);
	});

	// GDTF byte/physical contracts now live in the canonical Rust reader tests. The desktop
	// invokes its typed preview/import transport instead of converting through flat definitions.
});
