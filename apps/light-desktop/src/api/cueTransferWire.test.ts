import { describe, expect, it } from "vitest";
import type { CueTransferActionRequest } from "../features/cueTransfer/contracts";
import {
	decodeCueTransferActionOutcome,
	decodeCueTransferErrorResponse,
	encodeCueTransferActionRequest,
} from "./cueTransferWire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CHOICE_ID = "22222222-2222-4222-8222-222222222222";
const REQUEST_ID = "33333333-3333-4333-8333-333333333333";
const SOURCE_CUE_ID = "44444444-4444-4444-8444-444444444444";
const DESTINATION_CUE_ID = "55555555-5555-4555-8555-555555555555";
const CUE_LIST_ID = "66666666-6666-4666-8666-666666666666";
const request: CueTransferActionRequest = {
	requestId: REQUEST_ID,
	choiceId: CHOICE_ID,
	mode: "status",
	expectedCommandLineRevision: 4,
};

function wireOutcome() {
	return {
		status: "changed",
		request_id: REQUEST_ID,
		choice_id: CHOICE_ID,
		correlation_id: "77777777-7777-4777-8777-777777777777",
		replayed: false,
		show_id: SHOW_ID,
		summary: {
			operation: "copy",
			mode: "status",
			source_cue_id: SOURCE_CUE_ID,
			source_cue_number: 1,
			destination_cue_id: DESTINATION_CUE_ID,
			destination_cue_number: 2,
		},
		show_revision: 8,
		projections: [
			{
				cue_list_id: CUE_LIST_ID,
				object_id: "legacy-destination",
				object_revision: 3,
				body: {
					id: CUE_LIST_ID,
					name: "Destination",
					priority: 0,
					mode: "sequence",
					looped: false,
					future: { retained: true },
					cues: [
						{
							id: DESTINATION_CUE_ID,
							number: 2,
							name: "Transferred",
							fade_millis: 0,
							delay_millis: 0,
							trigger: { type: "manual" },
							cue_only: false,
							changes: [],
							group_changes: [],
							phasers: [],
						},
					],
				},
			},
		],
		show_event_sequence: 12,
		command_line: {
			text: "FIXTURE",
			target: "FIXTURE",
			pristine: true,
			revision: 5,
			pending_choice: null,
		},
		interaction_event_sequence: 21,
		persistence_warning: null,
	};
}

describe("Cue transfer wire codec", () => {
	it("encodes only the typed retained-choice action", () => {
		expect(encodeCueTransferActionRequest(request)).toEqual({
			request_id: REQUEST_ID,
			choice_id: CHOICE_ID,
			mode: "status",
			expected_command_line_revision: 4,
		});
	});

	it("strictly decodes the authoritative lossless projections", () => {
		const decoded = decodeCueTransferActionOutcome(
			wireOutcome(),
			request,
			SHOW_ID,
			7,
		);
		expect(decoded).toMatchObject({
			requestId: REQUEST_ID,
			choiceId: CHOICE_ID,
			showId: SHOW_ID,
			showRevision: 8,
			showEventSequence: 12,
			interactionEventSequence: 21,
		});
		expect(decoded.projections[0].body).toMatchObject({
			future: { retained: true },
		});
	});

	it("rejects foreign scope, choice, and malformed projection authority", () => {
		const foreignShow = wireOutcome();
		foreignShow.show_id = "88888888-8888-4888-8888-888888888888";
		expect(() =>
			decodeCueTransferActionOutcome(foreignShow, request, SHOW_ID, 7),
		).toThrow(/show_id/);

		const foreignChoice = wireOutcome();
		foreignChoice.choice_id = "99999999-9999-4999-8999-999999999999";
		expect(() =>
			decodeCueTransferActionOutcome(foreignChoice, request, SHOW_ID, 7),
		).toThrow(/choice_id/);

		const duplicate = wireOutcome();
		duplicate.projections.push(structuredClone(duplicate.projections[0]));
		expect(() =>
			decodeCueTransferActionOutcome(duplicate, request, SHOW_ID, 7),
		).toThrow(/unique Cuelist object IDs/);
	});

	it("rejects an outcome that retains the pending choice", () => {
		const retained = wireOutcome();
		(retained.command_line as Record<string, unknown>).pending_choice = {
			type: "cue_move_copy",
			choice_id: CHOICE_ID,
			show_id: SHOW_ID,
			show_revision: 7,
			operation: "copy",
			command: "COPY SET 1 CUE 1 AT SET 2 CUE 2",
			options: [],
			cancel_label: "Cancel",
		};
		expect(() =>
			decodeCueTransferActionOutcome(retained, request, SHOW_ID, 7),
		).toThrow(/pending_choice/);
	});

	it("strictly decodes conflict revisions", () => {
		expect(
			decodeCueTransferErrorResponse({
				kind: "conflict",
				error: "stale choice",
				current_revision: 5,
				current_related_revision: 8,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "stale choice",
			currentRevision: 5,
			currentRelatedRevision: 8,
			retryable: false,
		});
		expect(() =>
			decodeCueTransferErrorResponse({
				kind: "conflict",
				error: "stale",
				retryable: false,
				details: {},
			}),
		).toThrow(/details/);
	});
});
