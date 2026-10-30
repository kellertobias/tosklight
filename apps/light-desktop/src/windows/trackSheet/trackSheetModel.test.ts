import { describe, expect, it } from "vitest";
import type { AttributeValue, Cue, CueList } from "../../api/types/playback";
import { editTrackValue, removeTrackValue } from "./trackSheetEdits";
import {
	addressKey,
	buildTrackSheet,
	filterTrackSheet,
	type TrackSheetContext,
	trackCell,
} from "./trackSheetModel";

const level = (value: number): AttributeValue => ({ kind: "normalized", value });

function cue(number: string, changes: Cue["changes"], extra: Partial<Cue> = {}): Cue {
	return {
		id: `cue-${number}`,
		number,
		name: "",
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		changes,
		...extra,
	};
}

function list(cues: Cue[]): CueList {
	return { id: "list", name: "Main", cues, mode: "sequence", priority: 0, looped: false };
}

const context: TrackSheetContext = {
	groupMembers: new Map([["front", ["f1", "f2"]]]),
	attributeFamily: (attribute) =>
		attribute === "intensity" ? "intensity" : attribute === "color" ? "color" : undefined,
	fixtureOrder: ["f1", "f2"],
};

const states = (sheet: ReturnType<typeof buildTrackSheet>, fixture: string, attribute = "intensity") => {
	const row = sheet.rows.find((item) => item.key === addressKey(fixture, attribute));
	if (!row) throw new Error(`no row ${fixture}`);
	return sheet.cues.map((_, index) => trackCell(row, index).state);
};

describe("Track Sheet model", () => {
	const show = list([
		cue("1", [{ fixture_id: "f1", attribute: "intensity", value: level(0.5) }]),
		cue("2", [{ fixture_id: "f2", attribute: "intensity", value: level(1) }]),
		cue("3", [{ fixture_id: "f1", attribute: "intensity", value: level(0.8) }]),
		cue("4", [{ fixture_id: "f1", attribute: "intensity", value: level(0.8) }]),
		cue("5", [{ fixture_id: "f1", attribute: "intensity", value: level(0.2) }]),
		cue("6", [{ fixture_id: "f1", attribute: "intensity", value: null }]),
	]);

	it("marks new, tracked, up, blocked, down, released and empty cells", () => {
		const sheet = buildTrackSheet(show, context);
		expect(states(sheet, "f1")).toEqual(["hard", "tracked", "up", "block", "down", "released"]);
		expect(states(sheet, "f2")).toEqual(["empty", "hard", "tracked", "tracked", "tracked", "tracked"]);
		const row = sheet.rows[0];
		expect(trackCell(row, 1)).toMatchObject({ state: "tracked", value: level(0.5), sourceCue: 0 });
	});

	it("shows Group values per member, lets a fixture value win, and marks Cue-only restores", () => {
		const sheet = buildTrackSheet(
			list([
				cue("1", [{ fixture_id: "f2", attribute: "intensity", value: level(0.3) }], {
					group_changes: [{ group_id: "front", attribute: "intensity", value: level(0.6) }],
					cue_only: true,
				}),
				cue("2", [{ fixture_id: "f1", attribute: "intensity", value: null, automatic_restore: true }]),
			]),
			context,
		);
		const f1 = sheet.rows.find((row) => row.fixtureId === "f1")!;
		const f2 = sheet.rows.find((row) => row.fixtureId === "f2")!;
		expect(trackCell(f1, 0)).toMatchObject({ state: "hard", groupId: "front", cueOnly: true });
		expect(trackCell(f2, 0)).toMatchObject({ value: level(0.3), groupId: null });
		expect(trackCell(f1, 1).state).toBe("restore");
	});

	it("filters by fixtures, attribute family and changes only", () => {
		const withColor = list([
			...show.cues,
			cue("7", [{ fixture_id: "f2", attribute: "color", value: { kind: "discrete", value: "red" } }]),
		]);
		const sheet = buildTrackSheet(withColor, context);
		expect(filterTrackSheet(sheet, { fixtures: new Set(["f2"]), family: "all", changesOnly: false }).rows.map((row) => row.key))
			.toEqual([addressKey("f2", "intensity"), addressKey("f2", "color")]);
		expect(filterTrackSheet(sheet, { fixtures: null, family: "color", changesOnly: false }).rows).toHaveLength(1);
		expect(filterTrackSheet(sheet, { fixtures: new Set(["f2"]), family: "intensity", changesOnly: true }).columns).toEqual([1]);
	});

	it("builds a 500 fixture × 300 Cue list quickly", () => {
		const fixtures = Array.from({ length: 500 }, (_, index) => `fixture-${index}`);
		const cues = Array.from({ length: 300 }, (_, index) =>
			cue(String(index + 1), fixtures
				.filter((_, fixture) => (fixture + index) % 7 === 0)
				.map((fixture_id) => ({ fixture_id, attribute: "intensity", value: level(((index % 10) + 1) / 10) }))),
		);
		const started = performance.now();
		const sheet = buildTrackSheet(list(cues), { ...context, fixtureOrder: fixtures });
		const elapsed = performance.now() - started;
		expect(sheet.rows).toHaveLength(500);
		expect(elapsed).toBeLessThan(1500);
		// Reading any one cell is a binary search, not a walk through every Cue.
		const lookups = performance.now();
		for (let index = 0; index < 20_000; index++) trackCell(sheet.rows[index % 500], index % 300);
		expect(performance.now() - lookups).toBeLessThan(500);
	});
});

