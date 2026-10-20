import { describe, expect, it } from "vitest";
import { paneTileGeometry, reflowPlaybackCells } from "./paneTileGeometry";

const viewport = { width: 900, height: 350, columnGap: 3, rowGap: 3 };
describe("Playback pane appearance", () => {
	it("uses square automatic cells and scrolls authored rows beyond the viewport", () => {
		const geometry = paneTileGeometry(viewport, 280, { mode: "automatic" });
		expect(geometry.columns).toBe(6);
		expect(geometry.style.gridTemplateColumns).toBe("repeat(6, 147.5px)");
		expect(geometry.style.gridTemplateRows).toBe("repeat(47, 147.5px)");
		expect(Number(geometry.style.height)).toBeGreaterThan(viewport.height);
	});
	it("fits six columns and three visible rows while preserving later authored rows", () => {
		const geometry = paneTileGeometry(viewport, 280, {
			mode: "fixed",
			rows: 3,
			columns: 6,
		});
		expect(geometry.style.gridTemplateColumns).toBe("repeat(6, 147.5px)");
		expect(geometry.style.gridTemplateRows).toBe(`repeat(47, ${344 / 3}px)`);
		expect(
			paneTileGeometry(viewport, 18, { mode: "fixed", rows: 3, columns: 6 })
				.style.height,
		).toBe(350);
	});
	it("keeps sparse authored addresses in order across responsive reflow without filling gaps", () => {
		const cells = [401, 402, 490, 491, 800].map((slot, index) => ({
			slot,
			rowIndex: index,
			columnIndex: 0,
		}));
		const original = structuredClone(cells);
		const reflowed = reflowPlaybackCells(cells, 2);
		expect(reflowed.map((cell) => cell.slot)).toEqual([
			401, 402, 490, 491, 800,
		]);
		expect(reflowed.map((cell) => [cell.rowIndex, cell.columnIndex])).toEqual([
			[0, 0],
			[0, 1],
			[1, 0],
			[1, 1],
			[2, 0],
		]);
		expect(cells).toEqual(original);
	});
	it("responds to pane width without changing authored count", () => {
		expect(
			paneTileGeometry({ ...viewport, width: 300 }, 280, { mode: "automatic" })
				.columns,
		).toBe(2);
	});
});
