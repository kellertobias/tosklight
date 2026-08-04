import { describe, expect, it } from "vitest";
import {
	formatPositionAxis,
	formatPositionMovement,
	positionAxisRepresentation,
	positionMovementRepresentation,
} from "./positionMovement";

function fixture(
	...representations: Array<"speed" | "time" | "speed_or_time">
) {
	return {
		definition: {
			heads: [
				{
					parameters: representations.map((representation) => ({
						attribute: "position.movement",
						metadata: {
							position_movement_representation: representation,
						},
					})),
				},
			],
		},
	};
}

describe("Position Movement representation", () => {
	it("retains the fixture-authored speed or time meaning without converting it", () => {
		expect(positionMovementRepresentation([fixture("speed")])).toBe("speed");
		expect(positionMovementRepresentation([fixture("time")])).toBe("move_time");
		expect(positionMovementRepresentation([fixture("speed_or_time")])).toBe(
			"speed_time",
		);
	});

	it("labels mixed fixture representations instead of inventing a conversion", () => {
		expect(
			positionMovementRepresentation([fixture("speed"), fixture("time")]),
		).toBe("mixed");
		expect(formatPositionMovement("25%...75%", "mixed")).toBe(
			"25%...75% mixed representation",
		);
	});
});

describe("Pan and Tilt representation", () => {
	const axisFixture = (
		attribute: "pan" | "tilt",
		representation: "absolute" | "endless",
	) => ({
		definition: {
			heads: [
				{
					parameters: [
						{
							attribute,
							metadata: { position_axis_representation: representation },
						},
					],
				},
			],
		},
	});

	it("keeps absolute and endless operation on the same canonical axis", () => {
		expect(
			positionAxisRepresentation([axisFixture("pan", "absolute")], "pan"),
		).toBe("absolute");
		expect(
			positionAxisRepresentation([axisFixture("pan", "endless")], "pan"),
		).toBe("endless");
		expect(formatPositionAxis("40%", "endless")).toBe("40% endless");
	});

	it("reports mixed selected-fixture modes without converting them", () => {
		expect(
			positionAxisRepresentation(
				[axisFixture("tilt", "absolute"), axisFixture("tilt", "endless")],
				"tilt",
			),
		).toBe("mixed");
		expect(formatPositionAxis("10%...90%", "mixed")).toBe(
			"10%...90% mixed mode",
		);
	});
});
