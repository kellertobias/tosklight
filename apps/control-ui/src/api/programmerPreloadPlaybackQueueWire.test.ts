import { describe, expect, it } from "vitest";
import {
	decodeProgrammerPreloadPlaybackQueueEventMessage,
	decodeProgrammerPreloadPlaybackQueueSnapshot,
} from "./programmerPreloadPlaybackQueueWire";

const USER_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OTHER_USER = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

function actions() {
	return [
		{ playback_number: 4, action: "go", surface: "physical" },
		{ playback_number: 4, action: "go", surface: "osc" },
		{ playback_number: 2, action: "temporary_off", surface: "matter" },
	];
}

function projection(userId = USER_ID) {
	return { user_id: userId, revision: 3, actions: actions() };
}

function event(userId = USER_ID) {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-20T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-preload-playback-queue:${userId}`,
			},
			related_objects: [],
			source: { kind: "action", source: "osc" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_preload_playback_queue_changed",
				change: { projection: projection(userId) },
			},
		},
	};
}

describe("Preload playback queue wire decoding", () => {
	it("preserves ordered duplicate actions from a strict snapshot", () => {
		expect(
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{ cursor: { sequence: 11 }, projection: projection() },
				USER_ID,
			),
		).toEqual({
			cursor: 11,
			projection: {
				userId: USER_ID,
				revision: 3,
				actions: [
					{ playbackNumber: 4, action: "go", surface: "physical" },
					{ playbackNumber: 4, action: "go", surface: "osc" },
					{
						playbackNumber: 2,
						action: "temporary_off",
						surface: "matter",
					},
				],
			},
		});
	});

	it("decodes only the exact-user replaceable event", () => {
		expect(
			decodeProgrammerPreloadPlaybackQueueEventMessage(event(), USER_ID),
		).toMatchObject({
			type: "event",
			sequence: 12,
			correlationId: CORRELATION_ID,
			projection: { userId: USER_ID, revision: 3 },
		});
	});

	it("rejects foreign users and undeclared queue content", () => {
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{ cursor: { sequence: 1 }, projection: projection(OTHER_USER) },
				USER_ID,
			),
		).toThrow(/requested user/);
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{
					cursor: { sequence: 1 },
					projection: { ...projection(), values: [] },
				},
				USER_ID,
			),
		).toThrow(/values/);
	});

	it.each([
		["action", "pause"],
		["surface", "network"],
		["playback_number", 65_536],
	] as const)("rejects invalid %s values", (field, invalid) => {
		const malformed = actions();
		malformed[0] = { ...malformed[0], [field]: invalid };
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueSnapshot(
				{
					cursor: { sequence: 1 },
					projection: { ...projection(), actions: malformed },
				},
				USER_ID,
			),
		).toThrow();
	});

	it("rejects the wrong object, delivery, and missing correlation", () => {
		const wrongObject = event();
		wrongObject.event.object.id = `programming-preload-values:${USER_ID}`;
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueEventMessage(wrongObject, USER_ID),
		).toThrow(/playback-queue/);
		const wrongDelivery = event();
		wrongDelivery.event.delivery = "lossless";
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueEventMessage(wrongDelivery, USER_ID),
		).toThrow(/replaceable/);
		const missingCorrelation = event();
		Reflect.deleteProperty(missingCorrelation.event, "correlation_id");
		expect(() =>
			decodeProgrammerPreloadPlaybackQueueEventMessage(
				missingCorrelation,
				USER_ID,
			),
		).toThrow(/correlation_id/);
	});
});
