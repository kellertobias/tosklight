import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../api/types";
import {
	channelFaderDisabledReason,
	channelFixtureLabel,
	channelPatchUnavailableReason,
	channelProjection,
} from "./ChannelsWindow";

function fixture(
	number: number,
	attributes: string[],
	name?: string | null,
): PatchedFixture {
	return {
		fixture_id: `fixture-${number}`,
		fixture_number: number,
		name: name ?? `Wash ${number}`,
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
	it("orders intensity faders by fixture ID and labels them with the fixture name", () => {
		const channels = channelProjection(
			[fixture(20, ["intensity"]), fixture(2, ["intensity"])],
			null,
		);

		expect(
			channels.map(({ fixtureLabel, fixtureId, attributeLabel, level }) => ({
				fixtureLabel,
				fixtureId,
				attributeLabel,
				level,
			})),
		).toEqual([
			{
				fixtureLabel: "Wash 2",
				fixtureId: "2",
				attributeLabel: "Intensity",
				level: 25,
			},
			{
				fixtureLabel: "Wash 20",
				fixtureId: "20",
				attributeLabel: "Intensity",
				level: 25,
			},
		]);
	});

	it("falls back to the profile name, then the Fixture ID, for an unnamed fixture", () => {
		const unnamed = fixture(7, ["intensity"], "");
		(unnamed.definition as { name: string }).name = "";
		expect(channelFixtureLabel(unnamed)).toBe("Fixture 7");

		const profileNamed = fixture(8, ["intensity"], "   ");
		(profileNamed.definition as { name: string }).name = "Generic Dimmer";
		expect(channelFixtureLabel(profileNamed)).toBe("Generic Dimmer");
	});

	it("uses the virtual Fixture ID as the last-resort label", () => {
		const virtual = {
			...fixture(0, ["intensity"], ""),
			virtual_fixture_number: 1,
		} as PatchedFixture;
		(virtual.definition as { name: string }).name = "";
		expect(channelFixtureLabel(virtual)).toBe("Fixture 0.1");
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
			["Wash 2", "pan", "Pan"],
			["Wash 2", "intensity", "Intensity"],
			["Wash 20", "color.red", "Red"],
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
