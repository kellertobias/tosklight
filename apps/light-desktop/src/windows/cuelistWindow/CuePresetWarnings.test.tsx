import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Cue, StoredPreset, PatchedFixture } from "../../api/types";
import { ShowObjectsStateProvider } from "../../features/showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../../features/showObjects/store";
import {
	CuePresetWarnings,
	missingCuePresetSources,
} from "./CuePresetWarnings";

vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: vi.fn(),
}));
vi.mock("../../features/patch/PatchState", () => ({
	usePatchedFixturesView: () => [],
	usePatchStatus: () => ({ status: "ready", error: null }),
}));
afterEach(cleanup);

const cue: Cue = {
	id: "cue",
	number: "1",
	name: "",
	fade_millis: 0,
	delay_millis: 0,
	trigger: { type: "manual" },
	changes: [
		{
			fixture_id: "lamp",
			attribute: "intensity",
			value: null,
			preset_reference: {
				preset_instance_id: "original",
				source_owner: { type: "universal" },
				source_attribute: "intensity",
			},
		},
	],
};
const preset: StoredPreset = {
	instance_id: "original",
	number: 1,
	name: "Wash",
	values: {},
	universal_values: { intensity: 0 },
};

describe("Cue Preset source warnings", () => {
	it("recognizes derived Aim, waits for Patch and warns on deleted target or source", () => {
		const aim = { ...preset, aim_at_fixture_number: 5, universal_values: {} };
		const linked: Cue = {
			...cue,
			changes: [
				{
					...cue.changes[0],
					attribute: "position",
					preset_reference: {
						...cue.changes[0].preset_reference!,
						source_attribute: "position",
					},
				},
			],
		};
		const fixture = {
			fixture_id: "target",
			fixture_number: 5,
			definition: { heads: [] },
		} as unknown as PatchedFixture;
		expect(missingCuePresetSources([linked], [aim])).toEqual([]);
		expect(missingCuePresetSources([linked], [aim], [fixture])).toEqual([]);
		expect(
			missingCuePresetSources(
				[linked],
				[aim],
				[{ ...fixture, rotation: { x: NaN, y: 0, z: 0 } }],
			)[0],
		).toContain("Aim target is missing or invalid");
		expect(missingCuePresetSources([linked], [aim], [])[0]).toContain(
			"Aim target is missing",
		);
		expect(missingCuePresetSources([linked], [], [fixture])[0]).toContain(
			"Source Preset is missing",
		);
		expect(
			missingCuePresetSources(
				[linked],
				[aim],
				[{ ...fixture, position_master: "deleted-point" }],
			)[0],
		).toContain("Aim target is missing");
	});
	it("uses immutable identity, accepts zero and ignores unrelated literal changes", () => {
		expect(missingCuePresetSources([cue], [{ ...preset, number: 99 }])).toEqual(
			[],
		);
		expect(
			missingCuePresetSources(
				[
					{
						...cue,
						changes: [
							{ fixture_id: "lamp", attribute: "intensity", value: null },
						],
					},
				],
				[],
			),
		).toEqual([]);
		expect(
			missingCuePresetSources(
				[cue],
				[{ ...preset, instance_id: "replacement" }],
			)[0],
		).toContain("Source Preset is missing");
	});
	it("names missing fixture and Group source addresses and preserves fallback explanation", () => {
		const grouped: Cue = {
			...cue,
			changes: [],
			group_changes: [
				{
					group_id: "odd",
					attribute: "color",
					value: null,
					preset_reference: {
						preset_instance_id: "original",
						source_owner: { type: "group", group_id: "odd" },
						source_attribute: "color",
					},
				},
			],
		};
		expect(missingCuePresetSources([grouped], [preset])).toEqual([
			"Cue 1 · Group odd · color: Preset source value is missing. The recorded fallback is used. Restore the source or re-record this Cue value.",
		]);
	});
	it("waits for authoritative collection, then clears locally when the source returns", () => {
		const store = new ShowObjectsStore();
		store.reset("show");
		render(
			<ShowObjectsStateProvider store={store}>
				<CuePresetWarnings cues={[cue]} active />
			</ShowObjectsStateProvider>,
		);
		expect(screen.queryByRole("status")).toBeNull();
		act(() => store.setCollection("show", "preset", [], 1));
		expect(screen.getByRole("status")).toHaveTextContent("recorded fallback");
		act(() =>
			store.setCollection(
				"show",
				"preset",
				[
					{
						id: "2.99",
						kind: "preset",
						revision: 2,
						updated_at: "",
						body: preset,
					},
				],
				2,
			),
		);
		expect(screen.queryByRole("status")).toBeNull();
	});
});
