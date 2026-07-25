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
		desks: { [DESK_ID]: { "surface-a": [zone()] } },
		...overrides,
	};
}

function saveOutcome(overrides: Record<string, unknown> = {}) {
	return {
		request_id: "request-a",
		show_id: SHOW_ID,
		desk_id: DESK_ID,
		surface_id: "surface-a",
		zones: [zone()],
		replayed: false,
		changed: true,
		...overrides,
	};
}

describe("Virtual Playback exclusion-zone wire", () => {
	it("decodes an exact authority-scoped snapshot", () => {
		expect(decodeVirtualPlaybackZonesSnapshot(snapshot(), SCOPE)).toEqual({
			showId: SHOW_ID,
			desks: { [DESK_ID]: { "surface-a": [zone()] } },
		});
	});

	it("rejects a foreign show snapshot", () => {
		expect(() =>
			decodeVirtualPlaybackZonesSnapshot(
				snapshot({ show_id: OTHER_ID }),
				SCOPE,
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it.each([
		["unexpected field", snapshot({ extra: true })],
		[
			"untrimmed surface",
			snapshot({ desks: { [DESK_ID]: { " surface-a": [zone()] } } }),
		],
		[
			"duplicate zone ids",
			snapshot({
				desks: { [DESK_ID]: { "surface-a": [zone(), zone()] } },
			}),
		],
		[
			"duplicate cells",
			snapshot({
				desks: {
					[DESK_ID]: {
						"surface-a": [{ ...zone(), slots: [1, 1] }],
					},
				},
			}),
		],
		[
			"cell above the persisted grid domain",
			snapshot({
				desks: {
					[DESK_ID]: {
						"surface-a": [{ ...zone(), slots: [1, 145] }],
					},
				},
			}),
		],
		[
			"unknown zone field",
			snapshot({
				desks: {
					[DESK_ID]: {
						"surface-a": [{ ...zone(), color: "red" }],
					},
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
				saveOutcome(),
				SCOPE,
				"surface-a",
				"request-a",
			),
		).toMatchObject({
			requestId: "request-a",
			surfaceId: "surface-a",
			zones: [zone()],
		});
		expect(() =>
			decodeVirtualPlaybackZonesSaveOutcome(
				saveOutcome({ surface_id: "surface-b" }),
				SCOPE,
				"surface-a",
				"request-a",
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it.each([
		["show", { show_id: OTHER_ID, desk_id: DESK_ID }],
		["desk", { show_id: SHOW_ID, desk_id: OTHER_ID }],
	])("rejects a foreign %s save outcome", (_label, identity) => {
		expect(() =>
			decodeVirtualPlaybackZonesSaveOutcome(
				saveOutcome(identity),
				SCOPE,
				"surface-a",
				"request-a",
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it("validates an outgoing save before serialization", () => {
		expect(encodeVirtualPlaybackZonesSaveRequest("request-a", [zone()])).toEqual({
			request_id: "request-a",
			zones: [zone()],
		});
		expect(() =>
			encodeVirtualPlaybackZonesSaveRequest("request-a", [
				{ ...zone(), name: "", slots: [1, 2] },
			]),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it("round-trips retained legacy cells above the assignable slot limit", () => {
		const legacy = { ...zone(), slots: [128, 144] };
		expect(
			decodeVirtualPlaybackZonesSnapshot(
				snapshot({ desks: { [DESK_ID]: { "surface-a": [legacy] } } }),
				SCOPE,
			).desks[DESK_ID]["surface-a"],
		).toEqual([legacy]);
		expect(encodeVirtualPlaybackZonesSaveRequest("request-a", [legacy])).toEqual({
			request_id: "request-a",
			zones: [legacy],
		});
	});
});
