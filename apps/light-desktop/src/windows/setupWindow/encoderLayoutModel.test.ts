import { describe, expect, it } from "vitest";
import type {
	AttributeConfiguration,
	ConfiguredAttributeDescriptor,
} from "../../api/client/attributeConfiguration";
import { moveAttributeToSlot, unplacedDescriptors } from "./encoderLayoutModel";

function descriptor(
	id: string,
	group: ConfiguredAttributeDescriptor["encoder_group"],
	page: number,
	slot: number,
): ConfiguredAttributeDescriptor {
	return {
		id,
		label: id,
		encoder_group: group,
		encoder_page: page,
		encoder_slot: slot,
		value_type: "continuous",
		display_unit: null,
		physical_unit: null,
		normalized_min: 0,
		normalized_max: 1,
		domain_min: 0,
		domain_max: 100,
		cyclic: false,
		recordable: true,
		built_in: true,
		retired: false,
		activation_group_id: id,
	} as ConfiguredAttributeDescriptor;
}

const descriptors = [
	descriptor("red", "color", 1, 1),
	descriptor("green", "color", 1, 2),
	descriptor("blue", "color", 1, 3),
	descriptor("dimmer", "intensity", 1, 1),
];

const configuration: AttributeConfiguration = {
	version: 1,
	custom_attributes: [],
	activation_groups: [],
	placements: descriptors.map((entry) => ({
		attribute: entry.id,
		encoder_group: entry.encoder_group,
		encoder_page: entry.encoder_page,
		encoder_slot: entry.encoder_slot,
		push_turn_of: null,
	})),
} as AttributeConfiguration;

function placement(next: AttributeConfiguration, attribute: string) {
	const found = next.placements.find(
		(entry) => entry.attribute === attribute,
	);
	return found
		? `${found.encoder_group}/${found.encoder_page}.${found.encoder_slot}`
		: "unplaced";
}

describe("encoder layout model", () => {
	it("reorders inside one row without leaving a hole", () => {
		const next = moveAttributeToSlot(
			configuration,
			descriptors,
			"blue",
			{ group: "color", page: 1, slot: 1 },
			6,
		);
		expect(placement(next, "blue")).toBe("color/1.1");
		expect(placement(next, "red")).toBe("color/1.2");
		expect(placement(next, "green")).toBe("color/1.3");
	});

	it("moves an encoder to another page at the configured width", () => {
		const next = moveAttributeToSlot(
			configuration,
			descriptors,
			"red",
			{ group: "color", page: 2, slot: 1 },
			2,
		);
		// Width 2 packs the remaining Color encoders into page 1 and appends red after them.
		expect(placement(next, "green")).toBe("color/1.1");
		expect(placement(next, "blue")).toBe("color/1.2");
		expect(placement(next, "red")).toBe("color/2.1");
	});

	it("renumbers both groups when an encoder changes group", () => {
		const next = moveAttributeToSlot(
			configuration,
			descriptors,
			"green",
			{ group: "intensity", page: 1, slot: 1 },
			6,
		);
		expect(placement(next, "green")).toBe("intensity/1.1");
		expect(placement(next, "dimmer")).toBe("intensity/1.2");
		expect(placement(next, "red")).toBe("color/1.1");
		expect(placement(next, "blue")).toBe("color/1.2");
	});

	it("assigns an attribute that had no slot at all", () => {
		const spare = descriptor("uv", "color", 1, 1);
		const next = moveAttributeToSlot(
			configuration,
			[...descriptors, spare],
			"uv",
			{ group: "color", page: 1, slot: 2 },
			6,
		);
		expect(unplacedDescriptors([...descriptors, spare], configuration)).toEqual(
			[spare],
		);
		expect(placement(next, "uv")).toBe("color/1.2");
		expect(unplacedDescriptors([...descriptors, spare], next)).toEqual([]);
	});
});
