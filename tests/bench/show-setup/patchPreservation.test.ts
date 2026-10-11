import { describe, expect, it, vi } from "vitest";
import type {
	PatchFixtureInput,
	PatchFixtureProjection,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import { ApiDriver } from "../core/api";
import {
	setFixtureAddressThroughApi,
	unpatchFixture,
} from "../../support/operator/patch";

const fixture: PatchFixtureProjection = {
	fixture_id: "fixture",
	fixture_revision: 4,
	fixture_number: 9004,
	virtual_fixture_number: null,
	name: "Preserve fixture",
	profile_id: "profile",
	profile_revision: 2,
	mode_id: "mode",
	split_patches: [{ split: 0, universe: 50, address: 141 }],
	layer_id: "layer",
	direct_control: null,
	internal_bindings: { library: "fixture-library", output: "output-bind" },
	location: { x: 1250, y: -2500, z: 3000 },
	rotation: { x: 10, y: 20, z: 30 },
	scenery_size_metres: { x: 2, y: 3, z: 4 },
	scenery_options: null,
	model_scale: 1.3,
	note: "Operator note",
	position_master: "point",
	multipatch: [],
	logical_heads: [],
	freeze_targets: [],
	group_masters_enabled: false,
	grand_master_enabled: false,
	invert_pan: true,
	invert_tilt: true,
	position_calibration: {
		revision: 2,
		quality: "measured",
		pan_zero_degrees: 12,
		tilt_zero_degrees: 34,
	},
	color_calibration: { version: 1, revision: 3, paths: [] },
	bracket_angle: 27,
	shaper_angle: 45,
	installed_appearance: {
		light_source: { type: "halogen" },
		color_temperature_kelvin: 3200,
		luminous_output_lumens: 7000,
		gel: {
			type: "custom",
			name: "Amber",
			color_srgb: "#ffaa00",
			note: "Retain",
		},
		shaper_angles_degrees: [10, 20, 30, 40],
	},
	move_in_black_enabled: true,
	move_in_black_delay_millis: 450,
	highlight_overrides: [],
};

describe("Patch acceptance edits preserve authored fixture semantics", () => {
	it.each([
		"address",
		"unpatch",
	] as const)("changes only split addresses for %s", async (operation) => {
		const api = new ApiDriver("http://desk.local");
		let requestBody: { fixtures: PatchFixtureInput[] } | undefined;
		vi.spyOn(api, "request").mockImplementation(
			async (method, path, body, _auth, revision, context) => {
				if (path === "/api/v2/bootstrap")
					return { active_show: { id: "show" } };
				if (path === "/api/v2/patch")
					return { patch_revision: 19, fixtures: [fixture] };
				expect(method).toBe("POST");
				expect(path).toBe("/api/v2/patch/fixtures");
				expect(revision).toBe(19);
				expect(context).toEqual({ showId: "show" });
				requestBody = body as { fixtures: PatchFixtureInput[] };
				return { changed: true };
			},
		);
		if (operation === "address")
			await setFixtureAddressThroughApi(api, fixture.fixture_id, "51.201");
		else await unpatchFixture(api, fixture.fixture_id);
		const { fixture_revision, logical_heads, freeze_targets, ...input } =
			fixture;
		expect(requestBody!.fixtures).toEqual([
			{
				...input,
				split_patches: [
					{
						split: 0,
						universe: operation === "address" ? 51 : null,
						address: operation === "address" ? 201 : null,
					},
				],
			},
		]);
		expect(fixture.split_patches).toEqual([
			{ split: 0, universe: 50, address: 141 },
		]);
	});
});
