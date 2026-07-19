import { describe, expect, it } from "vitest";
import {
	decodeProgrammerCaptureModeEventMessage,
	decodeProgrammerCaptureModeSnapshot,
} from "./programmerCaptureModeWire";
import { WireValidationError } from "./wireValidation";

const USER_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OTHER_USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

function projection(overrides: Record<string, unknown> = {}) {
	return {
		user_id: USER_ID,
		revision: 4,
		blind: true,
		preview: false,
		preload_capture_programmer: true,
		...overrides,
	};
}

function captureEvent(overrides: Record<string, unknown> = {}) {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-20T10:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-capture-mode:${USER_ID}`,
			},
			related_objects: [],
			source: { kind: "action", source: "osc" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_capture_mode_changed",
				change: { projection: projection() },
			},
			...overrides,
		},
	};
}

describe("Programmer capture-mode wire", () => {
	it("decodes an exact-user snapshot", () => {
		expect(
			decodeProgrammerCaptureModeSnapshot(
				{ cursor: { sequence: 18 }, projection: projection() },
				USER_ID,
			),
		).toEqual({
			cursor: 18,
			projection: {
				userId: USER_ID,
				revision: 4,
				blind: true,
				preview: false,
				preloadCaptureProgrammer: true,
			},
		});
	});

	it("rejects foreign users and malformed switches", () => {
		expect(() =>
			decodeProgrammerCaptureModeSnapshot(
				{
					cursor: { sequence: 18 },
					projection: projection({ user_id: OTHER_USER_ID }),
				},
				USER_ID,
			),
		).toThrow(/requested user/);
		expect(() =>
			decodeProgrammerCaptureModeSnapshot(
				{
					cursor: { sequence: 18 },
					projection: projection({ blind: "yes" }),
				},
				USER_ID,
			),
		).toThrow(WireValidationError);
	});

	it("decodes the exact replaceable user event and control messages", () => {
		expect(
			decodeProgrammerCaptureModeEventMessage(captureEvent(), USER_ID),
		).toEqual({
			type: "event",
			sequence: 19,
			correlationId: CORRELATION_ID,
			projection: {
				userId: USER_ID,
				revision: 4,
				blind: true,
				preview: false,
				preloadCaptureProgrammer: true,
			},
		});
		expect(
			decodeProgrammerCaptureModeEventMessage(
				{
					type: "gap",
					gap: {
						after_sequence: 19,
						oldest_available: 25,
						latest_sequence: 30,
					},
				},
				USER_ID,
			),
		).toEqual({
			type: "gap",
			afterSequence: 19,
			oldestAvailable: 25,
			latestSequence: 30,
		});
	});

	it.each([
		["desk-scoped event", { desk_id: OTHER_USER_ID }],
		["lossless delivery", { delivery: "lossless" }],
		["runtime source", { source: { kind: "runtime" } }],
		["missing correlation", { correlation_id: undefined }],
	])("rejects a %s", (_label, replacement) => {
		const candidate = captureEvent(replacement as Record<string, unknown>);
		if ("correlation_id" in replacement)
			delete (candidate.event as Record<string, unknown>).correlation_id;
		expect(() =>
			decodeProgrammerCaptureModeEventMessage(candidate, USER_ID),
		).toThrow(WireValidationError);
	});

	it("rejects another Programmer object and a foreign projection", () => {
		const wrongObject = captureEvent({
			object: {
				capability: "programmer",
				id: `programming-values:${USER_ID}`,
			},
		});
		expect(() =>
			decodeProgrammerCaptureModeEventMessage(wrongObject, USER_ID),
		).toThrow(/programming-capture-mode/);

		const foreignProjection = captureEvent();
		foreignProjection.event.payload.change.projection = projection({
			user_id: OTHER_USER_ID,
		});
		expect(() =>
			decodeProgrammerCaptureModeEventMessage(foreignProjection, USER_ID),
		).toThrow(/requested user/);
	});
});
