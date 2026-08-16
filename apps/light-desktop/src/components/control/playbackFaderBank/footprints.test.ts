import { describe, expect, it } from "vitest";
import type {
	PlaybackDefinition,
	PlaybackPage,
	PlaybackSurfaceLayout,
} from "../../../api/types";
import type { ShowObject } from "../../../features/showObjects/contracts";
import { projectPlaybackFootprints } from "./footprints";

const layout: PlaybackSurfaceLayout = {
	playbacks_per_row: 3,
	rows: [
		{ first_playback_slot: 11, has_fader: false, button_count: 1 },
		{ first_playback_slot: 31, has_fader: true, button_count: 3 },
	],
};

function definition(
	number: number,
	footprint:
		| { type: "normal" }
		| { type: "taller"; upper_button: "go" }
		| {
				type: "wider";
				right_buttons: PlaybackDefinition["buttons"];
				right_fader: PlaybackDefinition["fader"];
		  },
): ShowObject<"playback"> {
	const body: PlaybackDefinition & { footprint: typeof footprint } = {
		number,
		name: `Playback ${number}`,
		target: { type: "cue_list", cue_list_id: `cue-list-${number}` },
		buttons: ["go_minus", "go", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		footprint,
	};
	return {
		kind: "playback",
		id: `playback-${number}`,
		revision: 1,
		updated_at: "",
		body,
	};
}

function page(slots: Record<string, number>): PlaybackPage {
	return { number: 1, name: "Main", slots, virtual_playbacks: {} };
}

function project(
	definitions: ShowObject<"playback">[],
	slots: Record<string, number>,
	playbackLayout: PlaybackSurfaceLayout = layout,
) {
	return projectPlaybackFootprints({
		playbackDefinitions: definitions,
		page: page(slots),
		playbackLayout,
		columns: playbackLayout.playbacks_per_row,
		firstSlot: 1,
		pageSize: 6,
	});
}

describe("playback footprint projection", () => {
	it("keeps normal anchors and nonsequential empty cells independently available", () => {
		const result = project([definition(1, { type: "normal" })], { "31": 1 });

		expect(result.cells.map(({ slot }) => slot)).toEqual([
			11, 12, 13, 31, 32, 33,
		]);
		expect(result.anchors).toMatchObject([
			{
				slot: 31,
				state: "anchor",
				requested: "normal",
				effective: "normal",
				claimedSlots: [],
				rowStart: 2,
				columnStart: 1,
				rowSpan: 1,
				columnSpan: 1,
			},
		]);
		expect(result.unclaimed.map(({ slot }) => slot)).toEqual([
			11, 12, 13, 32, 33,
		]);
		expect(result.bindings.get(31)).toMatchObject({
			physicalSlot: 31,
			anchorSlot: 31,
			position: "anchor",
		});
	});

	it("lets the lower anchor claim the topmost button in the aligned upper cell", () => {
		const result = project(
			[definition(4, { type: "taller", upper_button: "go" })],
			{ "32": 4 },
		);

		expect(result.anchors).toMatchObject([
			{
				slot: 32,
				requested: "taller",
				effective: "taller",
				claimedSlots: [12],
				rowStart: 1,
				columnStart: 2,
				rowSpan: 2,
				columnSpan: 1,
			},
		]);
		expect(result.claimed).toMatchObject([
			{
				slot: 12,
				anchorSlot: 32,
				position: "taller_upper",
			},
		]);
		expect(result.bindings.get(12)).toEqual({
			physicalSlot: 12,
			anchorSlot: 32,
			playbackNumber: 4,
			position: "taller_upper",
			controls: [{ physicalButton: 1, control: { type: "taller_button" } }],
		});
	});

	it("lets the left anchor claim only its immediate right cell and second fader", () => {
		const result = project(
			[
				definition(7, {
					type: "wider",
					right_buttons: ["go_minus", "go", "flash"],
					right_fader: "x_fade",
				}),
			],
			{ "31": 7 },
		);

		expect(result.anchors[0]).toMatchObject({
			slot: 31,
			effective: "wider",
			claimedSlots: [32],
			columnStart: 1,
			columnSpan: 2,
		});
		expect(result.bindings.get(32)).toEqual({
			physicalSlot: 32,
			anchorSlot: 31,
			playbackNumber: 7,
			position: "wider_right",
			controls: [
				{
					physicalButton: 1,
					control: { type: "right_button", number: 1 },
				},
				{
					physicalButton: 2,
					control: { type: "right_button", number: 2 },
				},
				{
					physicalButton: 3,
					control: { type: "right_button", number: 3 },
				},
				{ control: { type: "right_fader" } },
			],
		});
	});

	it.each([
		1, 2,
	] as const)("binds only %i right-side button(s) for that Playback topology", (buttonCount) => {
		const playback = definition(8, {
			type: "wider",
			right_buttons: ["go_minus", "go", "flash"],
			right_fader: "x_fade",
		});
		playback.body.button_count = buttonCount;
		const result = project([playback], { "31": 8 });
		const controls = result.bindings.get(32)?.controls ?? [];

		expect(
			controls.filter(({ control }) => control.type === "right_button"),
		).toHaveLength(buttonCount);
		expect(controls.at(-1)).toEqual({
			control: { type: "right_fader" },
		});
	});

	it("falls back at boundaries, on incompatible upper rows, and on occupied neighbors", () => {
		const incompatible: PlaybackSurfaceLayout = {
			...layout,
			rows: [{ ...layout.rows[0], button_count: 0 }, layout.rows[1]],
		};
		const taller = definition(1, { type: "taller", upper_button: "go" });
		const wider = definition(2, {
			type: "wider",
			right_buttons: ["none", "none", "none"],
			right_fader: "master",
		});
		const normal = definition(3, { type: "normal" });

		expect(project([taller], { "11": 1 }).anchors[0]).toMatchObject({
			effective: "normal",
			fallbackReason: "upper_neighbor_out_of_range",
		});
		expect(
			project([taller], { "31": 1 }, incompatible).anchors[0],
		).toMatchObject({
			effective: "normal",
			fallbackReason: "upper_neighbor_incompatible",
		});
		expect(project([wider], { "33": 2 }).anchors[0]).toMatchObject({
			effective: "normal",
			fallbackReason: "right_neighbor_out_of_range",
		});
		expect(
			project([wider, normal], { "31": 2, "32": 3 }).anchors,
		).toMatchObject([
			{ slot: 31, effective: "normal", fallbackReason: "neighbor_occupied" },
			{ slot: 32, effective: "normal", fallbackReason: null },
		]);
		expect(project([wider], { "31": 2, "32": 99 }).anchors[0]).toMatchObject({
			effective: "normal",
			fallbackReason: "neighbor_occupied",
		});
	});

	it("falls every claimant back when wider and taller footprints want the same cell", () => {
		const wider = definition(1, {
			type: "wider",
			right_buttons: ["none", "none", "none"],
			right_fader: "master",
		});
		const taller = definition(2, { type: "taller", upper_button: "go" });
		const result = project([wider, taller], { "11": 1, "32": 2 });

		expect(result.claimed).toEqual([]);
		expect(result.anchors).toMatchObject([
			{
				slot: 11,
				requested: "wider",
				effective: "normal",
				fallbackReason: "conflicting_claim",
			},
			{
				slot: 32,
				requested: "taller",
				effective: "normal",
				fallbackReason: "conflicting_claim",
			},
		]);
		expect(result.unclaimed.map(({ slot }) => slot)).toEqual([12, 13, 31, 33]);
	});

	it("preserves requested footprint data while a smaller surface falls back and restores it", () => {
		const playback = definition(9, {
			type: "wider",
			right_buttons: ["go_minus", "go", "flash"],
			right_fader: "x_fade",
		});
		const before = structuredClone(playback.body);
		const small = projectPlaybackFootprints({
			playbackDefinitions: [playback],
			page: page({ "1": 9 }),
			columns: 1,
			firstSlot: 1,
			pageSize: 1,
		});
		const restored = project([playback], { "31": 9 });

		expect(small.anchors[0]).toMatchObject({
			requested: "wider",
			effective: "normal",
			fallbackReason: "right_neighbor_out_of_range",
		});
		expect(restored.anchors[0]).toMatchObject({
			requested: "wider",
			effective: "wider",
			claimedSlots: [32],
		});
		expect(playback.body).toEqual(before);
	});
});
