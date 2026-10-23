import { describe, expect, it } from "vitest";
import { timelineTrailingGutter } from "./timelineTrailingGutter";

describe("timeline trailing gutter", () => {
	it("reserves classic scrollbar space without changing the true time canvas", () => {
		const canvas = 1000, client = 485, offset = 500;
		const gutter = timelineTrailingGutter(canvas, client, offset, 0);
		expect(gutter).toBe(15);
		// Chromium's native range uses offset width; the real canvas must reach client right.
		expect(canvas + gutter - offset).toBe(canvas - client);
	});
	it("excludes borders and leaves overlay scrollbars unchanged", () => {
		expect(timelineTrailingGutter(1000, 485, 504, 4)).toBe(15);
		expect(timelineTrailingGutter(1000, 500, 504, 4)).toBe(0);
	});
	it("does not create overflow when the canvas fits", () => {
		expect(timelineTrailingGutter(485, 485, 500, 0)).toBe(0);
		expect(timelineTrailingGutter(400, 485, 500, 0)).toBe(0);
	});
});
