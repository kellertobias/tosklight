import { describe, expect, it } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import {
	browserScreenUrl,
	createScreenConfiguration,
	playbackLayoutLegacyFields,
	screenForAddAction,
	updateScreenConfiguration,
} from "./screenConfiguration";

const layout = { desks: [], activeDeskId: "main" };
const configuredScreen: ScreenConfiguration = {
	id: "configured",
	name: "Stage manager",
	layout,
	content: { type: "desktop" },
	show_dock: false,
	show_playbacks: true,
	playback_count: 12,
	playback_rows: 2,
	first_playback_slot: 9,
	page_mode: "independent",
	show_page_controls: false,
	desired_open: false,
	display_id: "display-2",
	bounds: { x: 10, y: 20, width: 900, height: 700 },
	fullscreen: true,
};

describe("Add Screen action", () => {
	it("builds a stable browser route without carrying incompatible app modes", () => {
		const url = new URL(
			browserScreenUrl(
				"screen-1",
				"https://desk.example.test/light?demo=product&frontend-warmup-disabled=1#old",
			),
		);

		expect(url.searchParams.get("screen")).toBe("screen-1");
		expect(url.searchParams.get("demo")).toBeNull();
		expect(url.searchParams.get("frontend-warmup-disabled")).toBe("1");
		expect(url.hash).toBe("");
	});

	it("opens the first configured screen that is currently closed", () => {
		const result = screenForAddAction(
			[configuredScreen],
			layout,
			() => "unused",
		);

		expect(result).toEqual({ ...configuredScreen, desired_open: true });
	});

	it("creates an open default screen when every configured screen is open", () => {
		const result = screenForAddAction(
			[{ ...configuredScreen, desired_open: true }],
			layout,
			() => "new-screen",
		);

		expect(result).toMatchObject({
			id: "new-screen",
			name: "Screen 2",
			layout,
			first_playback_slot: 81,
			playback_count: 40,
			playback_rows: 4,
			desired_open: true,
			display_id: null,
			fullscreen: false,
		});
	});

	it("creates the first configured external screen at playback 41", () => {
		expect(
			createScreenConfiguration([], layout, false, () => "first-screen"),
		).toMatchObject({
			id: "first-screen",
			first_playback_slot: 41,
			playback_count: 40,
			playback_rows: 4,
			desired_open: false,
			playback_layout: {
				playbacks_per_row: 10,
				rows: [
					{ first_playback_slot: 41, has_fader: true, button_count: 3 },
					{ first_playback_slot: 51, has_fader: true, button_count: 3 },
					{ first_playback_slot: 61, has_fader: true, button_count: 3 },
					{ first_playback_slot: 71, has_fader: true, button_count: 3 },
				],
			},
		});
	});

	it("projects a row layout into legacy desk and screen fields", () => {
		expect(
			playbackLayoutLegacyFields({
				playbacks_per_row: 6,
				rows: [
					{ first_playback_slot: 1, has_fader: false, button_count: 1 },
					{ first_playback_slot: 21, has_fader: true, button_count: 3 },
				],
			}),
		).toEqual({
			columns: 6,
			rows: 2,
			buttons: 3,
			playback_count: 12,
			playback_rows: 2,
			first_playback_slot: 1,
		});
	});

	it("turns Dock off atomically for fixed panes and does not restore it on leaving", () => {
		const fixed = updateScreenConfiguration(configuredScreen, {
			content: {
				type: "fixed_pane",
				pane: {
					type: "cues",
					cue_list_id: "",
				},
			},
		});
		expect(fixed.show_dock).toBe(false);

		expect(
			updateScreenConfiguration(fixed, { content: { type: "desktop" } }),
		).toMatchObject({
			content: { type: "desktop" },
			show_dock: false,
		});
	});

	it("keeps Dock available for pixel-sized side panes", () => {
		const side = updateScreenConfiguration(
			{ ...configuredScreen, show_dock: true },
			{
				content: {
					type: "fixed_side_pane",
					pane: { type: "cues", cue_list_id: "" },
					side: "left",
					width_px: 420,
					base: "desktop",
				},
			},
		);

		expect(side.show_dock).toBe(true);
	});

	it("removes Desktop Dock from a side pane with a non-Desktop base", () => {
		const side = updateScreenConfiguration(
			{ ...configuredScreen, show_dock: true },
			{
				content: {
					type: "fixed_side_pane",
					pane: { type: "cues", cue_list_id: "" },
					side: "right",
					width_px: 420,
					base: "control_surface",
				},
			},
		);

		expect(side.show_dock).toBe(false);
	});

	it("removes Desktop Dock from Control surface and None base content", () => {
		for (const type of ["control_surface", "none"] as const)
			expect(
				updateScreenConfiguration(
					{ ...configuredScreen, show_dock: true },
					{ content: { type } },
				),
			).toMatchObject({ content: { type }, show_dock: false });
	});
});
