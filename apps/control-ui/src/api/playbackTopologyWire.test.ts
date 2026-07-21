import { describe, expect, it } from "vitest";
import type { PlaybackTopologyRequest } from "../features/playbackTopology/contracts";
import {
	decodePlaybackTopologyErrorResponse,
	decodePlaybackTopologyOutcome,
	encodePlaybackTopologyRequest,
} from "./playbackTopologyWire";
import type { CueList, PlaybackDefinition } from "./types";

const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
const REQUEST_ID = "33333333-3333-4333-8333-333333333333";
const CORRELATION_ID = "44444444-4444-4444-8444-444444444444";

function playback(number = 0): PlaybackDefinition {
	return {
		number,
		name: "Front Wash",
		target: { type: "cue_list" as const, cue_list_id: CUE_LIST_ID },
		buttons: ["toggle", "none", "none"],
		button_count: 1,
		fader: "master",
		has_fader: false,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#8b5cf6",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

function request(
	type: "configure_slot" | "clear_mapped_playback" = "configure_slot",
): PlaybackTopologyRequest {
	const common = {
		type,
		page: 4,
		slot: 2,
		expectedPageRevision: 7,
		expectedPageObjectId: "legacy-page-four",
		expectedPlaybackRevision: 3,
		expectedPlaybackObjectId: "legacy-playback-seven",
	};
	return {
		requestId: REQUEST_ID,
		action:
			type === "configure_slot"
				? { ...common, type, playback: playback() }
				: { ...common, type },
	};
}

function cueListRequest(): PlaybackTopologyRequest {
	return {
		requestId: REQUEST_ID,
		action: {
			type: "save_cue_list",
			cueListId: CUE_LIST_ID,
			expectedRevision: 4,
			expectedObjectId: "legacy-main-list",
			body: cueList(),
		},
	};
}

function mapExistingRequest(
	pageIdentity: { revision: number; objectId: string | null } = {
		revision: 7,
		objectId: "legacy-page-four",
	},
): PlaybackTopologyRequest {
	return {
		requestId: REQUEST_ID,
		action: {
			type: "map_existing_playback",
			page: 4,
			slot: 2,
			playbackNumber: 7,
			expectedPageRevision: pageIdentity.revision,
			expectedPageObjectId: pageIdentity.objectId,
			expectedPlaybackRevision: 3,
			expectedPlaybackObjectId: "legacy-playback-seven",
		},
	};
}

function cueList(): CueList {
	return {
		id: CUE_LIST_ID,
		name: "Main",
		priority: 0,
		mode: "sequence",
		looped: false,
		cues: [],
	};
}

function pageObject(slots: Record<string, number> = { 2: 7 }) {
	return {
		state: "present",
		kind: "playback_page",
		object_id: "legacy-page-four",
		object_revision: 8,
		body: { number: 4, name: "Page 4", slots, future_page: true },
	};
}

function playbackObject() {
	return {
		state: "present",
		kind: "playback",
		object_id: "legacy-playback-seven",
		object_revision: 4,
		body: { ...playback(7), future_playback: { keep: true } },
	};
}

function cueListObject() {
	return {
		state: "present",
		kind: "cue_list",
		object_id: "legacy-main-list",
		object_revision: 5,
		body: { ...cueList(), future_cue_list: { retained: true } },
	};
}

function changedOutcome(overrides: Record<string, unknown> = {}) {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		show_revision: 12,
		resolution: {
			kind: "page_slot",
			page: 4,
			slot: 2,
			playback_number: 7,
		},
		status: "changed",
		objects: [playbackObject(), pageObject()],
		event_sequence: 41,
		replayed: false,
		...overrides,
	};
}

describe("Playback topology v2 wire", () => {
	it("encodes one strict configure action with revision metadata", () => {
		expect(encodePlaybackTopologyRequest(request())).toEqual({
			request_id: REQUEST_ID,
			action: {
				type: "configure_slot",
				page: 4,
				slot: 2,
				expected_page_revision: 7,
				expected_page_object_id: "legacy-page-four",
				expected_playback_revision: 3,
				expected_playback_object_id: "legacy-playback-seven",
				playback: {
					...playback(),
					presentation_icon: null,
					presentation_image: null,
				},
			},
		});
	});

	it("encodes one exact existing-Playback map without a Playback body", () => {
		expect(encodePlaybackTopologyRequest(mapExistingRequest())).toEqual({
			request_id: REQUEST_ID,
			action: {
				type: "map_existing_playback",
				page: 4,
				slot: 2,
				playback_number: 7,
				expected_page_revision: 7,
				expected_page_object_id: "legacy-page-four",
				expected_playback_revision: 3,
				expected_playback_object_id: "legacy-playback-seven",
			},
		});
		expect(
			encodePlaybackTopologyRequest(
				mapExistingRequest({ revision: 0, objectId: null }),
			),
		).toMatchObject({
			action: {
				expected_page_revision: 0,
				expected_page_object_id: null,
			},
		});
	});

	it("encodes the exact Cuelist storage identity precondition", () => {
		expect(encodePlaybackTopologyRequest(cueListRequest())).toEqual({
			request_id: REQUEST_ID,
			action: {
				type: "save_cue_list",
				cue_list_id: CUE_LIST_ID,
				expected_revision: 4,
				expected_object_id: "legacy-main-list",
				body: cueList(),
			},
		});
	});

	it("keeps decoded target extensions out of the strict action DTO", () => {
		const configure = request();
		if (configure.action.type !== "configure_slot")
			throw new Error("configure");
		const withExtension = {
			...playback(),
			target: {
				type: "cue_list",
				cue_list_id: CUE_LIST_ID,
				future_target: { retained_in_show: true },
			},
		} as unknown as PlaybackDefinition;
		const encoded = encodePlaybackTopologyRequest({
			...configure,
			action: { ...configure.action, playback: withExtension },
		});

		const encodedPlayback = encoded.action as {
			playback: { target: unknown };
		};
		expect(encodedPlayback.playback.target).toEqual({
			type: "cue_list",
			cue_list_id: CUE_LIST_ID,
		});
	});

	it("decodes an atomic changed outcome with lossless legacy object IDs", () => {
		const outcome = decodePlaybackTopologyOutcome(
			changedOutcome(),
			request(),
			11,
		);

		expect(outcome).toMatchObject({
			status: "changed",
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
			showRevision: 12,
			eventSequence: 41,
			resolution: { playbackNumber: 7 },
			objects: [
				{
					kind: "playback",
					objectId: "legacy-playback-seven",
					body: { number: 7, future_playback: { keep: true } },
				},
				{
					kind: "playback_page",
					objectId: "legacy-page-four",
					body: { number: 4, future_page: true },
				},
			],
		});
	});

	it("accepts only the one authoritative Page for an existing-Playback map", () => {
		const outcome = decodePlaybackTopologyOutcome(
			changedOutcome({ objects: [pageObject()] }),
			mapExistingRequest(),
			11,
		);
		expect(outcome).toMatchObject({
			status: "changed",
			resolution: { playbackNumber: 7 },
			objects: [
				{
					kind: "playback_page",
					objectId: "legacy-page-four",
					objectRevision: 8,
				},
			],
		});

		expect(() =>
			decodePlaybackTopologyOutcome(
				changedOutcome({ objects: [pageObject(), playbackObject()] }),
				mapExistingRequest(),
				11,
			),
		).toThrow("only the authoritative mapped Page");
		expect(() =>
			decodePlaybackTopologyOutcome(
				changedOutcome({
					resolution: {
						kind: "page_slot",
						page: 4,
						slot: 2,
						playback_number: 8,
					},
					objects: [pageObject({ 2: 8 })],
				}),
				mapExistingRequest(),
				11,
			),
		).toThrow("the requested Playback");
	});

	it("validates an existing-Playback no-change and exact identity pairs", () => {
		const { event_sequence: _eventSequence, ...withoutEvent } =
			changedOutcome();
		expect(
			decodePlaybackTopologyOutcome(
				{
					...withoutEvent,
					status: "no_change",
					show_revision: 11,
					objects: [{ ...pageObject(), object_revision: 7 }],
					replayed: true,
				},
				mapExistingRequest(),
				11,
			),
		).toMatchObject({ status: "no_change", replayed: true });

		for (const invalidRequest of [
			mapExistingRequest({ revision: 0, objectId: "legacy-page-four" }),
			mapExistingRequest({ revision: 7, objectId: null }),
			{
				...mapExistingRequest(),
				action: {
					...mapExistingRequest().action,
					expectedPlaybackRevision: 0,
				},
			},
		])
			expect(() => encodePlaybackTopologyRequest(invalidRequest)).toThrow(
				"$.action.expected",
			);
	});

	it("resolves a saved Cuelist by semantic ID while preserving its storage key", () => {
		const outcome = decodePlaybackTopologyOutcome(
			{
				request_id: REQUEST_ID,
				correlation_id: CORRELATION_ID,
				show_revision: 12,
				resolution: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
				status: "changed",
				objects: [cueListObject()],
				event_sequence: 41,
				replayed: false,
			},
			cueListRequest(),
			11,
		);

		expect(outcome.objects[0]).toMatchObject({
			objectId: "legacy-main-list",
			body: { id: CUE_LIST_ID },
		});
	});

	it("rejects a saved Cuelist whose known body differs from the request", () => {
		expect(() =>
			decodePlaybackTopologyOutcome(
				{
					request_id: REQUEST_ID,
					correlation_id: CORRELATION_ID,
					show_revision: 12,
					resolution: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
					status: "changed",
					objects: [
						{
							...cueListObject(),
							body: { ...cueList(), name: "Different" },
						},
					],
					event_sequence: 41,
					replayed: false,
				},
				cueListRequest(),
				11,
			),
		).toThrow(/submitted Cuelist known fields/);
	});

	it("decodes no-change and multi-page clear without inventing an event", () => {
		const clear = request("clear_mapped_playback");
		if (clear.action.type !== "clear_mapped_playback")
			throw new Error("clear request fixture");
		const emptyClear: PlaybackTopologyRequest = {
			...clear,
			action: {
				...clear.action,
				expectedPageRevision: 8,
				expectedPlaybackRevision: 0,
				expectedPlaybackObjectId: null,
			},
		};
		const { event_sequence: _eventSequence, ...changedWithoutEvent } =
			changedOutcome();
		const outcome = decodePlaybackTopologyOutcome(
			{
				...changedWithoutEvent,
				status: "no_change",
				show_revision: 11,
				resolution: {
					kind: "page_slot",
					page: 4,
					slot: 2,
					playback_number: null,
				},
				objects: [pageObject({})],
				replayed: true,
			},
			emptyClear,
			11,
		);
		expect(outcome).toMatchObject({ status: "no_change", replayed: true });
		expect("eventSequence" in outcome).toBe(false);

		const changed = decodePlaybackTopologyOutcome(
			changedOutcome({
				objects: [
					pageObject({}),
					{
						...pageObject({}),
						object_id: "page-two",
						body: { number: 2, name: "Page 2", slots: {} },
					},
					{
						state: "deleted",
						kind: "playback",
						object_id: "legacy-playback-seven",
						object_revision: 4,
					},
				],
			}),
			clear,
			11,
		);
		expect(changed.objects).toHaveLength(3);
	});

	it("strictly rejects undeclared, mismatched, and incomplete authority", () => {
		for (const [value, path] of [
			[{ ...changedOutcome(), bootstrap: {} }, "$.bootstrap"],
			[{ ...changedOutcome(), request_id: "another" }, "$.request_id"],
			[{ ...changedOutcome(), show_revision: 11 }, "$.show_revision"],
			[
				{
					...changedOutcome(),
					resolution: {
						kind: "page_slot",
						page: 4,
						slot: 3,
						playback_number: 7,
					},
				},
				"$.resolution",
			],
			[{ ...changedOutcome(), objects: [playbackObject()] }, "$.objects"],
			[
				{
					...changedOutcome(),
					objects: [{ ...playbackObject(), unexpected: true }, pageObject()],
				},
				"$.objects[0].unexpected",
			],
		] as const)
			expect(() => decodePlaybackTopologyOutcome(value, request(), 11)).toThrow(
				path,
			);

		for (const objects of [
			[{ ...playbackObject(), object_revision: 999 }, pageObject()],
			[playbackObject(), { ...pageObject(), object_id: "another-page" }],
			[
				{
					...playbackObject(),
					body: { ...playbackObject().body, name: "Unexpected body" },
				},
				pageObject(),
			],
		] as const)
			expect(() =>
				decodePlaybackTopologyOutcome(
					changedOutcome({ objects }),
					request(),
					11,
				),
			).toThrow("$.objects");

		expect(() =>
			decodePlaybackTopologyOutcome(
				changedOutcome({
					objects: [playbackObject(), pageObject(), cueListObject()],
				}),
				request(),
				11,
			),
		).toThrow("only the configured Page and Playback");

		expect(() =>
			decodePlaybackTopologyOutcome(
				{
					...changedOutcome(),
					resolution: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
					objects: [cueListObject(), playbackObject()],
				},
				cueListRequest(),
				11,
			),
		).toThrow("only the authoritative requested Cuelist");
		expect(() =>
			decodePlaybackTopologyOutcome(
				{
					...changedOutcome(),
					resolution: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
					objects: [{ ...cueListObject(), object_revision: 999 }],
				},
				cueListRequest(),
				11,
			),
		).toThrow("Cuelist revision 5");

		expect(() =>
			decodePlaybackTopologyOutcome(
				changedOutcome({
					objects: [
						{
							state: "deleted",
							kind: "playback",
							object_id: "legacy-playback-seven",
							object_revision: 4,
						},
					],
				}),
				request("clear_mapped_playback"),
				11,
			),
		).toThrow("only cleared Pages and the mapped Playback");

		expect(() =>
			decodePlaybackTopologyOutcome(
				changedOutcome({
					objects: [
						pageObject({}),
						{
							state: "deleted",
							kind: "playback",
							object_id: "another-playback",
							object_revision: 4,
						},
					],
				}),
				request("clear_mapped_playback"),
				11,
			),
		).toThrow("exact deleted mapped Playback");
	});

	it("requires exact unchanged object revisions for a no-change outcome", () => {
		const { event_sequence: _eventSequence, ...withoutEvent } =
			changedOutcome();
		const value = {
			...withoutEvent,
			status: "no_change",
			show_revision: 11,
			objects: [
				{ ...playbackObject(), object_revision: 3 },
				{ ...pageObject(), object_revision: 7 },
			],
		};
		expect(decodePlaybackTopologyOutcome(value, request(), 11)).toMatchObject({
			status: "no_change",
		});

		expect(() =>
			decodePlaybackTopologyOutcome(
				{
					...value,
					objects: [playbackObject(), { ...pageObject(), object_revision: 7 }],
				},
				request(),
				11,
			),
		).toThrow("Playback revision 3");

		const cueNoChange = {
			...value,
			resolution: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			objects: [{ ...cueListObject(), object_revision: 4 }],
		};
		expect(
			decodePlaybackTopologyOutcome(cueNoChange, cueListRequest(), 11),
		).toMatchObject({ status: "no_change" });
	});

	it("decodes strict conflict metadata for exact repair", () => {
		expect(
			decodePlaybackTopologyErrorResponse({
				kind: "conflict",
				error: "stale Playback Page revision",
				current_revision: 15,
				current_related_revision: 9,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "stale Playback Page revision",
			currentRevision: 15,
			currentRelatedRevision: 9,
			retryable: false,
		});
	});
});
