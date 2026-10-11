import { describe, expect, it } from "vitest";
import { soloFenceClasses } from "./soloFence";
import type { PlaybackSlotProjection } from "./types";
const cell = (
	slot: number,
	rowIndex: number,
	first: number,
): PlaybackSlotProjection => ({
	slot,
	rowIndex,
	row: {
		first_position: first,
		count: 2,
		presentation: "buttons",
		button_count: 1,
	},
	playback: null,
	cue: null,
	group: null,
	footprint: null,
});
const cells = [
	cell(401, 0, 401),
	cell(402, 0, 401),
	cell(601, 1, 601),
	cell(602, 1, 601),
];
describe("canonical Solo fences", () => {
	it("joins displayed adjacent members across numbering gaps and closes concave corners", () => {
		const region = {
			id: "l",
			name: "L shape",
			members: [401, 402, 601].map((position) => ({ page: 3, position })),
		};
		const topLeft = soloFenceClasses(cells[0], cells, 3, 2, [region]);
		expect(topLeft).toContain("solo-corner-bottom-right");
		expect(topLeft).not.toContain("solo-fence-right");
		expect(topLeft).not.toContain("solo-fence-bottom");
		expect(soloFenceClasses(cells[2], cells, 3, 2, [region])).not.toContain(
			"solo-fence-top",
		);
		expect(soloFenceClasses(cells[3], cells, 3, 2, [region])).toBe("");
	});
	it("does not join adjacent members of unrelated regions or another page", () => {
		const regions = [
			{
				id: "a",
				name: "A",
				members: [
					{ page: 1, position: 401 },
					{ page: 2, position: 9 },
				],
			},
			{
				id: "b",
				name: "B",
				members: [
					{ page: 1, position: 402 },
					{ page: 2, position: 10 },
				],
			},
		];
		expect(soloFenceClasses(cells[0], cells, 1, 2, regions)).toContain(
			"solo-fence-right",
		);
		expect(soloFenceClasses(cells[1], cells, 1, 2, regions)).toContain(
			"solo-fence-left",
		);
		expect(soloFenceClasses(cells[0], cells, 2, 2, regions)).toBe("");
	});
});
