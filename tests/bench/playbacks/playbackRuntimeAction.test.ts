import { describe, expect, it, vi } from "vitest";
import { ApiDriver, type Session } from "../core/api";
import { goCueListPlayback } from "./playbackRuntimeAction";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SESSION_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const CUE_LIST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "66666666-6666-4666-8666-666666666666";
const FOREIGN_SHOW_ID = "77777777-7777-4777-8777-777777777777";
const FOREIGN_DESK_ID = "88888888-8888-4888-8888-888888888888";
const PLAYBACK_ADDRESS = { page: 2, position: 7 };

describe("Playback runtime acceptance intent", () => {
	it("sends one direct Cuelist GO through the exact v2 authority", async () => {
		const fetchMock = playbackFetch();

		await expect(
			goCueListPlayback(api(), intent(), dependencies(fetchMock)),
		).resolves.toMatchObject({
			correlation_id: CORRELATION_ID,
			requested: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			resolved: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			outcome: { status: "applied" },
			event_sequence: 19,
		});

		assertExactCalls(fetchMock);
	});

	it("sends each repeated live GO without an intent identity", async () => {
		const fetchMock = playbackFetch();
		const driver = api();
		await goCueListPlayback(driver, intent(), dependencies(fetchMock));
		await goCueListPlayback(driver, intent(), dependencies(fetchMock));
		expect(actionCalls(fetchMock)).toHaveLength(2);
		for (const [, init] of actionCalls(fetchMock)) {
			const body = JSON.parse(String(init?.body));
			expect(body).not.toHaveProperty("request_id");
			expect(body).toHaveProperty("control_source", {
				surface_id: "main",
				control_id: 1,
			});
		}
	});

	it.each([
		["Show", playbackSnapshot({ showId: FOREIGN_SHOW_ID }), /foreign Show/],
		["desk", playbackSnapshot({ deskId: FOREIGN_DESK_ID }), /foreign desk/],
	] as const)("rejects foreign %s authority before GO", async (_, snapshot, error) => {
		const fetchMock = playbackFetch({ snapshot });

		await expect(
			goCueListPlayback(api(), intent(), dependencies(fetchMock)),
		).rejects.toThrow(error);
		expect(actionCalls(fetchMock)).toHaveLength(0);
	});

	it("rejects malformed outcomes without retrying the live action", async () => {
		const malformedFetch = playbackFetch({
			outcome: { ...actionOutcome(), event_sequence: "19" },
		});
		await expect(
			goCueListPlayback(api(), intent(), dependencies(malformedFetch)),
		).rejects.toThrow(/event_sequence/);
		expect(actionCalls(malformedFetch)).toHaveLength(1);
	});

	it("rejects revision drift and never retries a server conflict", async () => {
		const splitRevision = playbackSnapshot({ playbackRevision: 13 });
		const splitFetch = playbackFetch({ snapshot: splitRevision });
		await expect(
			goCueListPlayback(api(), intent(), dependencies(splitFetch)),
		).rejects.toThrow(/revision changed/);
		expect(actionCalls(splitFetch)).toHaveLength(0);

		const conflictFetch = playbackFetch({
			status: 409,
			outcome: {
				kind: "conflict",
				error: "Playback authority revision changed",
				retryable: false,
			},
		});
		await expect(
			goCueListPlayback(api(), intent(), dependencies(conflictFetch)),
		).rejects.toMatchObject({ name: "ApiRequestError", status: 409 });
		expect(actionCalls(conflictFetch)).toHaveLength(1);
	});

	it("rejects session replacement before POST and after a late outcome", async () => {
		const beforeDriver = api();
		const beforeFetch = playbackFetch({
			onSnapshot: () => replaceSession(beforeDriver),
		});
		await expect(
			goCueListPlayback(beforeDriver, intent(), dependencies(beforeFetch)),
		).rejects.toThrow(/session changed/);
		expect(actionCalls(beforeFetch)).toHaveLength(0);

		const lateDriver = api();
		const lateFetch = playbackFetch({
			onAction: () => replaceSession(lateDriver),
		});
		await expect(
			goCueListPlayback(lateDriver, intent(), dependencies(lateFetch)),
		).rejects.toThrow(/session changed/);
		expect(actionCalls(lateFetch)).toHaveLength(1);
	});
});

