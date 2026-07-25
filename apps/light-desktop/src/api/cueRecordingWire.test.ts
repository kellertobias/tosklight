import { describe, expect, it } from "vitest";
import type { CueRecordingRequest } from "../features/cueRecording/contracts";
import {
	decodeCueRecordErrorResponse,
	decodeCueRecordingOutcome,
	encodeCueRecordingRequest,
} from "./cueRecordingWire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
const CUE_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";

function request(
	target: CueRecordingRequest["target"] = {
		kind: "page_slot",
		page: 4,
		slot: 2,
	},
): CueRecordingRequest {
	return {
		requestId: REQUEST_ID,
		target,
		operation: "overwrite",
		cueNumber: 1,
		timing: { fadeMillis: 1200, delayMillis: 250 },
		cueOnly: false,
		name: "Opening",
		capturePolicy: "current_capture",
		activationPolicy: "hold",
	};
}

function cueListBody(name = "Opening") {
	return {
		id: CUE_LIST_ID,
		name: "Main",
		priority: 0,
		mode: "sequence",
		looped: false,
		cues: [
			{
				id: CUE_ID,
				number: 1,
				name,
				fade_millis: 1200,
				delay_millis: 250,
				trigger: { type: "manual" },
				cue_only: false,
				changes: [
					{
						fixture_id: "fixture-1",
						attribute: "Intensity",
						value: { kind: "normalized", value: 0.75 },
					},
				],
				group_changes: [],
				phasers: [],
			},
		],
	};
}

function wireCueList(revision = 2, body = cueListBody()) {
	return { id: CUE_LIST_ID, revision, body };
}

function wirePlayback(cueListId = CUE_LIST_ID) {
	return {
		id: "7",
		revision: 2,
		body: {
			number: 7,
			name: "Main",
			target: { type: "cue_list", cue_list_id: cueListId },
			buttons: ["go_minus", "go", "flash"],
		},
	};
}

function wirePage() {
	return {
		id: "4",
		revision: 2,
		body: { number: 4, name: "Page 4", slots: { 2: 7 } },
	};
}

function changedOutcome(overrides: Record<string, unknown> = {}) {
	return {
		status: "changed",
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		replayed: false,
		captured_source: "normal",
		show_revision: 8,
		recorded_cue: { id: CUE_ID, number: 1, deleted: false },
		projections: {
			cue_list: wireCueList(),
			playback: wirePlayback(),
			page: wirePage(),
		},
		show_event_sequence: 12,
		runtime: null,
		...overrides,
	};
}

function noChangeOutcome(overrides: Record<string, unknown> = {}) {
	const { show_event_sequence: _sequence, runtime: _runtime, ...base } =
		changedOutcome({
			status: "no_change",
			show_revision: 7,
			projections: {
				cue_list: wireCueList(1),
				playback: wirePlayback(),
				page: wirePage(),
			},
		});
	return { ...base, ...overrides };
}

function runtime(showId = SHOW_ID) {
	return {
		projection: {
			scope: { show_id: showId, show_revision: 8 },
			requested: { kind: "playback", playback_number: 7 },
			playback_number: 7,
			target: "cue_list",
			cue_list_id: CUE_LIST_ID,
			runtime: null,
		},
		event_sequence: 21,
	};
}

