import { describe, expect, it } from "vitest";
import { soloFenceSegments, type SoloFenceCell } from "./soloFence";
import { soloFencePaths } from "./soloFencePaths";
const cell = (
	position: number,
	left: number,
	top: number,
	size = 40,
): SoloFenceCell => ({
	key: String(position),
	position,
	rect: { left, top, right: left + size, bottom: top + size },
});
const paths = (cells: SoloFenceCell[]) =>
	soloFencePaths(
		soloFenceSegments(cells, 1, [
			{
				id: "region",
				name: "Region",
				members: cells.map((item) => ({ page: 1, position: item.position })),
			},
		]),
	);
describe("rounded Solo union contours", () => {
	it("simplifies adjacent rectangles into one outer rounded perimeter", () => {
		const result = paths([cell(401, 0, 0), cell(402, 40, 0)]);
		expect(result).toHaveLength(1);
		expect(result[0].match(/Q /g)).toHaveLength(4);
		expect(result[0]).toContain("Q 80 40 74 40");
		expect(result[0]).not.toContain("Q 40 ");
		expect(result[0]).toMatch(/ Z$/);
	});
	it("rounds the concave corner of an L-shaped union without adding an internal edge", () => {
		const result = paths([cell(401, 0, 0), cell(402, 40, 0), cell(421, 0, 40)]);
		expect(result).toHaveLength(1);
		expect(result[0].match(/Q /g)).toHaveLength(6);
		expect(result[0]).toContain("Q 40 40 40 46");
	});
	it("keeps both separated and corner-touching islands as independent closed outlines", () => {
		for (const offset of [40, 80]) {
			const result = paths([cell(401, 0, 0), cell(422, offset, offset)]);
			expect(result).toHaveLength(2);
			for (const path of result) expect(path.match(/Q /g)).toHaveLength(4);
		}
	});
	it("keeps adjacent distinct Solo regions as two complete rounded outlines", () => {
		const cells = [cell(401, 0, 0), cell(402, 40, 0)];
		const regions = cells.map((item) => ({
			id: String(item.position),
			name: "Separate",
			members: [{ page: 1, position: item.position }],
		}));
		const result = soloFencePaths(soloFenceSegments(cells, 1, regions));
		expect(result).toHaveLength(2);
		for (const path of result) expect(path.match(/Q /g)).toHaveLength(4);
		expect(result[0]).toContain("Q 40 40");
		expect(result[1]).toContain("Q 40 0");
	});

	it("clamps the radius to short edges rather than crossing corners", () => {
		expect(paths([cell(401, 0, 0, 4)])[0]).toContain("Q 4 0 4 2");
	});
});
