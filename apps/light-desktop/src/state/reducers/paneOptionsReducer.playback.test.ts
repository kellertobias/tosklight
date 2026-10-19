import { describe, expect, it } from "vitest";
import type { PaneModel } from "../../types";
import { desksWithoutFixtureSheetCompactModes } from "../../components/shell/fixtureSheetCompactModePersistence";
import { appReducer, initialState } from "../appReducer";

function layout() {
	const pane = (id: string, mode: "screen" | "free-region"): PaneModel => ({
		id,
		kind: "playbacks",
		title: id,
		x: 1,
		y: 1,
		width: 8,
		height: 8,
		surface_id: `pane:${id}`,
		playbackViewMode: mode,
		playbackScreenSurfaceId: "screen:second",
	});
	return appReducer(initialState, {
		type: "HYDRATE_LAYOUT",
		activeDeskId: "active",
		desks: [
			{
				id: "active",
				name: "Active",
				panes: [pane("free", "free-region"), pane("screen", "screen")],
			},
			{
				id: "inactive",
				name: "Inactive",
				panes: [pane("elsewhere", "free-region")],
			},
		],
	});
}

function reopen(state: ReturnType<typeof layout>) {
	// This is the exact portable pane projection used by LayoutPersistence for save/export.
	const stored = JSON.parse(
		JSON.stringify(desksWithoutFixtureSheetCompactModes(state.desks)),
	);
	return appReducer(initialState, {
		type: "HYDRATE_LAYOUT",
		desks: stored,
		activeDeskId: state.activeDeskId,
	});
}

describe("Playback pane control presentation persistence", () => {
	it("loads older panes as tiles without inventing stored metadata", () => {
		for (const pane of reopen(layout()).desks[0].panes) {
			expect(pane.playbackShowControls ?? false).toBe(false);
			expect(pane).not.toHaveProperty("playbackShowControls");
		}
	});

	it.each([
		"free",
		"screen",
	])("saves explicit controls and tiles only for addressed %s pane", (id) => {
		const before = layout();
		const index = id === "free" ? 0 : 1;
		const changed = appReducer(before, {
			type: "SET_PANE_PLAYBACK_CONTROLS",
			id,
			showControls: true,
		});
		expect(changed.desks[0].panes[index]).toEqual({
			...before.desks[0].panes[index],
			playbackShowControls: true,
		});
		expect(changed.desks[0].panes[1 - index]).toBe(
			before.desks[0].panes[1 - index],
		);
		expect(changed.desks[1]).toBe(before.desks[1]);
		const restored = reopen(changed);
		expect(restored.desks[0].panes[index]).toEqual(
			changed.desks[0].panes[index],
		);
		const tiles = appReducer(restored, {
			type: "SET_PANE_PLAYBACK_CONTROLS",
			id,
			showControls: false,
		});
		expect(reopen(tiles).desks[0].panes[index]).toEqual({
			...before.desks[0].panes[index],
			playbackShowControls: false,
		});
	});

	it("does not change a pane on an inactive Desktop", () => {
		const before = layout();
		const after = appReducer(before, {
			type: "SET_PANE_PLAYBACK_CONTROLS",
			id: "elsewhere",
			showControls: true,
		});
		expect(after.desks).toEqual(before.desks);
	});
});
