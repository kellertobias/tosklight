import { describe, expect, it, vi } from "vitest";
import type {
	DynamicDefinitionProjection,
	DynamicUpdateIntent,
} from "../../api/types";
import { projectionEncoderSlots } from "./ProjectionEncoderSlots";

function dynamic(
	spatial_mapping: unknown = { projection: { type: "inherit" } },
): DynamicDefinitionProjection {
	return { spatial_mapping } as unknown as DynamicDefinitionProjection;
}

function pages(
	value: DynamicDefinitionProjection,
	width = 6,
	onMutate = vi.fn(async () => undefined),
) {
	const slots = projectionEncoderSlots(value, width, onMutate);
	const result: string[][] = [];
	for (let index = 0; index < slots.length; index += width)
		result.push(
			slots
				.slice(index, index + width)
				.filter((slot) => !slot.disabled)
				.map((slot) => slot.label),
		);
	return result;
}

const cylindrical = dynamic({
	projection: {
		type: "replace",
		value: {
			anchor: { x: 0, y: 0, z: 0 },
			view_direction: { x: 0, y: 0, z: -1 },
			rotation_degrees: 0,
			kind: "cylindrical",
		},
	},
	shape: { type: "inherit" },
});

describe("projection encoder slots", () => {
	it("places the projection on page one and orients it on page two", () => {
		expect(pages(cylindrical)).toEqual([
			["Projection", "Position X", "Position Y", "Position Z"],
			["Direction X", "Direction Y", "Direction Z", "Rotation"],
		]);
	});

	it("keeps the two pages meaningful at a wider encoder layout", () => {
		// Without padding the direction would climb onto page one at width 10.
		expect(pages(cylindrical, 10)).toEqual([
			["Projection", "Position X", "Position Y", "Position Z"],
			["Direction X", "Direction Y", "Direction Z", "Rotation"],
		]);
	});

	it("gives a spherical projection a direction and no rotation", () => {
		const spherical = dynamic({
			projection: {
				type: "replace",
				value: {
					anchor: { x: 0, y: 0, z: 0 },
					view_direction: { x: 0, y: 0, z: -1 },
					rotation_degrees: 0,
					kind: "spherical",
				},
			},
			shape: { type: "inherit" },
		});
		expect(pages(spherical)).toEqual([
			["Projection", "Position X", "Position Y", "Position Z"],
			["Direction X", "Direction Y", "Direction Z"],
		]);
	});

	it("fits a planar projection on one page", () => {
		expect(pages(dynamic())).toEqual([
			["Projection", "Direction X", "Direction Y", "Direction Z", "Rotation"],
		]);
	});

	it("turning an encoder overrides an inherited projection", async () => {
		const onMutate =
			vi.fn<
				(intent: DynamicUpdateIntent, group?: string) => Promise<undefined>
			>(async () => undefined);
		const slots = projectionEncoderSlots(dynamic(), 6, onMutate);
		const directionX = slots.find((slot) => slot.label === "Direction X");
		await directionX?.apply(0.5, "group-1");
		expect(onMutate).toHaveBeenCalledOnce();
		expect(onMutate.mock.calls[0]?.[0]).toMatchObject({
			type: "set_spatial_mapping",
			spatial_mapping: {
				projection: { type: "replace", value: { view_direction: { x: 0.5 } } },
			},
		});
	});
});
