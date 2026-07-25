import { describe, expect, it } from "vitest";
import type { PlaybackProjection } from "../playbackRuntime/contracts";
import { cueProjection, SHOW_ID } from "../playbackRuntime/testFixtures";
import type { ShowObject } from "../showObjects/contracts";
import {
	demoFaderLevel,
	demoMappedPlaybackNumbers,
	demoSlotPlaybackNumbers,
} from "./demoPlaybackMapping";

function page(
	number: number,
	slots: Record<string, number>,
): ShowObject<"playback_page"> {
	return {
		kind: "playback_page",
		id: `page-${number}`,
		revision: 1,
		updated_at: "2026-07-19T10:00:00Z",
		body: { number, name: `Page ${number}`, slots },
	};
}

describe("demoSlotPlaybackNumbers", () => {
	it("maps only the demo slots of the exact active desk Page", () => {
		const pages = [
			page(1, { "1": 11, "2": 12, "5": 15, "21": 121 }),
			page(2, { "1": 21, "21": 221 }),
		];
		expect([...demoSlotPlaybackNumbers(pages, 2)]).toEqual([
			[1, 21],
			[21, 221],
		]);
	});

	it("maps nothing while the active desk Page is unresolved", () => {
		const pages = [page(1, { "1": 11 })];
		expect(demoSlotPlaybackNumbers(pages, null).size).toBe(0);
	});

	it("maps nothing when the active desk Page has no portable assignments", () => {
		expect(demoSlotPlaybackNumbers([page(1, { "1": 11 })], 7).size).toBe(0);
	});

	it("skips unassigned demo slots instead of inventing them", () => {
		const mapped = demoSlotPlaybackNumbers([page(1, { "3": 33 })], 1);
		expect(mapped.get(3)).toBe(33);
		expect(mapped.has(1)).toBe(false);
	});
});

describe("demoMappedPlaybackNumbers", () => {
	it("returns each mapped Playback number once, in ascending order", () => {
		const mapped = demoSlotPlaybackNumbers(
			[page(1, { "1": 9, "2": 4, "3": 9, "21": 1 })],
			1,
		);
		expect(demoMappedPlaybackNumbers(mapped)).toEqual([1, 4, 9]);
	});
});

describe("demoFaderLevel", () => {
	it("prefers the authoritative fader position", () => {
		const projection = cueProjection();
		if (projection.target !== "cue_list" || !projection.runtime)
			throw new Error("fixture must expose a cue list runtime");
		projection.runtime.fader_position = 0.25;
		projection.runtime.master = 1;
		expect(demoFaderLevel(projection)).toBe(0.25);
	});

	it("clamps an out-of-range position into the rendered range", () => {
		const projection = cueProjection();
		if (projection.target !== "cue_list" || !projection.runtime)
			throw new Error("fixture must expose a cue list runtime");
		projection.runtime.fader_position = 4;
		expect(demoFaderLevel(projection)).toBe(1);
		projection.runtime.fader_position = -4;
		expect(demoFaderLevel(projection)).toBe(0);
	});

	it("reads zero for an absent, unmapped or non-cue-list Playback", () => {
		const missing: PlaybackProjection = {
			scope: { show_id: SHOW_ID, show_revision: 4 },
			requested: { kind: "playback", playback_number: 3 },
			playback_number: 3,
			target: "missing",
		};
		expect(demoFaderLevel(undefined)).toBe(0);
		expect(demoFaderLevel(missing)).toBe(0);
	});
});
