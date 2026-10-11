import { describe, expect, it } from "vitest";
import {
	soloFenceClasses,
	soloFenceSegments,
	soloFenceCell,
} from "./soloFence";
import type { PlaybackSlotProjection } from "./types";
const cell = (
	slot: number,
	rowIndex: number,
	first: number,
): PlaybackSlotProjection => ({
	slot,
	rowIndex,
	columnIndex: slot - first,
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
	const expanded = (
		slot: number,
		row: number,
		column: number,
		rowSpan: number,
		columnSpan: number,
	): PlaybackSlotProjection => ({
		...cell(slot, row, slot - column),
		footprint: {
			rowStart: row + 1,
			columnStart: column + 1,
			rowSpan,
			columnSpan,
		} as PlaybackSlotProjection["footprint"],
	});
	const pair = {
		id: "pair",
		name: "Pair",
		members: [1, 3, 4].map((position) => ({ page: 1, position })),
	};
	it.each([
		{ wide: true, cells: [expanded(1, 0, 0, 1, 2), cell(3, 0, 1)] },
		{ wide: false, cells: [expanded(1, 0, 0, 2, 1), cell(3, 2, 3)] },
	])("joins expanded shared boundaries with wider=$wide", ({ wide, cells }) => {
		expect(soloFenceClasses(cells[0], cells, 1, 3, [pair])).not.toContain(
			wide ? "solo-fence-right" : "solo-fence-bottom",
		);
		expect(soloFenceClasses(cells[1], cells, 1, 3, [pair])).not.toContain(
			wide ? "solo-fence-left" : "solo-fence-top",
		);
	});
	it("joins a Wider member to both normal members below", () => {
		const shape = [expanded(1, 0, 0, 1, 2), cell(3, 1, 3), cell(4, 1, 3)];
		expect(soloFenceClasses(shape[0], shape, 1, 2, [pair])).not.toContain(
			"solo-fence-bottom",
		);
		for (const member of shape.slice(1))
			expect(soloFenceClasses(member, shape, 1, 2, [pair])).not.toContain(
				"solo-fence-top",
			);
	});
	it("retains the exposed half of a Wider bottom edge on a partial join", () => {
		const shape = [expanded(1, 0, 0, 1, 2), cell(3, 1, 3)];
		expect(soloFenceClasses(shape[0], shape, 1, 2, [pair])).toContain(
			"solo-fence-bottom",
		);
		expect(soloFenceClasses(shape[1], shape, 1, 2, [pair])).not.toContain(
			"solo-fence-top",
		);
	});

	it("draws exactly the exposed half of a partial edge and separate duplicate footprints", () => {
		const shape = [
			expanded(1, 0, 0, 1, 2),
			cell(3, 1, 3),
			expanded(1, 3, 0, 1, 2),
			cell(3, 4, 3),
		];
		const segments = soloFenceSegments(shape.map(soloFenceCell), 1, [pair]);
		expect(
			segments.filter(
				(line) => line.cellKey === "0:0" && line.edge === "bottom",
			),
		).toEqual([{ cellKey: "0:0", edge: "bottom", x1: 1, y1: 1, x2: 2, y2: 1 }]);
		expect(
			segments.filter(
				(line) => line.cellKey === "3:0" && line.edge === "bottom",
			),
		).toEqual([{ cellKey: "3:0", edge: "bottom", x1: 1, y1: 4, x2: 2, y2: 4 }]);
		expect(
			segments.filter(
				(line) => ["1:0", "4:0"].includes(line.cellKey) && line.edge === "top",
			),
		).toEqual([]);
	});

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
