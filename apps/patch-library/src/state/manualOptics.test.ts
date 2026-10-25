import { describe, expect, it } from "vitest";
import type { PatchProfileRevision } from "../contracts";
import type {
	FixtureDefinition,
	InstalledFixtureAppearance,
	PatchedFixture,
} from "../wire";
import { patchedFixtureCandidate, projectionToPatchedFixture } from "./model";

const definition = {
	id: "profile-fresnel",
	revision: 1,
	name: "Fresnel",
	mode: "Manual",
	profile_id: "profile-fresnel",
	mode_id: "mode-manual",
	footprint: 0,
	heads: [],
	physical: {},
	profile_snapshot: null,
} as unknown as FixtureDefinition;

const profile = {
	profileId: "profile-fresnel",
	profileRevision: 1,
	contentDigest: "",
	manufacturer: "Generic",
	name: "Fresnel",
	fixtureType: "wash",
	patchPolicy: "visual_only",
	referencedModes: [],
	profileSnapshot: null,
} as unknown as PatchProfileRevision;

function appearance(
	manual?: InstalledFixtureAppearance["manual_optics"],
): InstalledFixtureAppearance {
	return {
		light_source: { type: "profile_default" },
		color_temperature_kelvin: null,
		luminous_output_lumens: null,
		gel: { type: "open_white" },
		shaper_angles_degrees: [0, 0, 0, 0],
		...(manual === undefined ? {} : { manual_optics: manual }),
	};
}

function fixture(
	root: InstalledFixtureAppearance,
	copy: InstalledFixtureAppearance,
): PatchedFixture {
	return {
		fixture_id: "front",
		fixture_number: 1,
		virtual_fixture_number: null,
		name: "Front Fresnel",
		definition,
		universe: null,
		address: null,
		split_patches: [],
		layer_id: "default",
		direct_control: null,
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
		logical_heads: [],
		installed_appearance: root,
		multipatch: [
			{
				id: "copy",
				name: "Copy",
				universe: null,
				address: null,
				split_patches: [],
				location: { x: 1, y: 0, z: 0 },
				rotation: { x: 0, y: 0, z: 0 },
				installed_appearance: copy,
			},
		],
		move_in_black_enabled: true,
		move_in_black_delay_millis: 0,
		highlight_overrides: {},
	};
}

function read(input: ReturnType<typeof patchedFixtureCandidate>["input"]) {
	return projectionToPatchedFixture(
		{ ...input, fixtureRevision: 1, logicalHeads: [] },
		profile,
		() => definition,
	);
}

describe("Architect manual optics round trip", () => {
	it("preserves independent root and copy optics through source and gel edits without aliasing", () => {
		const root = appearance({
			zoom_degrees: 48,
			shaper_blades: [0.1, 0.2, 0.3, 0.4],
		});
		const copy = appearance({
			zoom_degrees: null,
			shaper_blades: [0.4, 0.3, 0.2, 0.1],
		});
		const { input } = patchedFixtureCandidate(fixture(root, copy));
		expect(input.installedAppearance?.manualOptics).toEqual({
			zoomDegrees: 48,
			shaperBlades: [0.1, 0.2, 0.3, 0.4],
		});
		expect(input.multipatch[0].installedAppearance?.manualOptics).toEqual({
			zoomDegrees: null,
			shaperBlades: [0.4, 0.3, 0.2, 0.1],
		});
		input.installedAppearance!.lightSource = { type: "halogen" };
		input.multipatch[0].installedAppearance!.gel = {
			type: "custom",
			name: "Warm",
			colorSrgb: "#FFEEDD",
			note: null,
		};
		const returned = read(input);
		expect(returned.installed_appearance?.manual_optics).toEqual(
			root.manual_optics,
		);
		expect(returned.multipatch![0].installed_appearance?.manual_optics).toEqual(
			copy.manual_optics,
		);
		expect(returned.installed_appearance?.light_source).toEqual({
			type: "halogen",
		});
		expect(returned.multipatch![0].installed_appearance?.gel).toMatchObject({
			type: "custom",
			name: "Warm",
		});
		input.installedAppearance!.manualOptics!.shaperBlades[0] = 1;
		expect(root.manual_optics!.shaper_blades[0]).toBe(0.1);
		expect(returned.installed_appearance!.manual_optics!.shaper_blades[0]).toBe(
			0.1,
		);
	});

	it("does not invent manual fields for legacy root or copy appearances", () => {
		const { input } = patchedFixtureCandidate(
			fixture(appearance(), appearance()),
		);
		expect(input.installedAppearance).not.toHaveProperty("manualOptics");
		expect(input.multipatch[0].installedAppearance).not.toHaveProperty(
			"manualOptics",
		);
		const returned = read(input);
		expect(returned.installed_appearance).not.toHaveProperty("manual_optics");
		expect(returned.multipatch![0].installed_appearance).not.toHaveProperty(
			"manual_optics",
		);
	});

	it("keeps explicit frontend null distinct from an omitted manual block", () => {
		const { input } = patchedFixtureCandidate(
			fixture(appearance(null), appearance(null)),
		);
		expect(input.installedAppearance?.manualOptics).toBeNull();
		expect(input.multipatch[0].installedAppearance?.manualOptics).toBeNull();
		const returned = read(input);
		expect(returned.installed_appearance?.manual_optics).toBeNull();
		expect(returned.multipatch![0].installed_appearance?.manual_optics).toBeNull();
	});
});
