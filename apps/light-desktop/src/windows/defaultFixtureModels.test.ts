import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../api/types";
import {
	chooseDefaultModelName,
	DEFAULT_MODEL_NAMES,
	defaultModelUrl,
	fixtureModelSource,
} from "./defaultFixtureModels";

/**
 * A fixture with just enough of a definition for the rule to read.
 *
 * Deliberately carries a misleading manufacturer, name and model on every case below: the rule
 * must reach the same answer whatever the fixture is called, and a test that used honest names
 * would pass even if the name-matching came back.
 */
function fixture(
	deviceType: string,
	attributes: string[],
	modelAsset: string | null = null,
): PatchedFixture {
	return {
		fixture_id: "f1",
		definition: {
			id: "d1",
			revision: 1,
			manufacturer: "Nonsense Lighting",
			name: "Product 9000",
			model: "XYZ",
			device_type: deviceType,
			model_asset: modelAsset,
			heads: [
				{
					index: 0,
					name: "main",
					shared: false,
					parameters: attributes.map((attribute) => ({
						attribute,
						components: [{ offset: 0, byte_order: "msb_first" }],
						default: 0,
						virtual_dimmer: false,
						capabilities: [],
					})),
				},
			],
		},
	} as unknown as PatchedFixture;
}

const rgb = ["dimmer", "color.red", "color.green", "color.blue"];
const movingGobo = ["dimmer", "pan", "tilt", "gobo.1"];

describe("the shipped body a fixture falls back to", () => {
	/**
	 * The cases the native renderer's own tests pin, answered here by name.
	 *
	 * The two rules live in two languages and can only be kept honest by asserting the same
	 * answers on both sides. A fixture drawn one way by the renderer and another by the desk is
	 * the same fixture looking like two things.
	 *
	 * @see crates/viz/project/src/default_model.rs
	 */
	it("agrees with the native renderer, case for case", () => {
		// A declared type wins over the channel set: an RGB blinder is still a blinder, and a
		// static profile is not a moving head.
		expect(chooseDefaultModelName(fixture("blinder", rgb))).toBe(
			"blinder-4-cell",
		);
		expect(chooseDefaultModelName(fixture("profile", ["dimmer"]))).toBe(
			"profile-spot",
		);
		expect(
			chooseDefaultModelName(
				fixture("profile moving head", ["dimmer", "pan", "tilt"]),
			),
		).toBe("moving-head-profile");

		// A type nobody shipped still lands on the channel rules.
		expect(chooseDefaultModelName(fixture("something new", movingGobo))).toBe(
			"moving-head-profile",
		);

		// Subtractive mixing is still colour mixing: a CMY head is not a gobo spot.
		expect(
			chooseDefaultModelName(
				fixture("something new", [
					"dimmer",
					"pan",
					"tilt",
					"color.cyan",
					"color.magenta",
					"color.yellow",
				]),
			),
		).toBe("moving-head-led-wash-400");

		// A laser's pattern-position channels must not read as a yoke.
		expect(chooseDefaultModelName(fixture("laser", ["dimmer", "pan", "tilt"]))).toBe(
			"show-laser",
		);
		// Nor a hazer's.
		expect(chooseDefaultModelName(fixture("hazer", ["fog"]))).toBe("hazer");
		expect(chooseDefaultModelName(fixture("scanner", ["dimmer", "pan"]))).toBe(
			"scanner-mirror-spot",
		);
		expect(chooseDefaultModelName(fixture("strip light", rgb))).toBe(
			"led-strip-rgbcct-1000",
		);
		expect(chooseDefaultModelName(fixture("par", rgb))).toBe("led-par-x-in-1");
		expect(chooseDefaultModelName(fixture("par", ["dimmer"]))).toBe(
			"par-64-short-nose-black",
		);
		expect(chooseDefaultModelName(fixture("moving wash", rgb))).toBe(
			"moving-head-led-wash-400",
		);

		// Nothing but a level.
		expect(chooseDefaultModelName(fixture("", ["dimmer"]))).toBe(
			"fresnel-barn-doors",
		);
	});

	/**
	 * The rule that was actually broken. The desk used to match on manufacturer and product name,
	 * so renaming a profile changed the picture and disagreed with the renderer about one fixture.
	 */
	it("reaches the same answer whatever the fixture is called", () => {
		const named = fixture("moving profile", movingGobo);
		const renamed = {
			...named,
			definition: {
				...named.definition,
				manufacturer: "Totally Different",
				name: "A7",
				model: "Source Four",
			},
		} as PatchedFixture;
		expect(chooseDefaultModelName(renamed)).toBe(
			chooseDefaultModelName(named),
		);
	});

	/** A name the rule can return but the build never bundled would be an invisible fixture. */
	it("bundles a file for every body the rule can choose", () => {
		for (const name of DEFAULT_MODEL_NAMES) {
			expect(defaultModelUrl(name), name).toBeTruthy();
		}
	});

	/** A profile that carries its own model keeps it; the shipped set is only the fallback. */
	it("prefers the model the package carries", () => {
		const withModel = fixture("par", rgb, "data:model/gltf-binary;base64,AAA");
		expect(fixtureModelSource(withModel)).toBe(
			"data:model/gltf-binary;base64,AAA",
		);
		expect(fixtureModelSource(fixture("par", rgb))).toBe(
			defaultModelUrl("led-par-x-in-1"),
		);
	});
});
