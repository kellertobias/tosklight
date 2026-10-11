import { describe, expect, it } from "vitest";
import {
	copyPlaybackLayout,
	gridPlaybackLayout,
	playbackLayoutError,
	playbackSurfaceCells,
} from "./surfaceLayout";

describe("shared playback surface geometry", () => {
	it("maps the acceptance surfaces to exact positions without allocating assignments", () => {
		const main = {
			rows: [
				{
					first_position: 1,
					count: 15,
					presentation: "fader" as const,
					button_count: 3,
				},
				{
					first_position: 101,
					count: 15,
					presentation: "buttons" as const,
					button_count: 1,
				},
			],
		};
		expect(playbackSurfaceCells(main).map((cell) => cell.position)).toEqual([
			...Array.from({ length: 15 }, (_, index) => index + 1),
			...Array.from({ length: 15 }, (_, index) => index + 101),
		]);
		const second = gridPlaybackLayout(201, 10, 5);
		second.rows.push({
			first_position: 301,
			count: 5,
			presentation: "push_encoder",
			button_count: 1,
		});
		expect(playbackSurfaceCells(second)).toHaveLength(55);
		expect(playbackSurfaceCells(second).at(-1)).toMatchObject({
			position: 305,
			presentation: "push_encoder",
		});
		const pane = playbackSurfaceCells(gridPlaybackLayout(401, 20, 14));
		expect(pane).toHaveLength(280);
		expect(pane.at(-1)?.position).toBe(680);
	});
	it("copies screen layout into an independent review draft", () => {
		const screen = gridPlaybackLayout(201, 10, 5);
		const draft = copyPlaybackLayout(screen);
		draft.rows[0].first_position = 401;
		expect(screen.rows[0].first_position).toBe(201);
		expect(Object.keys(draft)).toEqual(["rows"]);
	});
	it("rejects overlaps and out-of-range positions", () => {
		expect(playbackLayoutError(gridPlaybackLayout(401, 20, 14))).toBeNull();
		expect(playbackLayoutError(gridPlaybackLayout(990, 20, 1))).toContain(
			"between 1 and 1000",
		);
		const layout = gridPlaybackLayout(1, 5, 2);
		layout.rows[1].first_position = 3;
		expect(playbackLayoutError(layout)).toContain("appears twice");
	});
});
