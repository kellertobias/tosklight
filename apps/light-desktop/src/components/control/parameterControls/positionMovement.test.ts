import { describe, expect, it } from "vitest";
import {
	formatPositionMovement,
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
