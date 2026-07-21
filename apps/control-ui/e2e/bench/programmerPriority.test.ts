import { describe, expect, it, vi } from "vitest";
import { ApiDriver } from "./api";
import { setProgrammerPriority } from "./programmerPriority";

const USER_ID = "11111111-1111-4111-8111-111111111111";
const DESK_ID = "22222222-2222-4222-8222-222222222222";
const REQUEST_ID = "33333333-3333-4333-8333-333333333333";
const CORRELATION_ID = "44444444-4444-4444-8444-444444444444";

describe("Programmer priority acceptance intent", () => {
	it("reads only the exact user snapshot and sends one revisioned v2 action", async () => {
		const fetchMock = vi.fn(
			async (input: RequestInfo | URL, init?: RequestInit) => {
				const url = String(input);
				if (url.endsWith("/snapshot")) return json(snapshot(), 200, 7);
				expect(init?.method).toBe("POST");
				return json(noChangeOutcome(), 200, 7);
			},
		);
		const outcome = await setProgrammerPriority(
			api(),
			{ surface: "api", priority: -23 },
			{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
		);

		expect(outcome).toMatchObject({
			status: "no_change",
			requestId: REQUEST_ID,
			projection: { userId: USER_ID, revision: 7, priority: -23 },
		});
		expect(fetchMock).toHaveBeenCalledTimes(2);
		const [snapshotCall, actionCall] = fetchMock.mock.calls;
		expect(String(snapshotCall[0])).toBe(
			`http://desk.local/api/v2/users/${USER_ID}/programmer-priority/snapshot`,
		);
		expect(String(actionCall[0])).toBe(
			`http://desk.local/api/v2/users/${USER_ID}/programmer-priority/actions`,
		);
		expect(JSON.parse(String(actionCall[1]?.body))).toEqual({
			request_id: REQUEST_ID,
			expected_revision: 7,
			priority: -23,
		});
		expect(
			fetchMock.mock.calls.every(([url]) =>
				String(url).match(/programmer-priority/),
			),
		).toBe(true);
	});

	it("uses the production decoder for changed replay and malformed authority", async () => {
		const replayFetch = vi.fn(async (input: RequestInfo | URL) =>
			String(input).endsWith("/snapshot")
				? json(snapshot(), 200, 7)
				: json({ ...changedOutcome(), replayed: true }, 200, 8),
		);
		await expect(
			setProgrammerPriority(
				api(),
				{ surface: "api", priority: -23 },
				{ fetch: replayFetch as typeof fetch, requestId: () => REQUEST_ID },
			),
		).resolves.toMatchObject({
			status: "changed",
			replayed: true,
			eventSequence: 19,
			projection: { revision: 8 },
		});

		const malformedFetch = vi.fn(async (input: RequestInfo | URL) =>
			String(input).endsWith("/snapshot")
				? json(snapshot(), 200, 7)
				: json(
						{
							...changedOutcome(),
							projection: { ...projection(8), user_id: DESK_ID },
						},
						200,
						8,
					),
		);
		await expect(
			setProgrammerPriority(
				api(),
				{ surface: "api", priority: -23 },
				{ fetch: malformedFetch as typeof fetch, requestId: () => REQUEST_ID },
			),
		).rejects.toThrow(/requested user/);
	});

	it("surfaces a typed revision conflict without attempting another action", async () => {
		const fetchMock = vi.fn(async (input: RequestInfo | URL) =>
			String(input).endsWith("/snapshot")
				? json(snapshot(), 200, 7)
				: json(
						{
							kind: "conflict",
							error: "priority revision conflict",
							current_revision: 8,
							retryable: false,
						},
						409,
					),
		);
		await expect(
			setProgrammerPriority(
				api(),
				{ surface: "api", priority: 9 },
				{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
			),
		).rejects.toEqual(
			expect.objectContaining({
				name: "ProgrammerPriorityTransportError",
				kind: "conflict",
				status: 409,
				currentRevision: 8,
				retryable: false,
			}),
		);
		expect(fetchMock).toHaveBeenCalledTimes(2);
	});
});

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = {
		session_id: "session",
		client_id: "client",
		token: "token",
		user: { id: USER_ID, name: "Operator" },
		desk: { id: DESK_ID, osc_alias: "main" },
	};
	return driver;
}

function projection(revision: number) {
	return {
		user_id: USER_ID,
		revision,
		priority: -23,
		changed_at: "2026-07-21T10:00:00Z",
	};
}

function snapshot() {
	return { cursor: { sequence: 18 }, projection: projection(7) };
}

function noChangeOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		projection: projection(7),
		status: "no_change",
		replayed: false,
		warning: null,
	};
}

function changedOutcome() {
	return {
		...noChangeOutcome(),
		projection: projection(8),
		status: "changed",
		event_sequence: 19,
	};
}

function json(value: unknown, status: number, revision?: number) {
	return new Response(JSON.stringify(value), {
		status,
		headers: revision == null ? undefined : { etag: `"${revision}"` },
	});
}
