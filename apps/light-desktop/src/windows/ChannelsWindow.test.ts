import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../api/types";
import {
	channelFaderDisabledReason,
	channelPatchUnavailableReason,
	channelProjection,
} from "./ChannelsWindow";

function fixture(number: number, attributes: string[]): PatchedFixture {
	return {
		fixture_id: `fixture-${number}`,
		fixture_number: number,
		virtual_fixture_number: null,
		universe: 1,
		address: number,
		logical_heads: [],
		definition: {
			name: `Fixture ${number}`,
			model: `Fixture ${number}`,
			heads: [
				{
					index: 0,
					name: "Main",
					shared: false,
					parameters: attributes.map((attribute) => ({
						attribute,
						components: [],
						default: attribute === "intensity" ? 0.25 : 0,
						virtual_dimmer: false,
						capabilities: [],
					})),
				},
			],
		} as unknown as PatchedFixture["definition"],
	} as PatchedFixture;
}

describe("Channel projection", () => {
	it("orders intensity faders by fixture ID and labels them as fixtures", () => {
		const channels = channelProjection(
			[fixture(20, ["intensity"]), fixture(2, ["intensity"])],
			null,
		);

		expect(
			channels.map(({ fixtureLabel, attributeLabel, level }) => ({
				fixtureLabel,
				attributeLabel,
				level,
			})),
		).toEqual([
			{ fixtureLabel: "2", attributeLabel: "Intensity", level: 25 },
			{ fixtureLabel: "20", attributeLabel: "Intensity", level: 25 },
		]);
	});

	it("groups all channels under each fixture in fixture-ID order", () => {
		const channels = channelProjection(
			[fixture(20, ["color.red"]), fixture(2, ["pan", "intensity"])],
			null,
			"all",
			[
				{ id: "pan", label: "Pan" },
				{ id: "intensity", label: "Intensity" },
				{ id: "color.red", label: "Red" },
			],
		);

		expect(
			channels.map((channel) => [
				channel.fixtureLabel,
				channel.attribute,
				channel.attributeLabel,
			]),
		).toEqual([
			["2", "pan", "Pan"],
			["2", "intensity", "Intensity"],
			["20", "color.red", "Red"],
		]);
	});
});

describe("Channel fader availability", () => {
	it.each([
		[false, "loading", "Channel controls are inactive"],
		[true, "loading", "Patch is loading"],
		[true, "repairing", "Patch is resynchronizing"],
		[true, "error", "Patch is unavailable"],
		[true, "ready", null],
	] as const)("explains active=%s Patch status=%s", (active, status, expected) => {
		expect(channelPatchUnavailableReason(active, { status, error: null })).toBe(
			expected,
		);
	});

	it("distinguishes Patch absence, empty slots, value authority, and recovery", () => {
		expect(
			channelFaderDisabledReason(
				false,
				false,
				"Programmer values are loading",
				"Patch is loading",
			),
		).toBe("Patch is loading");
		expect(channelFaderDisabledReason(false, true, null, null)).toBe(
			"Empty position",
		);
		expect(
			channelFaderDisabledReason(
				true,
				false,
				"Preload values are loading",
				null,
			),
		).toBe("Preload values are loading");
		expect(channelFaderDisabledReason(true, true, null, null)).toBeNull();
	});
});
