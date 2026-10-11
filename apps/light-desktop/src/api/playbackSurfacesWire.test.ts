import { describe, it, expect } from "vitest";
import { decodePlaybackSurfaces } from "./playbackSurfacesWire";
const snapshot = {
	playback_surfaces: {
		main: {
			name: "Main",
			layout: {
				rows: [
					{
						first_position: 301,
						count: 5,
						presentation: "push_encoder",
						button_count: 1,
					},
				],
			},
			page_source: { type: "own", page: 99 },
		},
	},
	selected_surface_id: "main",
	resolved_pages: { main: 99 },
	command_page: 99,
};
describe("shared surface authority wire", () => {
	it("retains exact explicit positions and authoritative command page", () =>
		expect(decodePlaybackSurfaces(snapshot)).toEqual(snapshot));
	it("rejects a command page inconsistent with visibly selected surface", () =>
		expect(() =>
			decodePlaybackSurfaces({ ...snapshot, command_page: 1 }),
		).toThrow("selected surface resolved page"));
	it("rejects retired layout fields and ranges beyond1000", () => {
		const surface = snapshot.playback_surfaces.main;
		for (const layout of [
			{ playbacks_per_row: 5, rows: [] },
			{
				rows: [
					{
						first_position: 999,
						count: 5,
						presentation: "buttons",
						button_count: 1,
					},
				],
			},
		])
			expect(() =>
				decodePlaybackSurfaces({
					...snapshot,
					playback_surfaces: { main: { ...surface, layout } },
				}),
			).toThrow();
	});
});
