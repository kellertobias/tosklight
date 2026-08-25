import { describe, expect, it } from "vitest";
import {
	decodeProgrammerPreloadPlaybackQueueEventMessage,
	decodeProgrammerPreloadPlaybackQueueSnapshot,
} from "./programmerPreloadPlaybackQueueWire";

const SESSION_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OTHER_USER = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

function actions() {
	return [
		{ playback_number: 4, page: 3, action: "go", surface: "physical" },
		{ playback_number: 4, action: "go", surface: "osc" },
		{
			playback_number: 2,
			page: null,
			action: "temporary_off",
			surface: "matter",
		},
	];
}

function projection() {
	return { revision: 3, actions: actions() };
}

function event() {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-20T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: "programming-preload-playback-queue",
			},
			related_objects: [],
			source: { kind: "action", source: "osc" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_preload_playback_queue_changed",
				change: { projection: projection() },
			},
		},
	};
}

describe("Preload playback queue wire decoding", () => {
	it("preserves ordered duplicate actions from a strict snapshot", () => {
		expect(
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{ cursor: { sequence: 11 }, projection: projection() }),
		).toEqual({
			cursor: 11,
			projection: {
				revision: 3,
				actions: [
					{ playbackNumber: 4, page: 3, action: "go", surface: "physical" },
					{ playbackNumber: 4, page: null, action: "go", surface: "osc" },
					{
						playbackNumber: 2,
						page: null,
						action: "temporary_off",
						surface: "matter",
					},
				],
			},
		});
	});

	it("decodes the replaceable event", () => {
		expect(
			decodeProgrammerPreloadPlaybackQueueEventMessage(event()),
		).toMatchObject({
			type: "event",
			sequence: 12,
			correlationId: CORRELATION_ID,
			projection: { revision: 3 },
		});
	});

	it("rejects undeclared queue content", () => {
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueSnapshot({
				cursor: { sequence: 1 },
				projection: { ...projection(), user_id: OTHER_USER },
			}),
		).toThrow(/user_id/);
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{
					cursor: { sequence: 1 },
					projection: { ...projection(), values: [] },
				}),
		).toThrow(/values/);
	});

	it.each([
		["action", "pause"],
		["surface", "network"],
		["playback_number", 65_536],
		["page", 256],
	] as const)("rejects invalid %s values", (field, invalid) => {
		const malformed = actions();
		malformed[0] = { ...malformed[0], [field]: invalid };
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{
					cursor: { sequence: 1 },
					projection: { ...projection(), actions: malformed },
				}),
		).toThrow();
	});

	it("rejects the wrong object, delivery, and missing correlation", () => {
		const wrongObject = event();
		wrongObject.event.object.id = "programming-preload-values";
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueEventMessage(wrongObject),
		).toThrow(/playback-queue/);
		const wrongDelivery = event();
		wrongDelivery.event.delivery = "lossless";
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueEventMessage(wrongDelivery),
		).toThrow(/replaceable/);
		const missingCorrelation = event();
		Reflect.deleteProperty(missingCorrelation.event, "correlation_id");
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueEventMessage(
				missingCorrelation),
		).toThrow(/correlation_id/);
	});
});
