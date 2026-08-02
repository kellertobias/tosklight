import { describe, expect, it } from "vitest";
import { plannedDemoLayout } from "../../support/plannedDemoLayouts";

describe("Plan 76 desktop layout", () => {
	it("persists literal Busking, Programming, and Theater pane composition", () => {
		const layout = plannedDemoLayout();
		expect(layout.desks.map((desk) => desk.name)).toEqual([
			"Group Programming",
			"Busking",
			"Cue Programming",
			"Programming",
			"Theater",
		]);
		expect(layout.desks[0].panes.map((pane) => pane.kind)).toEqual([
			"fixtures",
			"groups",
		]);
		expect(layout.desks[0].panes[1]).toMatchObject({
			width: 8,
			poolColumns: 7,
		});
		expect(layout.desks[1].panes.map((pane) => pane.kind)).toEqual([
			"groups",
			"presets",
			"presets",
			"presets",
			"virtual_playbacks",
		]);
		expect(layout.desks[1].panes.at(-1)).toMatchObject({
			virtualPlaybackRows: 5,
			virtualPlaybackColumns: 10,
		});
		expect(layout.desks[2].panes.map((pane) => pane.kind)).toEqual([
			"fixtures",
			"cuelist_pool",
			"cues",
		]);
		expect(layout.desks[2].panes[0]).toMatchObject({
			fixtureSheetActiveOnly: true,
		});
		expect(layout.desks[2].panes[2]).toMatchObject({
			cueListSource: "follow-selection",
		});
		expect(layout.desks[3].panes.map((pane) => pane.kind)).toEqual([
			"fixtures",
			"stage",
			"dmx",
		]);
		expect(layout.desks[3].panes[1]).toMatchObject({
			stageView: "3d",
			showBeamGuides: true,
			stageRenderQuality: "lines_and_beams",
		});
		expect(layout.desks[4].panes.map((pane) => pane.kind)).toEqual([
			"cue_list",
			"text_editor",
		]);
		for (const desk of layout.desks)
			for (const pane of desk.panes) {
				expect(pane.x).toBeGreaterThanOrEqual(1);
				expect(pane.y).toBeGreaterThanOrEqual(1);
				expect(pane.x + pane.width - 1).toBeLessThanOrEqual(24);
				expect(pane.y + pane.height - 1).toBeLessThanOrEqual(18);
			}
	});
});
