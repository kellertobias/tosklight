import { describe, expect, it } from "vitest";
import { squarePlaybackGeometry } from "./tileGeometry";

describe("square Playback pane tracks", () => {
	it("preserves112px touch cells and scrolls a20x14 Free region without stretching rows", () => {
		const geometry = squarePlaybackGeometry(
			{ width: 600, height: 700, columnGap: 5, rowGap: 5 },
			20,
			14,
			true,
		);
		expect(geometry).toMatchObject({
			gridTemplateColumns: "repeat(20, 112px)",
			gridTemplateRows: "repeat(14, 112px)",
			width: 2335,
			height: 1633,
			alignContent: "start",
			justifyContent: "start",
			padding: 0,
		});
	});
	it("uses available Free region width while retaining square rows on a tall pane", () => {
		expect(
			squarePlaybackGeometry(
				{ width: 495, height: 1000, columnGap: 5, rowGap: 7 },
				4,
				2,
				true,
			),
		).toMatchObject({
			gridTemplateColumns: "repeat(4, 120px)",
			gridTemplateRows: "repeat(2, 120px)",
			width: 495,
			height: 247,
		});
	});
	it("fits all Screen rows when height is limiting without imposing a112px minimum", () => {
		expect(
			squarePlaybackGeometry(
				{ width: 800, height: 315, columnGap: 3, rowGap: 5 },
				4,
				4,
				false,
			),
		).toMatchObject({
			gridTemplateColumns: "repeat(4, 75px)",
			gridTemplateRows: "repeat(4, 75px)",
			width: 309,
			height: 315,
		});
	});
	it("fits Screen width and leaves spare height below the square grid", () => {
		expect(
			squarePlaybackGeometry(
				{ width: 495, height: 1000, columnGap: 5, rowGap: 7 },
				4,
				2,
				false,
			),
		).toMatchObject({
			gridTemplateColumns: "repeat(4, 120px)",
			gridTemplateRows: "repeat(2, 120px)",
			width: 495,
			height: 247,
		});
	});
	it("recomputes square tracks after resize with the actual row and column gaps", () => {
		const initial = squarePlaybackGeometry(
			{ width: 495, height: 1000, columnGap: 5, rowGap: 7 },
			4,
			2,
			false,
		);
		const resized = squarePlaybackGeometry(
			{ width: 600, height: 209, columnGap: 8, rowGap: 9 },
			4,
			2,
			false,
		);
		expect(initial.width).toBe(495);
		expect(resized).toMatchObject({
			gridTemplateColumns: "repeat(4, 100px)",
			gridTemplateRows: "repeat(2, 100px)",
			width: 424,
			height: 209,
		});
	});
});
