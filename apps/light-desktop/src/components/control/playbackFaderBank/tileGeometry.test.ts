import { describe, expect, it } from "vitest";
import { fittedPlaybackGeometry } from "./tileGeometry";

describe("fitted Playback pane tracks", () => {
	it("fills both dimensions with every authored20x14 cell and no scrolling", () => {
		const geometry = fittedPlaybackGeometry(
			{ width: 600, height: 695, columnGap: 5, rowGap: 5 },
			20,
			14,
		);
		expect(geometry).toMatchObject({
			gridTemplateColumns: "repeat(20, 25.25px)",
			gridTemplateRows: "repeat(14, 45px)",
			width: 600,
			height: 695,
			alignContent: "start",
			justifyContent: "start",
			padding: 0,
		});
	});
	it("fills the full width with six columns and full height with three rows", () => {
		expect(
			fittedPlaybackGeometry(
				{ width: 1000, height: 250, columnGap: 5, rowGap: 5 },
				6,
				3,
			),
		).toMatchObject({
			gridTemplateColumns: "repeat(6, 162.5px)",
			gridTemplateRows: "repeat(3, 80px)",
			width: 1000,
			height: 250,
		});
	});
	it("preserves six columns and three rows when resizing to a narrower taller pane", () => {
		const initial = fittedPlaybackGeometry(
			{ width: 1000, height: 250, columnGap: 5, rowGap: 5 },
			6,
			3,
		);
		const resized = fittedPlaybackGeometry(
			{ width: 325, height: 550, columnGap: 5, rowGap: 5 },
			6,
			3,
		);
		expect(initial.gridTemplateRows).toBe("repeat(3, 80px)");
		expect(resized).toMatchObject({
			gridTemplateColumns: "repeat(6, 50px)",
			gridTemplateRows: "repeat(3, 180px)",
			width: 325,
			height: 550,
		});
	});
	it("uses different actual row and column gaps without leaving unused width", () => {
		expect(
			fittedPlaybackGeometry(
				{ width: 800, height: 315, columnGap: 3, rowGap: 5 },
				4,
				4,
			),
		).toMatchObject({
			gridTemplateColumns: "repeat(4, 197.75px)",
			gridTemplateRows: "repeat(4, 75px)",
			width: 800,
			height: 315,
		});
	});
	it("stretches rows to use all of a tall pane rather than leaving space below", () => {
		expect(
			fittedPlaybackGeometry(
				{ width: 495, height: 1000, columnGap: 5, rowGap: 7 },
				4,
				2,
			),
		).toMatchObject({
			gridTemplateColumns: "repeat(4, 120px)",
			gridTemplateRows: "repeat(2, 496.5px)",
			width: 495,
			height: 1000,
		});
	});
});
