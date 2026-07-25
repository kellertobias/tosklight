import { describe, expect, it, vi } from "vitest";
import type {
	ProgrammerPreloadLifecycleRequest,
	ProgrammerPreloadLifecycleScope,
} from "../features/programmerPreloadLifecycle/contracts";
import { HttpProgrammerPreloadLifecycleTransport } from "./ProgrammerPreloadLifecycleTransport";
import { WireValidationError } from "./wireValidation";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const OTHER_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const SCOPE = { showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID };

function request(
	overrides: Partial<ProgrammerPreloadLifecycleRequest> = {},
): ProgrammerPreloadLifecycleRequest {
	return {
		requestId: "preload-release",
		expectedCaptureModeRevision: 3,
		expectedValuesRevision: 5,
		expectedQueueRevision: 6,
		expectedSelectionRevision: 7,
		action: { type: "release" },
		...overrides,
	};
}

function noChange(requestId = "preload-release") {
	return {
		request_id: requestId,
		correlation_id: CORRELATION_ID,
		replayed: false,
		status: "no_change",
		active: false,
		capture_mode: {
			user_id: USER_ID,
			revision: 3,
			blind: false,
			preview: false,
			preload_capture_programmer: false,
		},
		values_revision: 5,
		queue_revision: 6,
		selection_revision: 7,
		warning: null,
	};
}

function harness(fetch = vi.fn<typeof globalThis.fetch>()) {
	const transport = new HttpProgrammerPreloadLifecycleTransport({
		baseUrl: "http://desk.local/",
		sessionToken: "session-token",
		authenticatedUserId: USER_ID,
		authenticatedDeskId: DESK_ID,
		deskBoundaryToken: "desk-boundary",
		fetch,
	});
	return { fetch, transport };
}

describe("HttpProgrammerPreloadLifecycleTransport", () => {
	it("constructs without I/O and posts one sparse typed no-change action", async () => {
		const fetch = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValue(new Response(JSON.stringify(noChange())));
		const { transport } = harness(fetch);

		expect(fetch).not.toHaveBeenCalled();
		await expect(transport.applyAction(SCOPE, request())).resolves.toMatchObject({
			status: "no_change",
			valuesProjection: null,
			queueProjection: null,
			commit: null,
		});
		expect(fetch).toHaveBeenCalledOnce();
		const [url, options] = fetch.mock.calls[0] ?? [];
		expect(url).toBe(
			`http://desk.local/api/v2/users/${USER_ID}/programmer-preload/actions`,
		);
		expect(options?.method).toBe("POST");
		expect(JSON.parse(String(options?.body))).toEqual({
			request_id: "preload-release",
			expected_capture_mode_revision: 3,
			expected_values_revision: 5,
			expected_queue_revision: 6,
			expected_selection_revision: 7,
			action: { type: "release" },
		});
		expect((options?.headers as Headers).get("authorization")).toBe(
			"Bearer session-token",
		);
		expect((options?.headers as Headers).get("x-light-desk-token")).toBe(
			"desk-boundary",
		);
	});

	it.each([
		["user", { ...SCOPE, userId: OTHER_ID }],
		["desk", { ...SCOPE, deskId: OTHER_ID }],
	] as const)("rejects a foreign authenticated %s before fetch", async (_label, scope) => {
		const { fetch, transport } = harness();

		await expect(
			transport.applyAction(scope as ProgrammerPreloadLifecycleScope, request()),
		).rejects.toMatchObject({ kind: "forbidden", status: 403 });
		expect(fetch).not.toHaveBeenCalled();
	});

	it.each([
		["control character", "invalid\nrequest"],
		["oversized UTF-8 byte length", "é".repeat(65)],
	] as const)("rejects a request ID with %s before fetch", async (_label, requestId) => {
		const { fetch, transport } = harness();

		await expect(
			transport.applyAction(SCOPE, request({ requestId })),
		).rejects.toBeInstanceOf(WireValidationError);
		expect(fetch).not.toHaveBeenCalled();
	});

	it("rejects unknown outcome fields and mismatched request IDs", async () => {
		const unknown = harness(
			vi.fn<typeof globalThis.fetch>().mockResolvedValue(
				new Response(JSON.stringify({ ...noChange(), legacy_values: [] })),
			),
		);
		await expect(unknown.transport.applyAction(SCOPE, request())).rejects.toThrow(
			/legacy_values.*declared wire field/,
		);

		const mismatched = harness(
			vi.fn<typeof globalThis.fetch>().mockResolvedValue(
				new Response(JSON.stringify(noChange("another-request"))),
			),
		);
		await expect(
			mismatched.transport.applyAction(SCOPE, request()),
		).rejects.toThrow(/request_id/);
	});
});