function assertExactCalls(fetchMock: ReturnType<typeof playbackFetch>) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toEqual([
		"http://desk.local/api/v2/playback-runtime/snapshot",
		"http://desk.local/api/v2/playback-actions",
	]);
	expect(
		urls.some((url) =>
			/\/api\/v1|bootstrap|visualization|\/playbacks(?:\/|$)/u.test(url),
		),
	).toBe(false);
	const [snapshot, action] = fetchMock.mock.calls;
	expect(snapshot?.[1]?.method).toBe("POST");
	expect(JSON.parse(String(snapshot?.[1]?.body))).toEqual({
		identities: [
			{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
			{ kind: "playback", ...PLAYBACK_ADDRESS },
		],
	});
	expect(action?.[1]?.method).toBe("POST");
	expect(JSON.parse(String(action?.[1]?.body))).toEqual({
		address: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
		action: { type: "go", pressed: true },
		control_source: { surface_id: "main", control_id: 1 },
	});
	for (const [, init] of fetchMock.mock.calls)
		expect(new Headers(init?.headers).get("authorization")).toBe(
			"Bearer token",
		);
	expect(actionCalls(fetchMock)).toHaveLength(1);
}

interface PlaybackFetchOptions {
	snapshot?: unknown;
	outcome?: unknown;
	status?: number;
	onSnapshot?: () => void;
	onAction?: () => void;
}

function playbackFetch(options: PlaybackFetchOptions = {}) {
	return vi.fn(async (input: RequestInfo | URL, _init?: RequestInit) => {
		const url = String(input);
		if (url.endsWith("/playback-runtime/snapshot")) {
			options.onSnapshot?.();
			return json(options.snapshot ?? playbackSnapshot());
		}
		if (url.endsWith("/playback-actions")) {
			options.onAction?.();
			return json(options.outcome ?? actionOutcome(), options.status ?? 200);
		}
		throw new Error(`Unexpected request ${url}`);
	});
}

function actionCalls(fetchMock: ReturnType<typeof playbackFetch>) {
	return fetchMock.mock.calls.filter(([input]) =>
		String(input).endsWith("/playback-actions"),
	);
}

function dependencies(fetchMock: ReturnType<typeof playbackFetch>) {
	return {
		fetch: fetchMock as typeof globalThis.fetch,
	};
}

function intent() {
	return {
		surface: "api" as const,
		showId: SHOW_ID,
		playbackAddress: PLAYBACK_ADDRESS,
		cueListId: CUE_LIST_ID,
	};
}

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = session();
	return driver;
}

function session(): Session {
	return {
		session_id: "11111111-1111-4111-8111-111111111111",
		client_id: "client",
		token: "token",
		desk: { id: DESK_ID, name: "Test desk" },
	};
}

function replaceSession(driver: ApiDriver) {
	driver.session = { ...session(), token: "replacement-token" };
}

interface PlaybackSnapshotOptions {
	showId?: string;
	deskId?: string;
	playbackRevision?: number;
}

function playbackSnapshot(options: PlaybackSnapshotOptions = {}) {
	const showId = options.showId ?? SHOW_ID;
	const deskId = options.deskId ?? DESK_ID;
	return {
		cursor: { sequence: 17 },
		desk: deskProjection(showId, deskId, 12),
		projections: [
			cueListProjection(
				{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
				showId,
				12,
			),
			cueListProjection(
				{ kind: "playback", ...PLAYBACK_ADDRESS },
				showId,
				options.playbackRevision ?? 12,
			),
		],
	};
}

function actionOutcome() {
	return {
		correlation_id: CORRELATION_ID,
		requested: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
		resolved: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
		outcome: { status: "applied" },
		durability: "durable",
		projection: cueListProjection(
			{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
			SHOW_ID,
			12,
		),
		related: [],
		desk: deskProjection(SHOW_ID, DESK_ID, 12),
		event_sequence: 19,
		desk_event_sequence: null,
	};
}

function cueListProjection(
	requested:
		| { kind: "cue_list"; cue_list_id: string }
		| { kind: "playback"; page: number; position: number },
	showId: string,
	showRevision: number,
) {
	return {
		scope: { show_id: showId, show_revision: showRevision },
		identity: requested,
		target: "cue_list",
		cue_list_id: CUE_LIST_ID,
		runtime: null,
	};
}

function deskProjection(showId: string, deskId: string, showRevision: number) {
	return {
		scope: { show_id: showId, show_revision: showRevision },
		desk_id: deskId,
		command_page: 1,
		selected_surface_id: "main",
		resolved_pages: { main: 1 },
		selected_playback: null,
	};
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { "content-type": "application/json" },
	});
}
