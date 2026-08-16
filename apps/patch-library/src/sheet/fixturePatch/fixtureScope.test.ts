import { describe, expect, it } from "vitest";
import type { FixtureDefinition } from "../../wire";
import { blankFixtureProfile } from "../fixtureProfileModel";
import { definitionMatchesScope, patchLayerIsVisible } from "./controller";

function definition(
	type: string,
	policy: "dmx" | "visual_only" = "dmx",
): FixtureDefinition {
	const profile = blankFixtureProfile();
	profile.fixture_type = type;
	profile.patch_policy = policy;
	return {
		schema_version: 2,
		id: profile.id,
		revision: profile.revision,
		manufacturer: "Generic",
		device_type: type,
		name: type,
		model: type,
		mode: profile.modes[0].name,
		footprint: policy === "visual_only" ? 0 : 1,
		heads: [],
		color_calibration: null,
		physical: {},
		model_asset: null,
		icon_asset: null,
		hazardous: false,
		direct_control_protocols: [],
		signal_loss_policy: { type: "hold_last" },
		safe_values: {},
		profile_id: profile.id,
		mode_id: profile.modes[0].id,
		profile_snapshot: profile,
	};
}

describe("Visualizer patch screen scopes", () => {
	it("separates DMX lights, venue models, and effect devices", () => {
		const wash = definition("wash");
		const venue = definition("venue", "visual_only");
		const laser = definition("laser");
		const particle = definition("effect");
		const kabuki = definition("scenery");
		const haze = definition("fogger");

		expect(definitionMatchesScope(wash, "dmx")).toBe(true);
		expect(definitionMatchesScope(venue, "venue")).toBe(true);
		for (const effect of [laser, particle, kabuki, haze])
			expect(definitionMatchesScope(effect, "effects")).toBe(true);

		expect(definitionMatchesScope(venue, "dmx")).toBe(false);
		expect(definitionMatchesScope(laser, "dmx")).toBe(false);
		expect(definitionMatchesScope(wash, "venue")).toBe(false);
		expect(definitionMatchesScope(wash, "effects")).toBe(false);
	});

	it("shows only empty layers or layers containing the current screen's fixtures", () => {
		const fixtures = [
			{ layer_id: "lights", definition: definition("wash") },
			{ layer_id: "venue", definition: definition("venue", "visual_only") },
			{ layer_id: "effects", definition: definition("laser") },
		];

		expect(patchLayerIsVisible("empty", fixtures, "dmx")).toBe(true);
		expect(patchLayerIsVisible("lights", fixtures, "dmx")).toBe(true);
		expect(patchLayerIsVisible("venue", fixtures, "dmx")).toBe(false);
		expect(patchLayerIsVisible("venue", fixtures, "venue")).toBe(true);
		expect(patchLayerIsVisible("lights", fixtures, "venue")).toBe(false);
		expect(patchLayerIsVisible("effects", fixtures, "effects")).toBe(true);
	});

	it("filters Media Add Server by fixture type rather than CITP metadata or manufacturer", () => {
		const tosklight = definition("media_server");
		tosklight.manufacturer = "ToskLight";
		const resolume = definition("media_server");
		resolume.manufacturer = "Resolume";
		resolume.direct_control_protocols = ["citp"];

		expect(definitionMatchesScope(tosklight, "media")).toBe(true);
		expect(definitionMatchesScope(resolume, "media")).toBe(true);
		expect(definitionMatchesScope(definition("wash"), "media")).toBe(false);
	});
});
