import { describe, expect, it } from "vitest";
import {
	decodeVirtualPlaybackZonesSaveOutcome,
	decodeVirtualPlaybackZonesSnapshot,
	encodeVirtualPlaybackZonesSaveRequest,
	VirtualPlaybackZonesProtocolError,
} from "./wire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const OTHER_ID = "33333333-3333-4333-8333-333333333333";
const SCOPE = { showId: SHOW_ID, deskId: DESK_ID };

function zone() {
	return { id: "stage-left", name: "Stage Left", slots: [1, 2] };
}

function snapshot(overrides: Record<string, unknown> = {}) {
	return {
		show_id: SHOW_ID,
		desk_id: DESK_ID,
		surfaces: { "surface-a": [zone()] },
		...overrides,
	};
}

describe("Virtual Playback exclusion-zone wire", () => {
	it("decodes an exact authority-scoped snapshot", () => {
		expect(decodeVirtualPlaybackZonesSnapshot(snapshot(), SCOPE)).toEqual({
			showId: SHOW_ID,
			deskId: DESK_ID,
			surfaces: { "surface-a": [zone()] },
		});
	});

	it.each([
		["show", { show_id: OTHER_ID }],
		["desk", { desk_id: OTHER_ID }],
	])("rejects a foreign %s snapshot", (_label, override) => {
		expect(() =>
			decodeVirtualPlaybackZonesSnapshot(snapshot(override), SCOPE),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it.each([
		["unexpected field", snapshot({ extra: true })],
		["untrimmed surface", snapshot({ surfaces: { " surface-a": [zone()] } })],
		[
			"duplicate zone ids",
			snapshot({ surfaces: { "surface-a": [zone(), zone()] } }),
		],
		[
			"duplicate cells",
			snapshot({
				surfaces: {
					"surface-a": [{ ...zone(), slots: [1, 1] }],
				},
			}),
		],
		[
			"cell above the persisted grid domain",
			snapshot({
				surfaces: {
					"surface-a": [{ ...zone(), slots: [1, 145] }],
				},
			}),
		],
		[
			"unknown zone field",
			snapshot({
				surfaces: {
					"surface-a": [{ ...zone(), color: "red" }],
				},
			}),
		],
	])("rejects malformed data: %s", (_label, value) => {
		expect(() => decodeVirtualPlaybackZonesSnapshot(value, SCOPE)).toThrow(
			VirtualPlaybackZonesProtocolError,
		);
	});

	it("decodes only the requested save surface", () => {
		expect(
			decodeVirtualPlaybackZonesSaveOutcome(
				{
					show_id: SHOW_ID,
					desk_id: DESK_ID,
					surface_id: "surface-a",
					zones: [zone()],
				},
				SCOPE,
				"surface-a",
			),
		).toEqual({ surfaceId: "surface-a", zones: [zone()] });
		expect(() =>
			decodeVirtualPlaybackZonesSaveOutcome(
				{
					show_id: SHOW_ID,
					desk_id: DESK_ID,
					surface_id: "surface-b",
					zones: [zone()],
				},
				SCOPE,
				"surface-a",
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it.each([
		["show", { show_id: OTHER_ID, desk_id: DESK_ID }],
		["desk", { show_id: SHOW_ID, desk_id: OTHER_ID }],
	])("rejects a foreign %s save outcome", (_label, identity) => {
		expect(() =>
			decodeVirtualPlaybackZonesSaveOutcome(
				{ ...identity, surface_id: "surface-a", zones: [zone()] },
				SCOPE,
				"surface-a",
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it("validates an outgoing save before serialization", () => {
		expect(encodeVirtualPlaybackZonesSaveRequest([zone()])).toEqual({
			zones: [zone()],
		});
		expect(() =>
			encodeVirtualPlaybackZonesSaveRequest([
				{ ...zone(), name: "", slots: [1, 2] },
			]),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it("round-trips retained legacy cells above the assignable slot limit", () => {
		const legacy = { ...zone(), slots: [128, 144] };
		expect(
			decodeVirtualPlaybackZonesSnapshot(
				snapshot({ surfaces: { "surface-a": [legacy] } }),
				SCOPE,
			).surfaces["surface-a"],
		).toEqual([legacy]);
		expect(encodeVirtualPlaybackZonesSaveRequest([legacy])).toEqual({
			zones: [legacy],
		});
	});
});
