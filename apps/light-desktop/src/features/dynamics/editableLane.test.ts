import { describe, expect, it } from "vitest";
import type { AttributeDescriptor, DynamicLaneProjection } from "../../api/types";
import {
	commitEditableLane,
	createDynamicLane,
	createRandomGroup,
	editableLane,
	graphLane,
	retargetDynamicLane,
} from "./editableLane";
import { dynamicLaneChoices, laneDomainForKey } from "./laneDomain";
import { isScalarDynamicLane } from "./laneModel";

function registryEntry(id: string, family: string, valueType = "continuous") {
	return {
		id,
		label: id,
		family,
		value_type: valueType,
		recordable: true,
		normalized_min: valueType === "continuous" ? 0 : null,
		normalized_max: valueType === "continuous" ? 1 : null,
	} as AttributeDescriptor;
}

const REGISTRY = [
	registryEntry("intensity", "intensity"),
	registryEntry("color", "color", "color"),
	registryEntry("color.red", "color"),
	registryEntry("color.cyan", "color"),
	registryEntry("color.tint", "color"),
	registryEntry("position", "position", "position"),
	registryEntry("pan", "position"),
	registryEntry("tilt", "position"),
	registryEntry("pan.continuous", "position"),
	registryEntry("focus", "focus"),
	registryEntry("zoom", "focus"),
];

describe("Dynamic lane chooser under programming contract 1", () => {
	it("offers Position, Color and Zoom as family components and keeps scalar attributes", () => {
		expect(
			dynamicLaneChoices(REGISTRY).map((choice) => [
				choice.family,
				choice.id,
				choice.label,
			]),
		).toEqual([
			["intensity", "intensity", "intensity"],
			["color", "color.red", "Red"],
			["color", "color.green", "Green"],
			["color", "color.blue", "Blue"],
			["color", "color.amber", "Amber"],
			["color", "color.hue", "Hue"],
			["color", "color.saturation", "Saturation"],
			["color", "color.white_blend", "White Blend"],
			["color", "color.temperature", "Temperature"],
			["color", "color.uv", "UV"],
			["color", "color.tint", "color.tint"],
			["position", "position.pan", "Pan"],
			["position", "position.tilt", "Tilt"],
			["focus", "focus", "focus"],
			["focus", "zoom", "Zoom"],
		]);
	});
});

describe("typed lanes from the editor", () => {
	it("creates Pan and Tilt as Angle lanes swinging around Current in degrees", () => {
		const pan = createDynamicLane("position.pan", "pan");
		expect(pan).toEqual({
			id: "pan",
			speed_multiplier: { numerator: 1, denominator: 1 },
			width: 1,
			random_group_id: null,
			phase: null,
			programming: {
				address: {
					representation: { kind: "angles" },
					component: { kind: "pan" },
				},
				configuration: {
					mode: "middle_amplitude",
					configuration: {
						middle: { kind: "current" },
						amplitude: { kind: "scalar", value: 45 },
						function: "sinus",
						size: 1,
						pwm: expect.any(Object),
						invert_waveform: false,
					},
				},
			},
		});
		expect(createDynamicLane("position.tilt")).toMatchObject({
			programming: {
				address: { component: { kind: "tilt" } },
				configuration: { configuration: { amplitude: { value: 30 } } },
			},
		});
	});

	it("creates Zoom in degrees and Color components on their semantic basis", () => {
		expect(createDynamicLane("zoom")).toMatchObject({
			programming: {
				address: {
					representation: { kind: "zoom", convention: "beam" },
					component: { kind: "zoom" },
				},
				configuration: {
					mode: "max_min",
					configuration: {
						minimum: { kind: "value", value: { kind: "scalar", value: 10 } },
						maximum: { kind: "value", value: { kind: "scalar", value: 40 } },
					},
				},
			},
		});
		const basis = (key: string) => {
			const lane = createDynamicLane(key);
			return isScalarDynamicLane(lane)
				? null
				: lane.programming.address.representation;
		};
		expect(basis("color.red")).toEqual({ kind: "semantic_color", basis: "recipe" });
		expect(basis("color.hue")).toEqual({
			kind: "semantic_color",
			basis: "hue_saturation",
		});
		expect(basis("color.white_blend")).toEqual({
			kind: "semantic_color",
			basis: "retain",
		});
		expect(isScalarDynamicLane(createDynamicLane("intensity"))).toBe(true);
	});

	it("round-trips a typed lane through the editor in descriptor units", () => {
		const pan = createDynamicLane("position.pan", "pan");
		const editable = editableLane(pan);
		expect(editable).toMatchObject({
			attribute: "position.pan",
			mode: "middle_amplitude",
			middle_amplitude: { middle: { type: "current" }, amplitude: 45 },
		});
		expect(commitEditableLane(editable!, pan)).toEqual(pan);

		const keyframed = commitEditableLane(
			{
				...editable!,
				mode: "keyframes",
				keyframes: {
					size: 1,
					points: [
						{ position: 0, source: { type: "value", value: -90 }, interpolation: "linear" },
						{ position: 0.5, source: { type: "current" }, interpolation: "linear" },
					],
				},
			},
			pan,
		);
		expect(keyframed).toMatchObject({
			programming: {
				configuration: {
					mode: "keyframes",
					configuration: {
						points: [
							{ source: { kind: "value", value: { kind: "scalar", value: -90 } } },
							{ source: { kind: "current" } },
						],
					},
				},
			},
		});
		expect(keyframed).not.toHaveProperty("attribute");
	});

	it("draws an Angle on its own range and leaves a level on 0–1", () => {
		const editable = editableLane(createDynamicLane("position.pan"))!;
		const graph = graphLane(editable, laneDomainForKey("position.pan"));
		expect(graph.middle_amplitude.amplitude).toBeCloseTo(45 / 540);
		const level = editableLane(createDynamicLane("intensity"))!;
		expect(graphLane(level, laneDomainForKey("intensity"))).toBe(level);
	});

	it("re-addresses a lane to another domain from that domain's defaults", () => {
		const intensity: DynamicLaneProjection = {
			...createDynamicLane("intensity", "lane"),
			width: 0.5,
		};
		const tilt = retargetDynamicLane(intensity, "position.tilt");
		expect(tilt).toMatchObject({
			id: "lane",
			width: 0.5,
			programming: { address: { component: { kind: "tilt" } } },
		});
		expect(retargetDynamicLane(intensity, "focus")).toMatchObject({
			id: "lane",
			attribute: "focus",
		});
	});

	it("gives a typed lane a typed Random range", () => {
		expect(createRandomGroup(laneDomainForKey("position.pan"))).toMatchObject({
			programming_range: {
				low: { kind: "value", value: { kind: "scalar", value: -45 } },
				high: { kind: "value", value: { kind: "scalar", value: 45 } },
			},
		});
		expect(createRandomGroup(laneDomainForKey("intensity"))).toMatchObject({
			low: { type: "value", value: 0 },
			high: { type: "value", value: 1 },
		});
	});
});
