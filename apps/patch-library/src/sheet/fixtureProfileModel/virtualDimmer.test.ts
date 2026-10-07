import { describe, expect, it } from "vitest";
import type { FixtureChannel, FixtureMode } from "../../wire";
import {
	headHasVirtualDimmer,
	withDefaultVirtualDimmerReaction,
} from "./virtualDimmer";

function mode(attributes: string[]): FixtureMode {
	return {
		channels: attributes.map(
			(attribute, index) =>
				({
					id: `channel-${index}`,
					head_id: "head",
					attribute,
					fixture_attribute: attribute,
					reacts_to_virtual_intensity: false,
					virtual_intensity_inverted: false,
				}) as FixtureChannel,
		),
	} as FixtureMode;
}

describe("virtual dimmer", () => {
	it("belongs to a light-emitting head without an Intensity channel", () => {
		expect(headHasVirtualDimmer(mode(["color.red", "color.green"]), "head")).toBe(true);
		expect(headHasVirtualDimmer(mode(["intensity", "color.red"]), "head")).toBe(false);
		expect(headHasVirtualDimmer(mode(["pan", "tilt"]), "head")).toBe(false);
		expect(headHasVirtualDimmer(mode(["color.cyan"]), "head")).toBe(false);
	});

	it("is followed by an emitter chosen on a head without a dimmer", () => {
		const rgb = mode(["color.red", "pan"]);
		const red = withDefaultVirtualDimmerReaction(rgb, rgb.channels[1], "color.blue");
		expect(red.reacts_to_virtual_intensity).toBe(true);
		expect(red.virtual_intensity_inverted).toBe(false);

		const dimmed = mode(["intensity", "pan"]);
		const blue = withDefaultVirtualDimmerReaction(
			dimmed,
			{ ...dimmed.channels[1], reacts_to_virtual_intensity: true },
			"color.blue",
		);
		expect(blue.reacts_to_virtual_intensity).toBe(false);

		// Anything but an emitter keeps the operator's choice.
		const inverse = { ...rgb.channels[1], reacts_to_virtual_intensity: true, virtual_intensity_inverted: true };
		expect(withDefaultVirtualDimmerReaction(rgb, inverse, "color.cyan")).toBe(inverse);
	});
});
