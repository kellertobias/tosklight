import { describe, expect, it, vi } from "vitest";
import { ApiDriver } from "./api";
import { mapExistingPlaybackToSlot } from "./mapExistingPlaybackToSlot";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const CUE_LIST_ID = "66666666-6666-4666-8666-666666666666";

describe("map existing Playback acceptance intent", () => {
	it("captures exact active Show, Page, and Playback authority for one action", async () => {
		const fetchMock = topologyFetch(changedOutcome());
		const outcome = await mapExistingPlaybackToSlot(
			api(),
			intent(),
			{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
		);

		expect(outcome).toMatchObject({
			requestId: REQUEST_ID,
			status: "changed",
			showRevision: 8,
			resolution: {
				kind: "page_slot",
				page: 1,
				slot: 1,
				playbackNumber: 1,
			},
		});
		assertNarrowCalls(fetchMock);
		const action = actionCalls(fetchMock)[0];
		expect((action[1]?.headers as Headers).get("if-match")).toBe('"7"');
		expect(JSON.parse(String(action[1]?.body))).toEqual({
			request_id: REQUEST_ID,
			action: {
				type: "map_existing_playback",
				page: 1,
				slot: 1,
				playback_number: 1,
				expected_page_revision: 3,
				expected_page_object_id: "1",
				expected_playback_revision: 4,
				expected_playback_object_id: "1",
			},
		});
	});

	it("accepts a strict replayed no-change outcome without another action", async () => {
		const fetchMock = topologyFetch(noChangeOutcome(), {
			page: pageSnapshot({ 1: 1 }),
		});
		await expect(
			mapExistingPlaybackToSlot(api(), intent(), {
				fetch: fetchMock as typeof fetch,
				requestId: () => REQUEST_ID,
			}),
		).resolves.toMatchObject({
			status: "no_change",
			replayed: true,
			showRevision: 7,
		});
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("surfaces strict typed revision-conflict metadata", async () => {
		const fetchMock = topologyFetch(
			{
				kind: "conflict",
				error: "stale Playback Page revision",
				current_revision: 8,
				current_related_revision: 4,
				retryable: false,
			},
			{ actionStatus: 409, actionEtag: '"8"' },
		);
		await expect(
			mapExistingPlaybackToSlot(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toMatchObject({
			name: "PlaybackTopologyActionError",
			kind: "conflict",
			status: 409,
			currentRevision: 8,
			currentRelatedRevision: 4,
			retryable: false,
		});
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("uses the production strict outcome decoder", async () => {
		const malformed = { ...changedOutcome(), legacy_payload: {} };
		await expect(
			mapExistingPlaybackToSlot(api(), intent(), {
				fetch: topologyFetch(malformed) as typeof fetch,
			}),
		).rejects.toThrow(/legacy_payload.*declared wire field/);
	});

	it.each([
		[
			"Show",
			playbackRuntime("77777777-7777-4777-8777-777777777777", DESK_ID),
			/foreign Show/,
		],
		[
			"desk",
			playbackRuntime(
				SHOW_ID,
				"88888888-8888-4888-8888-888888888888",
			),
			/foreign desk/,
		],
	] as const)("rejects a foreign active %s before mutation", async (_, active, error) => {
		const fetchMock = topologyFetch(changedOutcome(), { active });
		await expect(
			mapExistingPlaybackToSlot(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(error);
		expect(actionCalls(fetchMock)).toEqual([]);
	});

	it("rejects a mixed Show revision snapshot before mutation", async () => {
		const fetchMock = topologyFetch(changedOutcome(), { pageShowRevision: 8 });
		await expect(
			mapExistingPlaybackToSlot(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(/Show authority changed/);
		expect(actionCalls(fetchMock)).toEqual([]);
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

function intent() {
	return {
		surface: "api" as const,
		showId: SHOW_ID,
		page: 1,
		slot: 1,
		playbackNumber: 1,
	};
}

interface FetchOptions {
	active?: ReturnType<typeof playbackRuntime>;
	page?: ReturnType<typeof pageSnapshot>;
	pageShowRevision?: number;
	playbackShowRevision?: number;
	actionStatus?: number;
	actionEtag?: string;
}

function topologyFetch(outcome: unknown, options: FetchOptions = {}) {
	return vi.fn(async (input: RequestInfo | URL) => {
		const url = String(input);
		if (url.includes("playback-runtime/snapshot"))
			return json(options.active ?? playbackRuntime());
		if (url.includes("/objects/playback_page/1"))
			return json(options.page ?? pageSnapshot(), 200, {
				"x-light-show-revision": `"${options.pageShowRevision ?? 7}"`,
			});
		if (url.includes("/objects/playback/1"))
			return json(playbackSnapshot(), 200, {
				"x-light-show-revision": `"${options.playbackShowRevision ?? 7}"`,
			});
		if (url.includes("/playback-topology/actions"))
			return json(outcome, options.actionStatus ?? 200, {
				etag: options.actionEtag ?? (isNoChange(outcome) ? '"7"' : '"8"'),
			});
		throw new Error(`Unexpected request ${url}`);
	});
}

function assertNarrowCalls(fetchMock: ReturnType<typeof topologyFetch>) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toEqual([
		`http://desk.local/api/v2/desks/${DESK_ID}/playback-runtime/snapshot`,
		`http://desk.local/api/v1/shows/${SHOW_ID}/objects/playback_page/1`,
		`http://desk.local/api/v1/shows/${SHOW_ID}/objects/playback/1`,
		`http://desk.local/api/v2/shows/${SHOW_ID}/playback-topology/actions`,
	]);
	expect(urls.some((url) => /bootstrap|\/playbacks|programmers/.test(url))).toBe(
		false,
	);
	expect(actionCalls(fetchMock)).toHaveLength(1);
}

function actionCalls(fetchMock: ReturnType<typeof topologyFetch>) {
	return fetchMock.mock.calls.filter(([input]) =>
		String(input).endsWith("/playback-topology/actions"),
	);
}

function playbackRuntime(showId = SHOW_ID, deskId = DESK_ID) {
	return {
		cursor: { sequence: 20 },
		desk: {
			scope: { show_id: showId, show_revision: 7 },
			desk_id: deskId,
			active_page: 1,
			selected_playback: null,
		},
		projections: [],
	};
}

function pageSnapshot(slots: Record<number, number> = {}) {
	return {
		kind: "playback_page",
		id: "1",
		revision: 3,
		updated_at: "2026-07-21T10:00:00Z",
		body: { number: 1, name: "Main", slots },
	};
}

function playbackSnapshot() {
	return {
		kind: "playback",
		id: "1",
		revision: 4,
		updated_at: "2026-07-21T10:00:00Z",
		body: {
			number: 1,
			name: "Recorded Playback 1",
			target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
		},
	};
}

function changedOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		show_revision: 8,
		resolution: {
			kind: "page_slot",
			page: 1,
			slot: 1,
			playback_number: 1,
		},
		status: "changed",
		objects: [pageOutcome(4)],
		event_sequence: 21,
		replayed: false,
	};
}

function noChangeOutcome() {
	return {
		...changedOutcome(),
		show_revision: 7,
		status: "no_change",
		objects: [pageOutcome(3)],
		replayed: true,
		event_sequence: undefined,
	};
}

function pageOutcome(revision: number) {
	return {
		state: "present",
		kind: "playback_page",
		object_id: "1",
		object_revision: revision,
		body: { number: 1, name: "Main", slots: { 1: 1 } },
	};
}

function isNoChange(outcome: unknown) {
	return (outcome as { status?: unknown })?.status === "no_change";
}

function json(
	value: unknown,
	status = 200,
	headers?: Record<string, string>,
) {
	return new Response(JSON.stringify(value), { status, headers });
}
