import { describe, expect, it } from "vitest";
import type {
	AttributeConfiguration,
	ConfiguredAttributeDescriptor,
} from "../../api/client/attributeConfiguration";
import {
	addActivationMember,
	applyActivationPreset,
	deleteActivationGroup,
	removeActivationMember,
	renameActivationGroup,
} from "./activationPresets";

function descriptor(
	id: string,
	label: string,
	retired = false,
): ConfiguredAttributeDescriptor {
	return {
		id,
		label,
		encoder_group: "color",
		encoder_page: 1,
		encoder_slot: 1,
		retired,
		built_in: true,
	} as ConfiguredAttributeDescriptor;
}

const descriptors = [
	descriptor("color.mix", "Color Mix"),
	descriptor("color.wheel.1", "Color Wheel 1"),
	descriptor("pan", "Pan"),
	descriptor("tilt", "Tilt"),
	descriptor("haze", "Haze"),
	descriptor("legacy", "Legacy", true),
];

const configuration = {
	version: 1,
	custom_attributes: [],
	placements: [],
	activation_groups: [{ id: "old", label: "Old", members: ["pan"] }],
} as unknown as AttributeConfiguration;

const recommended = [
	{ id: "rec", label: "Recommended", members: ["color.mix"] },
];

function shape(next: AttributeConfiguration) {
	return next.activation_groups.map(
		(group) => `${group.label}: ${group.members.join(",")}`,
	);
}

describe("activation presets", () => {
	it("None clears every group", () => {
		expect(
			applyActivationPreset("none", configuration, descriptors, recommended)
				.activation_groups,
		).toEqual([]);
	});

	it("All collects the active attributes into one group", () => {
		expect(
			shape(applyActivationPreset("all", configuration, descriptors, recommended)),
		).toEqual(["All attributes: color.mix,color.wheel.1,pan,tilt,haze"]);
	});

	it("By Encoder Group applies the documented grouping and keeps the rest on their own", () => {
		expect(
			shape(
				applyActivationPreset(
					"encoder-group",
					configuration,
					descriptors,
					recommended,
				),
			),
		).toEqual([
			"Color: color.mix,color.wheel.1",
			"Position: pan,tilt",
			"Haze: haze",
		]);
	});

	it("Intelligent takes the server-projected recommendation", () => {
		expect(
			applyActivationPreset(
				"intelligent",
				configuration,
				descriptors,
				recommended,
			).activation_groups,
		).toEqual(recommended);
	});

	it("stays editable after a preset", () => {
		const preset = applyActivationPreset(
			"encoder-group",
			configuration,
			descriptors,
			recommended,
		);
		const renamed = renameActivationGroup(
			preset,
			preset.activation_groups[0].id,
			"Colour",
		);
		expect(renamed.activation_groups[0].label).toBe("Colour");

		// Adding moves the attribute out of whichever group held it.
		const moved = addActivationMember(
			renamed,
			renamed.activation_groups[0].id,
			"pan",
		);
		expect(shape(moved)).toEqual([
			"Colour: color.mix,color.wheel.1,pan",
			"Position: tilt",
			"Haze: haze",
		]);

		const withoutTilt = removeActivationMember(
			moved,
			moved.activation_groups[1].id,
			"tilt",
		);
		expect(shape(withoutTilt)).toEqual([
			"Colour: color.mix,color.wheel.1,pan",
			"Haze: haze",
		]);

		expect(
			shape(
				deleteActivationGroup(
					withoutTilt,
					withoutTilt.activation_groups[0].id,
				),
			),
		).toEqual(["Haze: haze"]);
	});
});