describe("Track Sheet edits", () => {
	const base = list([
		cue("1", [{ fixture_id: "f1", attribute: "intensity", value: level(0.5), fade_millis: 2000 }]),
		cue("2", []),
		cue("3", []),
	]);

	it("updates a stored value in place, keeping its timing (Update)", () => {
		const next = editTrackValue(base, context, { cue: 0, fixtureId: "f1", attribute: "intensity", value: level(0.7), mode: "cue" });
		expect(next.cues[0].changes).toEqual([
			{ fixture_id: "f1", attribute: "intensity", value: level(0.7), fade_millis: 2000, preset_reference: null, replacement_projection: null },
		]);
	});

	it("writes a tracked value into this Cue (Update All) or its source Cue (Update Tracked)", () => {
		const here = editTrackValue(base, context, { cue: 1, fixtureId: "f1", attribute: "intensity", value: level(0.9), mode: "cue" });
		expect(here.cues[1].changes).toEqual([{ fixture_id: "f1", attribute: "intensity", value: level(0.9) }]);
		expect(here.cues[0]).toBe(base.cues[0]);
		const source = editTrackValue(base, context, { cue: 2, fixtureId: "f1", attribute: "intensity", value: level(0.9), mode: "source" });
		expect(source.cues[0].changes[0].value).toEqual(level(0.9));
		expect(source.cues[2]).toBe(base.cues[2]);
	});

	it("stores a Cue-only value and restores the previous tracked value in the next Cue", () => {
		const next = editTrackValue(base, context, { cue: 1, fixtureId: "f1", attribute: "intensity", value: level(1), mode: "cue_only" });
		expect(next.cues[1].changes[0].value).toEqual(level(1));
		expect(next.cues[2].changes).toEqual([{ fixture_id: "f1", attribute: "intensity", value: level(0.5) }]);
		const sheet = buildTrackSheet(next, context);
		expect(sheet.cues.map((_, index) => trackCell(sheet.rows[0], index).value)).toEqual([level(0.5), level(1), level(0.5)]);
		const first = editTrackValue(base, context, { cue: 0, fixtureId: "f2", attribute: "intensity", value: level(1), mode: "cue_only" });
		expect(first.cues[1].changes).toEqual([{ fixture_id: "f2", attribute: "intensity", value: null }]);
	});

	it("removes only the value this Cue stores", () => {
		const next = removeTrackValue(base, { cue: 0, fixtureId: "f1", attribute: "intensity" });
		expect(next.cues[0].changes).toEqual([]);
		expect(() => removeTrackValue(base, { cue: 1, fixtureId: "f1", attribute: "intensity" })).toThrow();
	});
});

describe("Track Sheet values", () => {
	it("names a referenced Preset instead of its raw value, and flags a missing one", async () => {
		const { trackValueText } = await import("./trackSheetValues");
		const reference = { preset_instance_id: "warm", source_owner: { type: "universal" as const }, source_attribute: "color" };
		const presets = new Map([["warm", { name: "Warm Front", family: "Color", number: 3 }]]);
		expect(trackValueText(level(0.4), reference, presets)).toBe("Warm Front");
		expect(trackValueText(level(0.4), { ...reference, preset_instance_id: "gone" }, presets)).toBe("Missing Preset");
		expect(trackValueText(level(0.4), null, presets)).toBe("40%");
		expect(trackValueText({ kind: "position", value: { kind: "angles", pan_degrees: { kind: "value", value: 45 }, tilt_degrees: { kind: "value", value: -10 } } }, null, presets)).toBe("P 45° T -10°");
	});
});
