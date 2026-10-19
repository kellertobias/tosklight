import { describe, expect, it } from "vitest";
import type { PlaybackPage } from "../../../api/types";
import { projectPlaybackSlots } from "./projection";

describe("pane tile footprint presentation", () => {
	it("shows separate square positions while retaining the saved wider control footprint", () => {
		const page: PlaybackPage = {
			number: 1, name: "Main", assignments: {
				"401": { name: "Opening", buttons: ["go_minus", "go", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0, target: { type: "cue_list", cue_list_id: "opening" },
					footprint: { type: "wider", right_buttons: ["flash", "none", "none"], right_fader: "master" } },
			},
		};
		const options = { page, cueLists: [], groups: [], columns: 2, firstSlot: 401, pageSize: 2,
			playbackLayout: { rows: [{ first_position: 401, count: 2, presentation: "buttons" as const, button_count: 3 }] } };
		const saved = JSON.stringify(page);
		expect(projectPlaybackSlots(options).map(cell => cell.slot)).toEqual([401]);
		const tiles = projectPlaybackSlots({ ...options, ignoreFootprints: true });
		expect(tiles.map(cell => cell.slot)).toEqual([401, 402]);
		expect(tiles.every(cell => cell.footprint === null)).toBe(true);
		expect(JSON.stringify(page)).toBe(saved);
		expect(projectPlaybackSlots(options)[0].footprint?.columnSpan).toBe(2);
	});
});
