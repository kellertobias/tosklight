import { describe, expect, it } from "vitest";
import { PlaybackDeskState } from "./deskState";
import { deskProjection } from "./testFixtures";

describe("visible surface page operations", () => {
	it("changes the selected surface's resolved page with command context", () => {
		const state = new PlaybackDeskState();
		state.hydrate(
			{ ...deskProjection(), resolved_pages: { main: 1, screen: 8 } },
			1,
		);
		const pending = state.begin(2);
		expect(pending?.desk).toMatchObject({
			command_page: 2,
			resolved_pages: { main: 2, screen: 8 },
		});
	});
	it("retains the initiating surface when another visible surface becomes selected", () => {
		const state = new PlaybackDeskState();
		state.hydrate(
			{ ...deskProjection(), resolved_pages: { main: 1, screen: 8 } },
			1,
		);
		state.begin(2);
		const next = state.apply(
			{
				...deskProjection(8),
				selected_surface_id: "screen",
				resolved_pages: { main: 1, screen: 8 },
			},
			2,
		);
		expect(next).toMatchObject({
			command_page: 8,
			selected_surface_id: "screen",
			resolved_pages: { main: 2, screen: 8 },
		});
	});
});
