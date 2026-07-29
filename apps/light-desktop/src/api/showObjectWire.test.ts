import { describe, expect, it } from "vitest";
import { decodeShowObject } from "./showObjectWire";
import { WireValidationError } from "./wireValidation";

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

	it("keeps a legacy Cuelist storage key separate from its semantic ID", () => {
		const decoded = decodeShowObject(
			versioned("cue_list", "legacy-main-list", cueListBody()),
			"cue_list",
		);

		expect(decoded.id).toBe("legacy-main-list");
		expect(decoded.body.id).toBe(CUE_LIST_ID);
	});

	it("decodes current Playback defaults and dedicated Virtual page topology", () => {
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
				virtual_playbacks: {},
			}),
			"playback_page",
		);

		expect(playback.body).toMatchObject({
			buttons: ["go_minus", "go", "flash"],
			button_count: 3,
			fader: "master",
			has_fader: true,
			go_activates: true,
			auto_off: true,
		});
		expect(page.body.slots).toEqual({ 1: 7 });
		expect(page.body.virtual_playbacks).toEqual({});
	});

	it("uses target-aware defaults and migrates a legacy Speed Group fader", () => {
		const missing = decodeShowObject(
			versioned("playback", "speed-a", {
				number: 8,
				name: "Speed A",
				target: { type: "speed_group", group: "A" },
			}),
			"playback",
		);
		const legacy = decodeShowObject(
			versioned("playback", "speed-b", {
				number: 9,
				name: "Speed B",
				target: { type: "speed_group", group: "B" },
				buttons: ["double", "half", "learn"],
				fader: "speed",
			}),
			"playback",
		);

		expect(missing.body).toMatchObject({
			buttons: ["double", "half", "learn"],
			fader: "learned_percentage",
		});
		expect(legacy.body.fader).toBe("learned_percentage");
	});

	it.each([
		["group", "1", { name: "Front", fixtures: ["fixture-1"] }],
		["preset", "1", { name: "Open", family: "Dimmer", number: 1, values: {} }],
		["patch_layer", "base", { id: "base", name: "Base", order: 0 }],
		[
			"stage_layout",
			"stage",
			{
				version: 2,
				positions: { "fixture-1": { x: 1, y: 2, rotation: 0 } },
				positions3d: {
					"fixture-1": {
						x: 1,
						y: 2,
						z: 3,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
				camera3d: { position: [0, 1, 2], target: [0, 0, 0] },
			},
		],
		[
			"user_layout",
			"user",
			{
				desks: [
					{
						id: "desk-1",
						name: "Main",
						panes: [
							{
								id: "stage-1",
								kind: "stage",
								title: "Stage",
								x: 1,
								y: 1,
								width: 12,
								height: 9,
							},
						],
					},
				],
				activeDeskId: "desk-1",
			},
		],
	] as const)("decodes the typed %s family", (kind, id, body) => {
		const decoded = decodeShowObject(versioned(kind, id, body), kind);
		expect(decoded.kind).toBe(kind);
		expect(decoded.body).toMatchObject(body);
	});

	it.each([
		["mismatched object kind", versioned("preset", CUE_LIST_ID, cueListBody())],
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

	it("rejects a legacy Virtual Playbacks pane without the dedicated address schema", () => {
		const body = {
			desks: [
				{
					id: "desk-1",
					name: "Main",
					panes: [
						{
							id: "virtual-1",
							kind: "virtual_playbacks",
							title: "Virtual Playbacks",
							x: 1,
							y: 1,
							width: 12,
							height: 9,
							virtualPlaybackRows: 2,
							virtualPlaybackColumns: 2,
						},
					],
				},
			],
			activeDeskId: "desk-1",
		};
		expect(() =>
			decodeShowObject(versioned("user_layout", "user", body), "user_layout"),
		).toThrow(/virtualPlaybackPageMode/u);
	});
});
