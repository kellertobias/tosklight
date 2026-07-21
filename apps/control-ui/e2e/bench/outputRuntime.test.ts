import { describe, expect, it, vi } from "vitest";
import type { OutputRuntimeTransportError } from "../../src/features/outputRuntime/transport";
import { ApiDriver, type Session } from "./api";
import { setOutputRuntime } from "./outputRuntime";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const FOREIGN_SHOW_ID = "66666666-6666-4666-8666-666666666666";
const FOREIGN_DESK_ID = "77777777-7777-4777-8777-777777777777";

describe("Output runtime acceptance intent", () => {
	it("rejects a non-API surface before authority I/O", async () => {
		const fetchMock = outputFetch();
		const invalid = {
			...intent(),
			surface: "ui",
		} as unknown as Parameters<typeof setOutputRuntime>[1];

		await expect(
			setOutputRuntime(api(), invalid, {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(/only the API surface/);
		expect(fetchMock).not.toHaveBeenCalled();
	});

	it("captures exact authority and sends one revisioned changed action", async () => {
		const fetchMock = outputFetch({ outcome: changedOutcome() });
		const outcome = await setOutputRuntime(api(), intent(), {
			fetch: fetchMock as typeof fetch,
			requestId: () => REQUEST_ID,
		});

		expect(outcome).toMatchObject({
			status: "changed",
			requestId: REQUEST_ID,
			eventSequence: 19,
			projection: { revision: 5, grandMaster: 0.5, blackout: false },
		});
		assertNarrowCalls(fetchMock);
		expect(JSON.parse(String(actionCalls(fetchMock)[0]?.[1]?.body))).toEqual({
			request_id: REQUEST_ID,
			expected_show_id: SHOW_ID,
			expected_revision: 4,
			grand_master: 0.5,
			blackout: false,
		});
	});

	it("accepts a replayed no-change through the strict production decoder", async () => {
		const fetchMock = outputFetch({ outcome: noChangeOutcome() });

		await expect(
			setOutputRuntime(api(), intent(), {
				fetch: fetchMock as typeof fetch,
				requestId: () => REQUEST_ID,
			}),
		).resolves.toMatchObject({
			status: "no_change",
			replayed: true,
			eventSequence: null,
			projection: { revision: 4 },
		});
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("surfaces a typed revision conflict without retrying the action", async () => {
		const fetchMock = outputFetch({
			actionStatus: 409,
			outcome: {
				kind: "conflict",
				error: "Output runtime revision conflict",
				current_revision: 5,
				retryable: false,
			},
		});

		await expect(
			setOutputRuntime(api(), intent(), {
				fetch: fetchMock as typeof fetch,
				requestId: () => REQUEST_ID,
			}),
		).rejects.toEqual(
			expect.objectContaining<Partial<OutputRuntimeTransportError>>({
				name: "OutputRuntimeTransportError",
				kind: "conflict",
				status: 409,
				currentRevision: 5,
				retryable: false,
			}),
		);
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it.each([
		["Show", playbackSnapshot(FOREIGN_SHOW_ID, DESK_ID), /foreign Show/],
		["desk", playbackSnapshot(SHOW_ID, FOREIGN_DESK_ID), /foreign desk/],
	] as const)("rejects a foreign active %s before Output I/O", async (_, active, error) => {
		const fetchMock = outputFetch({ active });
		await expect(
			setOutputRuntime(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(error);
		expect(outputCalls(fetchMock)).toEqual([]);
	});

	it("rejects session replacement after snapshot before POST", async () => {
		const driver = api();
		const fetchMock = outputFetch({
			onOutputSnapshot: () => {
				driver.session = { ...session(), token: "replacement-token" };
			},
		});

		await expect(
			setOutputRuntime(driver, intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(/session changed/);
		expect(actionCalls(fetchMock)).toEqual([]);
	});

	it("rejects foreign Output authority and undeclared outcome fields", async () => {
		const foreignSnapshotFetch = outputFetch({
			snapshot: outputSnapshot({ scope: { show_id: FOREIGN_SHOW_ID } }),
		});
		await expect(
			setOutputRuntime(api(), intent(), {
				fetch: foreignSnapshotFetch as typeof fetch,
			}),
		).rejects.toThrow(/requested Show/);
		expect(actionCalls(foreignSnapshotFetch)).toEqual([]);

		const malformedFetch = outputFetch({
			outcome: { ...changedOutcome(), legacy_payload: {} },
		});
		await expect(
			setOutputRuntime(api(), intent(), {
				fetch: malformedFetch as typeof fetch,
				requestId: () => REQUEST_ID,
			}),
		).rejects.toThrow(/legacy_payload.*declared wire field/);
		expect(actionCalls(malformedFetch)).toHaveLength(1);
	});
});

interface OutputFetchOptions {
	active?: ReturnType<typeof playbackSnapshot>;
	snapshot?: ReturnType<typeof outputSnapshot>;
	outcome?: unknown;
	actionStatus?: number;
	onOutputSnapshot?: () => void;
}

function outputFetch(options: OutputFetchOptions = {}) {
	return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
		const url = String(input);
		if (url.endsWith("/playback-runtime/snapshot"))
			return json(options.active ?? playbackSnapshot());
		if (url.endsWith("/output-runtime/global-master")) {
			if (init?.method === "POST")
				return json(options.outcome ?? changedOutcome(), options.actionStatus);
			options.onOutputSnapshot?.();
			return json(options.snapshot ?? outputSnapshot());
		}
		throw new Error(`Unexpected request ${url}`);
	});
}

function assertNarrowCalls(fetchMock: ReturnType<typeof outputFetch>) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toEqual([
		`http://desk.local/api/v2/desks/${DESK_ID}/playback-runtime/snapshot`,
		`http://desk.local/api/v2/desks/${DESK_ID}/output-runtime/global-master`,
		`http://desk.local/api/v2/desks/${DESK_ID}/output-runtime/global-master`,
	]);
	expect(
		urls.some((url) =>
			/\/api\/v1\/|bootstrap|visualization|\/playbacks\//u.test(url),
		),
	).toBe(false);
	expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
		identities: [],
	});
	expect(actionCalls(fetchMock)).toHaveLength(1);
}

function outputCalls(fetchMock: ReturnType<typeof outputFetch>) {
	return fetchMock.mock.calls.filter(([input]) =>
		String(input).endsWith("/output-runtime/global-master"),
	);
}

function actionCalls(fetchMock: ReturnType<typeof outputFetch>) {
	return outputCalls(fetchMock).filter(([, init]) => init?.method === "POST");
}

function intent() {
	return {
		surface: "api" as const,
		showId: SHOW_ID,
		grandMaster: 0.5,
		blackout: false,
	};
}

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = session();
	return driver;
}

function session(): Session {
	return {
		session_id: "session",
		client_id: "client",
		token: "token",
		user: { id: USER_ID, name: "Operator" },
		desk: { id: DESK_ID, osc_alias: "main" },
	};
}

function playbackSnapshot(showId = SHOW_ID, deskId = DESK_ID) {
	return {
		cursor: { sequence: 17 },
		desk: {
			scope: { show_id: showId, show_revision: 9 },
			desk_id: deskId,
			active_page: 1,
			selected_playback: null,
		},
		projections: [],
	};
}

function projection(overrides: Record<string, unknown> = {}) {
	return {
		scope: { show_id: SHOW_ID },
		identity: "global_master",
		revision: 4,
		grand_master: 1,
		blackout: false,
		...overrides,
	};
}

function outputSnapshot(projectionOverrides: Record<string, unknown> = {}) {
	return {
		cursor: { sequence: 18 },
		projection: projection(projectionOverrides),
	};
}

function changedOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		projection: projection({
			revision: 5,
			grand_master: 0.5,
			blackout: false,
		}),
		status: "changed",
		event_sequence: 19,
		replayed: false,
		durability: "durable",
	};
}

function noChangeOutcome() {
	return {
		...changedOutcome(),
		projection: projection({ grand_master: 0.5, blackout: false }),
		status: "no_change",
		replayed: true,
		event_sequence: undefined,
	};
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), { status });
}
