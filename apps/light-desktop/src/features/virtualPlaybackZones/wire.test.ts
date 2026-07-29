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

function surface(overrides: Record<string, unknown> = {}) {
	return {
		revision: 4,
		page_mode: { type: "follow_main" },
		zones: [zone()],
		...overrides,
	};
}

function snapshot(overrides: Record<string, unknown> = {}) {
	return {
		show_id: SHOW_ID,
		desks: { [DESK_ID]: { "surface-a": surface() } },
		...overrides,
	};
}

function saveOutcome(overrides: Record<string, unknown> = {}) {
	return {
		request_id: "request-a",
		show_id: SHOW_ID,
		desk_id: DESK_ID,
		surface_id: "surface-a",
		surface: surface(),
		replayed: false,
		changed: true,
		...overrides,
	};
}

describe("Virtual Playback exclusion-zone wire", () => {
	it("decodes an exact authority-scoped snapshot", () => {
		expect(
			decodeVirtualPlaybackZonesSnapshot(snapshot({ future_field: true }), SCOPE),
		).toEqual({
			showId: SHOW_ID,
			desks: {
				[DESK_ID]: {
					"surface-a": {
						revision: 4,
						pageMode: { type: "follow_main" },
						zones: [zone()],
					},
				},
			},
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
		[
			"untrimmed surface",
			snapshot({ desks: { [DESK_ID]: { " surface-a": surface() } } }),
		],
		[
			"duplicate zone ids",
			snapshot({
				desks: {
					[DESK_ID]: {
						"surface-a": surface({ zones: [zone(), zone()] }),
					},
				},
			}),
		],
		[
			"duplicate cells",
			snapshot({
				desks: {
					[DESK_ID]: {
						"surface-a": surface({
							zones: [{ ...zone(), slots: [1, 1] }],
						}),
					},
				},
			}),
		],
		[
			"cell above the persisted grid domain",
			snapshot({
				desks: {
					[DESK_ID]: {
						"surface-a": surface({
							zones: [{ ...zone(), slots: [1, 8_999] }],
						}),
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
			surface: {
				revision: 4,
				pageMode: { type: "follow_main" },
				zones: [zone()],
			},
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
		expect(
			encodeVirtualPlaybackZonesSaveRequest(
				"request-a",
				4,
				{ type: "pinned", page: 7 },
				[zone()],
			),
		).toEqual({
			request_id: "request-a",
			expected_revision: 4,
			page_mode: { type: "pinned", page: 7 },
			zones: [zone()],
		});
		expect(() =>
			encodeVirtualPlaybackZonesSaveRequest(
				"request-a",
				4,
				{ type: "follow_main" },
				[{ ...zone(), name: "", slots: [1, 2] }],
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it("round-trips cells through the dedicated Virtual Playback domain", () => {
		const largest = { ...zone(), slots: [128, 8_998], future_field: true };
		expect(
			decodeVirtualPlaybackZonesSnapshot(
				snapshot({
					desks: {
						[DESK_ID]: {
							"surface-a": surface({ zones: [largest], future_field: true }),
						},
					},
				}),
				SCOPE,
			).desks[DESK_ID]["surface-a"].zones,
		).toEqual([{ ...zone(), slots: [128, 8_998] }]);
		expect(
			encodeVirtualPlaybackZonesSaveRequest(
				"request-a",
				4,
				{ type: "follow_main" },
				[{ ...zone(), slots: [128, 8_998] }],
			),
		).toEqual({
			request_id: "request-a",
			expected_revision: 4,
			page_mode: { type: "follow_main" },
			zones: [{ ...zone(), slots: [128, 8_998] }],
		});
	});

	it("rejects malformed surface revision and pinned page authority", () => {
		expect(() =>
			decodeVirtualPlaybackZonesSnapshot(
				snapshot({
					desks: {
						[DESK_ID]: {
							"surface-a": surface({ revision: -1 }),
						},
					},
				}),
				SCOPE,
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
		expect(() =>
			encodeVirtualPlaybackZonesSaveRequest(
				"request-a",
				4,
				{ type: "pinned", page: 128 },
				[zone()],
			),
		).toThrow(VirtualPlaybackZonesProtocolError);
	});
});