describe("Cue recording v2 wire", () => {
	it("encodes every target shape and only action-time metadata", () => {
		const targets: Array<[
			CueRecordingRequest["target"],
			Record<string, unknown>,
		]> = [
			[
				{ kind: "pool", playbackNumber: 7 },
				{ kind: "pool", playback_number: 7 },
			],
			[{ kind: "selected_playback" }, { kind: "selected_playback" }],
			[
				{ kind: "page_slot", page: 4, slot: 2 },
				{ kind: "page_slot", page: 4, slot: 2 },
			],
			[
				{ kind: "cue_list", cueListId: CUE_LIST_ID },
				{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
			],
		];

		for (const [target, encodedTarget] of targets) {
			const encoded = encodeCueRecordingRequest(request(target));
			expect(encoded.target).toEqual(encodedTarget);
			expect(encoded).toMatchObject({
				request_id: REQUEST_ID,
				operation: "overwrite",
				cue_number: 1,
				timing: { fade_millis: 1200, delay_millis: 250 },
				cue_only: false,
				name: "Opening",
				capture_policy: "current_capture",
				activation_policy: "hold",
			});
			expect(JSON.stringify(encoded)).not.toMatch(
				/selection|programmer_values|highlight|connectivity|priority|mode_state/i,
			);
		}
	});

	it("omits absent timing fields and encodes nullable Cue metadata", () => {
		const encoded = encodeCueRecordingRequest({
			...request({ kind: "selected_playback" }),
			cueNumber: undefined,
			timing: {},
			name: undefined,
			capturePolicy: "pending_or_active_preload",
			activationPolicy: "go_to_if_normal",
		});

		expect(encoded).toMatchObject({
			cue_number: null,
			timing: {},
			name: null,
			capture_policy: "pending_or_active_preload",
			activation_policy: "go_to_if_normal",
		});
	});

	it("strictly decodes changed and replayed no-change outcomes", () => {
		const changed = decodeCueRecordingOutcome(
			changedOutcome({ runtime: runtime() }),
			request(),
			SHOW_ID,
			7,
		);
		expect(changed).toMatchObject({
			status: "changed",
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
			showRevision: 8,
			showEventSequence: 12,
			capturedSource: "normal",
			recordedCue: { id: CUE_ID, number: 1, deleted: false },
			projections: {
				cueList: { kind: "cue_list", id: CUE_LIST_ID, revision: 2 },
				playback: { kind: "playback", id: "7", revision: 2 },
				page: { kind: "playback_page", id: "4", revision: 2 },
			},
			runtime: { eventSequence: 21 },
		});

		const noChange = decodeCueRecordingOutcome(
			noChangeOutcome({ replayed: true, captured_source: "active_preload" }),
			request(),
			SHOW_ID,
			7,
		);
		expect(noChange).toMatchObject({
			status: "no_change",
			replayed: true,
			capturedSource: "active_preload",
			showRevision: 7,
		});
		expect("showEventSequence" in noChange).toBe(false);
		expect("runtime" in noChange).toBe(false);
	});

	it("decodes an authoritative empty Subtract deletion without inventing runtime", () => {
		const subtract: CueRecordingRequest = {
			...request({ kind: "cue_list", cueListId: CUE_LIST_ID }),
			operation: "subtract",
		};
		const outcome = changedOutcome({
			recorded_cue: { id: CUE_ID, number: 1, deleted: true },
			projections: {
				cue_list: wireCueList(2, { ...cueListBody(), cues: [] }),
				playback: null,
				page: null,
			},
		});

		const decoded = decodeCueRecordingOutcome(outcome, subtract, SHOW_ID, 7);

		expect(decoded.recordedCue).toEqual({
			id: CUE_ID,
			number: 1,
			deleted: true,
		});
		expect(decoded.status === "changed" && decoded.runtime).toBeNull();
	});

	it.each([
		[
			"request identity",
			changedOutcome({ request_id: "another-request" }),
			"$.request_id",
		],
		[
			"changed Show revision",
			changedOutcome({ show_revision: 7 }),
			"$.show_revision",
		],
		[
			"recorded Cue number",
			changedOutcome({
				recorded_cue: { id: CUE_ID, number: 2, deleted: false },
			}),
			"$.recorded_cue.number",
		],
		[
			"recorded Cue identity",
			changedOutcome({
				recorded_cue: {
					id: "66666666-6666-4666-8666-666666666666",
					number: 1,
					deleted: false,
				},
			}),
			"$.recorded_cue",
		],
		[
			"Playback target",
			changedOutcome({
				projections: {
					cue_list: wireCueList(),
					playback: wirePlayback(
						"88888888-8888-4888-8888-888888888888",
					),
					page: null,
				},
			}),
			"$.projections.playback.body.target.cue_list_id",
		],
		[
			"runtime Show scope",
			changedOutcome({
				runtime: runtime("99999999-9999-4999-8999-999999999999"),
			}),
			"$.runtime.projection.scope.show_id",
		],
		[
			"runtime Show revision",
			changedOutcome({
				runtime: {
					...runtime(),
					projection: {
						...runtime().projection,
						scope: { show_id: SHOW_ID, show_revision: 7 },
					},
				},
			}),
			"$.runtime.projection.scope.show_revision",
		],
		[
			"runtime Playback identity",
			changedOutcome({
				runtime: {
					...runtime(),
					projection: { ...runtime().projection, playback_number: 8 },
				},
			}),
			"$.runtime.projection.playback_number",
		],
		[
			"runtime Cuelist identity",
			changedOutcome({
				runtime: {
					...runtime(),
					projection: {
						...runtime().projection,
						cue_list_id: "88888888-8888-4888-8888-888888888888",
					},
				},
			}),
			"$.runtime.projection.cue_list_id",
		],
		[
			"undeclared user scope",
			changedOutcome({ user_id: "foreign-user" }),
			"$.user_id",
		],
	] as const)("rejects mismatched or foreign %s data", (_label, value, path) => {
		expect(() =>
			decodeCueRecordingOutcome(value, request(), SHOW_ID, 7),
		).toThrow(path);
	});

	it("requires a direct Cuelist response to match its target without topology", () => {
		const direct = request({ kind: "cue_list", cueListId: CUE_LIST_ID });
		expect(() =>
			decodeCueRecordingOutcome(
				changedOutcome({
					projections: {
						cue_list: {
							...wireCueList(),
							id: "77777777-7777-4777-8777-777777777777",
							body: {
								...cueListBody(),
								id: "77777777-7777-4777-8777-777777777777",
							},
						},
						playback: null,
						page: null,
					},
				}),
				direct,
				SHOW_ID,
				7,
			),
		).toThrow("$.projections.cue_list.id");
	});

	it.each([
		[
			"direct Cuelist extras",
			request({ kind: "cue_list", cueListId: CUE_LIST_ID }),
			changedOutcome(),
			"$.projections",
		],
		[
			"Pool Playback identity",
			request({ kind: "pool", playbackNumber: 8 }),
			changedOutcome({
				projections: {
					cue_list: wireCueList(),
					playback: wirePlayback(),
					page: null,
				},
			}),
			"$.projections.playback.body.number",
		],
		[
			"selected Playback page",
			request({ kind: "selected_playback" }),
			changedOutcome(),
			"$.projections.page",
		],
		[
			"missing PageSlot page",
			request(),
			changedOutcome({
				projections: {
					cue_list: wireCueList(),
					playback: wirePlayback(),
					page: null,
				},
			}),
			"$.projections.page",
		],
		[
			"unrelated PageSlot assignment",
			request(),
			changedOutcome({
				projections: {
					cue_list: wireCueList(),
					playback: wirePlayback(),
					page: { ...wirePage(), body: { ...wirePage().body, slots: { 2: 9 } } },
				},
			}),
			"$.projections.page.body.slots.2",
		],
	] as const)("rejects %s topology", (_label, action, value, path) => {
		expect(() =>
			decodeCueRecordingOutcome(value, action, SHOW_ID, 7),
		).toThrow(path);
	});

	it("rejects event fields on no-change and missing changed event identity", () => {
		expect(() =>
			decodeCueRecordingOutcome(
				{ ...noChangeOutcome(), show_event_sequence: 12 },
				request(),
				SHOW_ID,
				7,
			),
		).toThrow("$.show_event_sequence");
		const missingSequence: Partial<ReturnType<typeof changedOutcome>> =
			changedOutcome();
		delete missingSequence.show_event_sequence;
		expect(() =>
			decodeCueRecordingOutcome(missingSequence, request(), SHOW_ID, 7),
		).toThrow("$.show_event_sequence");
	});

	it("rejects requests outside backend bounds", () => {
		for (const malformed of [
			{ ...request(), requestId: "\u0000" },
			{ ...request(), target: { kind: "pool" as const, playbackNumber: 0 } },
			{
				...request(),
				target: { kind: "page_slot" as const, page: 128, slot: 1 },
			},
			{
				...request(),
				target: { kind: "cue_list" as const, cueListId: "foreign" },
			},
			{ ...request(), cueNumber: 0 },
			{ ...request(), timing: { fadeMillis: -1 } },
			{ ...request(), name: "x".repeat(257) },
		])
			expect(() => encodeCueRecordingRequest(malformed)).toThrow();
	});

	it("strictly decodes structured action errors", () => {
		expect(
			decodeCueRecordErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				current_revision: 9,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "revision conflict",
			currentRevision: 9,
			retryable: false,
		});
		expect(() =>
			decodeCueRecordErrorResponse({
				kind: "forbidden",
				error: "foreign user",
				retryable: false,
				user_id: "foreign-user",
			}),
		).toThrow("$.user_id");
	});
});
