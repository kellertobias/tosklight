import { describe, expect, it } from "vitest";
import { boundsOverlap, labelWidth, placeLabels, type LabelBounds, type LabelRequest } from "./labelPlacement";

const bounds: LabelBounds = { x: 7, y: 10, width: 660, height: 430 };
function requests(count: number): LabelRequest[] {
	return Array.from({ length: count }, (_, index) => {
		const lines = [`ID ${index + 1}`, `DMX ${Math.floor(index / 32) + 1}.${index % 32 * 12 + 1}`];
		return { key: `instance-${index.toString().padStart(4, "0")}`, anchor: [330, 220], lines, width: labelWidth(lines, 6), height: 20 };
	});
}
function checkBoxes(boxes: LabelBounds[]) {
	for (let index = 0; index < boxes.length; index++) for (const next of boxes.slice(index + 1)) expect(boundsOverlap(boxes[index], next)).toBe(false);
}

describe("indexed fixture label placement", () => {
	it("retains every ID and DMX string in a157-instance exact-anchor cluster without collisions", () => {
		const input = requests(157);
		const layout = placeLabels(input, bounds);
		expect(layout.unplaced).toEqual([]);
		expect(layout.labels).toHaveLength(input.length);
		expect(new Set(layout.labels.map((label) => label.key)).size).toBe(157);
		expect(layout.labels.map((label) => label.lines)).toEqual(input.map((label) => label.lines));
		checkBoxes(layout.labels.map((label) => label.box));
		for (const { box } of layout.labels) {
			expect(box.x).toBeGreaterThanOrEqual(bounds.x);
			expect(box.y).toBeGreaterThanOrEqual(bounds.y);
			expect(box.x + box.width).toBeLessThanOrEqual(bounds.x + bounds.width + 0.001);
			expect(box.y + box.height).toBeLessThanOrEqual(bounds.y + bounds.height + 0.001);
		}
		expect(layout.intersectionChecks).toBeLessThan(100_000);
	});

	it("is deterministic across source ordering and keeps separate multi-patch instances", () => {
		const input = requests(60).map((request) => ({ ...request, lines: ["ID 117", "DMX 1.1"] }));
		expect(placeLabels(input, bounds)).toEqual(placeLabels([...input].reverse(), bounds));
		expect(placeLabels(input, bounds).labels).toHaveLength(60);
	});

	it("bounds1000-instance work and reports capacity without dropping or truncating identities", () => {
		const input = requests(1000);
		const layout = placeLabels(input, bounds);
		expect(layout.labels.length + layout.unplaced.length).toBe(1000);
		expect(layout.unplaced.length).toBeGreaterThan(0);
		expect(layout.intersectionChecks).toBeLessThan(1_000_000);
		expect(new Set([...layout.labels, ...layout.unplaced].map((label) => label.key)).size).toBe(1000);
		checkBoxes(layout.labels.map((label) => label.box));
	});
	it("never falls back to placing text on a lamp when there is no free space", () => {
		const layout = placeLabels(requests(1), bounds, 1.5, [bounds]);
		expect(layout.labels).toEqual([]);
		expect(layout.unplaced).toHaveLength(1);
	});

});
