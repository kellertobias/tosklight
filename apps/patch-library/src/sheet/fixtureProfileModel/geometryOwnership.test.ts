import { describe, expect, it } from "vitest";
import { blankFixtureProfile, blankHead, blankMode } from "./defaults";
import {
	geometryTemplate,
	liftGeometryBindings,
	modeGeometry,
} from "./geometry";
import { validateProfile } from "./validation";

function importedMover() {
	const profile = blankFixtureProfile();
	profile.manufacturer = "ACME";
	profile.name = "Imported mover geometry regression";
	profile.revision = 1;
	profile.modes = [
		blankMode("Standard"),
		blankMode("Extended"),
		blankMode("Different geometry owner"),
	];
	for (const mode of profile.modes.slice(0, 2))
		mode.heads = [
			{ ...blankHead(), name: "Yoke", master_shared: true },
			{ ...blankHead(1), name: "Head" },
		];
	profile.modes[2].heads = [{ ...blankHead(), name: "Unrelated" }];
	return profile;
}

describe("shared fixture geometry ownership", () => {
	it("saves an unmodified moving-head template without global heads and reopens every mode's owners", () => {
		const profile = importedMover();
		profile.geometry = geometryTemplate(
			"moving_head",
			profile.modes[0].heads.map((head) => head.id),
		);
		const saved = liftGeometryBindings(profile);
		expect(
			saved.geometry?.emitters.every((emitter) => emitter.head_id == null),
		).toBe(true);
		for (const mode of saved.modes.slice(0, 2)) {
			const bound = modeGeometry(saved, mode);
			expect(bound.emitters.map((emitter) => emitter.head_id)).toEqual(
				mode.heads.map((head) => head.id),
			);
		}
		expect(modeGeometry(saved, saved.modes[2]).emitters).toEqual([]);
		const reopened = JSON.parse(JSON.stringify(saved));
		expect(validateProfile(reopened)).toEqual([]);
		expect(
			modeGeometry(reopened, reopened.modes[1]).emitters.map(
				(emitter) => emitter.head_id,
			),
		).toEqual(saved.modes[1].heads.map((head) => head.id));
		expect(liftGeometryBindings(saved)).toBe(saved);
	});
	it("prunes replaced graph bindings without changing heads, channels or legacy mode geometry", () => {
		const profile = importedMover();
		const old = profile.geometry!;
		profile.modes[0].emitter_heads = [
			{ emitter_id: old.emitters[0].id, head_id: profile.modes[0].heads[0].id },
		];
		profile.modes[0].motion_attributes = [
			{ node_id: old.nodes[0].id, attribute: "pan" },
		];
		profile.modes[2].geometry = geometryTemplate(
			"fixed",
			profile.modes[2].heads.map((head) => head.id),
		);
		const originalModes = structuredClone(profile.modes);
		profile.geometry = geometryTemplate(
			"moving_head",
			profile.modes[0].heads.map((head) => head.id),
		);
		const saved = liftGeometryBindings(profile);
		expect(
			saved.modes[0].emitter_heads?.some(
				(binding) => binding.emitter_id === old.emitters[0].id,
			),
		).toBe(false);
		expect(
			saved.modes[0].motion_attributes?.some(
				(binding) => binding.node_id === old.nodes[0].id,
			),
		).toBe(false);
		expect(
			saved.modes.map((mode) => [mode.heads, mode.channels, mode.geometry]),
		).toEqual(
			originalModes.map((mode) => [mode.heads, mode.channels, mode.geometry]),
		);
	});
	it("identifies invalid emitter geometry and mode ownership locally before saving", () => {
		const profile = importedMover();
		const emitter = profile.geometry!.emitters[0];
		emitter.name = "Lens";
		emitter.node_id = "missing";
		emitter.beam_angle_degrees = 30;
		emitter.field_angle_degrees = 20;
		profile.modes[0].emitter_heads = [
			{ emitter_id: emitter.id, head_id: "missing-head" },
		];
		const errors = validateProfile(profile).join(" ");
		expect(errors).toMatch(/Lens.*Geometry part/i);
		expect(errors).toMatch(/Lens.*Field angle/i);
		expect(errors).toMatch(/Standard.*Lens.*Logical head/i);
	});
});
