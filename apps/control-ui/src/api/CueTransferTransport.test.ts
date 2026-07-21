import { describe, expect, it, vi } from "vitest";
import type {
	CueTransferActionRequest,
	CueTransferTransportError,
} from "../features/cueTransfer/contracts";
import { HttpCueTransferTransport } from "./CueTransferTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const CHOICE_ID = "22222222-2222-4222-8222-222222222222";
const REQUEST_ID = "33333333-3333-4333-8333-333333333333";
const CUE_LIST_ID = "44444444-4444-4444-8444-444444444444";
const SOURCE_CUE_ID = "55555555-5555-4555-8555-555555555555";
const DESTINATION_CUE_ID = "66666666-6666-4666-8666-666666666666";
const request: CueTransferActionRequest = {
	requestId: REQUEST_ID,
	choiceId: CHOICE_ID,
	mode: "plain",
	expectedCommandLineRevision: 4,
};

describe("HttpCueTransferTransport", () => {
	it("posts one authenticated revisioned action and verifies its ETag", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(JSON.stringify(outcome()), {
				status: 200,
				headers: { etag: '"8"' },
			}),
		);
		const transport = new HttpCueTransferTransport({
			baseUrl: "http://desk.test/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch,
		});

		await expect(transport.apply(SHOW_ID, 7, request)).resolves.toMatchObject({
			requestId: REQUEST_ID,
			choiceId: CHOICE_ID,
			showId: SHOW_ID,
		});
		expect(fetch).toHaveBeenCalledOnce();
		const [url, init] = fetch.mock.calls[0];
		expect(url).toBe(`http://desk.test/api/v2/shows/${SHOW_ID}/cues/transfer`);
		expect(init?.method).toBe("POST");
		const headers = new Headers(init?.headers);
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
		expect(headers.get("if-match")).toBe('"7"');
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: REQUEST_ID,
			choice_id: CHOICE_ID,
			mode: "plain",
			expected_command_line_revision: 4,
		});
	});

	it("maps a strict typed conflict response", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(
			new Response(
				JSON.stringify({
					kind: "conflict",
					error: "choice expired",
					current_revision: 5,
					current_related_revision: 8,
					retryable: false,
				}),
				{ status: 409, headers: { etag: '"5"' } },
			),
		);
		const transport = new HttpCueTransferTransport({
			baseUrl: "http://desk.test",
			sessionToken: "token",
			fetch,
		});

		await expect(transport.apply(SHOW_ID, 7, request)).rejects.toEqual(
			expect.objectContaining<Partial<CueTransferTransportError>>({
				status: 409,
				currentRevision: 5,
				currentRelatedRevision: 8,
				retryable: false,
			}),
		);
	});

	it("repairs through only the exact Cuelist and command-line reads", async () => {
		const fetch = vi.fn<typeof globalThis.fetch>(async (input) => {
			const url = String(input);
			if (url.endsWith(`/objects/cue_list`))
				return new Response(JSON.stringify([cueListObject()]), {
					headers: { etag: '"9"' },
				});
			if (url.endsWith(`/desks/${DESK_ID}/command-line`))
				return new Response(JSON.stringify(commandLineWire()), {
					headers: { etag: '"6"' },
				});
			throw new Error(`Unexpected broad request: ${url}`);
		});
		const transport = new HttpCueTransferTransport({
			baseUrl: "http://desk.test/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch,
		});

		await expect(transport.loadCueLists(SHOW_ID)).resolves.toMatchObject({
			showRevision: 9,
			objects: [{ id: "destination", revision: 2 }],
		});
		await expect(transport.loadCommandLine(DESK_ID)).resolves.toEqual(
			commandLine(),
		);
		expect(fetch.mock.calls.map(([url]) => String(url))).toEqual([
			`http://desk.test/api/v1/shows/${SHOW_ID}/objects/cue_list`,
			`http://desk.test/api/v2/desks/${DESK_ID}/command-line`,
		]);
		for (const [, init] of fetch.mock.calls) {
			const headers = new Headers(init?.headers);
			expect(headers.get("authorization")).toBe("Bearer session-token");
			expect(headers.get("x-light-desk-token")).toBe("desk-token");
		}
	});
});

function commandLine() {
	return {
		text: "FIXTURE",
		target: "FIXTURE",
		pristine: true,
		revision: 6,
		pendingChoice: null,
	};
}

function commandLineWire() {
	return {
		text: "FIXTURE",
		target: "FIXTURE",
		pristine: true,
		revision: 6,
		pending_choice: null,
	};
}

function cueListObject() {
	return {
		kind: "cue_list",
		id: "destination",
		revision: 2,
		updated_at: "",
		body: outcome().projections[0].body,
	};
}

function outcome() {
	return {
		status: "changed",
		request_id: REQUEST_ID,
		choice_id: CHOICE_ID,
		correlation_id: "77777777-7777-4777-8777-777777777777",
		replayed: false,
		show_id: SHOW_ID,
		summary: {
			operation: "copy",
			mode: "plain",
			source_cue_id: SOURCE_CUE_ID,
			source_cue_number: 1,
			destination_cue_id: DESTINATION_CUE_ID,
			destination_cue_number: 2,
		},
		show_revision: 8,
		projections: [
			{
				cue_list_id: CUE_LIST_ID,
				object_id: "destination",
				object_revision: 2,
				body: {
					id: CUE_LIST_ID,
					name: "Destination",
					priority: 0,
					mode: "sequence",
					looped: false,
					cues: [
						{
							id: DESTINATION_CUE_ID,
							number: 2,
							name: "Cue 2",
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
	};
}
