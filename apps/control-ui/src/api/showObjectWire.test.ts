import { describe, expect, it } from "vitest";
import { WireValidationError } from "./wireValidation";
import { decodeShowObject } from "./showObjectWire";

const CUE_LIST_ID = "11111111-1111-4111-8111-111111111111";
const CUE_ID = "22222222-2222-4222-8222-222222222222";

function versioned(kind: string, id: string, body: unknown) {
	return { kind, id, revision: 4, updated_at: "", body };
}

function cueListBody() {
	return {
		id: CUE_LIST_ID,
		name: "Main",
		priority: 0,
		mode: "sequence",
		looped: false,
		cues: [
			{
				id: CUE_ID,
				number: 1,
				name: "Opening",
				fade_millis: 1000,
				delay_millis: 0,
				trigger: { type: "manual" },
				cue_only: false,
				changes: [
					{
						fixture_id: "33333333-3333-4333-8333-333333333333",
						attribute: "Intensity",
						value: { kind: "normalized", value: 0.75 },
					},
				],
				group_changes: [],
				phasers: [],
			},
		],
		future_field: { retained: true },
	};
}

describe("show-object wire decoders", () => {
	it("strictly decodes CueList bodies while retaining unknown fields", () => {
		const decoded = decodeShowObject(
			versioned("cue_list", CUE_LIST_ID, cueListBody()),
			"cue_list",
		);

		expect(decoded.body).toMatchObject({
			id: CUE_LIST_ID,
			intensity_priority_mode: "htp",
			restart_mode: "first_cue",
			chaser_step_millis: 1000,
			future_field: { retained: true },
		});
		expect(decoded.body.cues[0].changes[0].value).toEqual({
			kind: "normalized",
			value: 0.75,
		});
	});

	it("decodes current Playback and page topology with legacy defaults", () => {
		const playback = decodeShowObject(
			versioned("playback", "7", {
				number: 7,
				name: "Main",
				target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
				buttons: ["go_minus", "go", "flash"],
			}),
			"playback",
		);
		const page = decodeShowObject(
			versioned("playback_page", "4", {
				number: 4,
				name: "Page 4",
				slots: { 1: 7 },
			}),
			"playback_page",
		);

		expect(playback.body).toMatchObject({
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: true,
		});
		expect(page.body.slots).toEqual({ 1: 7 });
	});

	it.each([
		[
			"mismatched object kind",
			versioned("preset", CUE_LIST_ID, cueListBody()),
		],
		[
			"mismatched CueList identity",
			versioned("cue_list", "other", cueListBody()),
		],
		[
			"invalid Cue number",
			versioned("cue_list", CUE_LIST_ID, {
				...cueListBody(),
				cues: [{ ...cueListBody().cues[0], number: 0 }],
			}),
		],
		[
			"invalid Playback target",
			versioned("playback", "7", {
				number: 7,
				name: "Main",
				target: { type: "unknown" },
			}),
		],
	] as const)("rejects %s", (_label, value) => {
		expect(() =>
			decodeShowObject(
				value,
				value.kind === "playback" ? "playback" : "cue_list",
			),
		).toThrow(WireValidationError);
	});
});
