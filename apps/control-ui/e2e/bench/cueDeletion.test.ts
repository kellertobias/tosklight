import { describe, expect, it, vi } from "vitest";
import { ApiDriver, commandLineOwnership } from "./api";
import {
	CueDeletionActionError,
	deleteCue,
	type DeleteCueIntent,
} from "./cueDeletion";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const CUE_LIST_ID = "66666666-6666-4666-8666-666666666666";
const CUE_ONE_ID = "77777777-7777-4777-8777-777777777777";
const CUE_TWO_ID = "88888888-8888-4888-8888-888888888888";
const CUE_THREE_ID = "99999999-9999-4999-8999-999999999999";

describe("Cue deletion acceptance intent", () => {
	it("routes whole-Cue deletion through the v2 command-line boundary", () => {
		expect(commandLineOwnership("DELETE SET 1 CUE 2")).toEqual({
			via: "command-line-http",
		});
	});

	it("resolves exact pool authority and sends one revisioned v2 action", async () => {
		const fetchMock = cueDeletionFetch();
		const outcome = await deleteCue(api(), poolIntent(), {
			fetch: fetchMock as typeof fetch,
			requestId: () => REQUEST_ID,
		});

		expect(outcome).toMatchObject({
			status: "changed",
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
			showId: SHOW_ID,
			showRevision: 8,
			cueList: {
				cueListId: CUE_LIST_ID,
				objectId: "stored-cuelist",
				objectRevision: 5,
			},
			deletedCue: { id: CUE_TWO_ID, number: 2 },
			showEventSequence: 21,
			replayed: false,
			persistenceWarning: null,
		});
		assertNarrowCalls(fetchMock, false);
		const [, init] = actionCalls(fetchMock)[0];
		expect((init?.headers as Headers).get("if-match")).toBe('"7"');
		expect((init?.headers as Headers).get("authorization")).toBe("Bearer token");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: REQUEST_ID,
			address: { type: "pool", playback_number: 1 },
			cue_number: 2,
			authority: {
				playback_number: 1,
				cue_list_id: CUE_LIST_ID,
				object_id: "stored-cuelist",
				object_revision: 4,
				cue_id: CUE_TWO_ID,
			},
		});
	});

	it.each([
		[
			"current Page",
			{ surface: "api", address: { type: "current_page", slot: 3 }, cueNumber: 2 },
			{ type: "current_page", expected_page: 1, slot: 3 },
		],
		[
			"explicit Page",
			{ surface: "api", address: { type: "page_slot", page: 1, slot: 3 }, cueNumber: 2 },
			{ type: "page_slot", page: 1, slot: 3 },
		],
	] as const)("resolves %s slot mapping without bootstrap", async (_, intent, address) => {
		const fetchMock = cueDeletionFetch();
		await deleteCue(api(), intent, {
			fetch: fetchMock as typeof fetch,
			requestId: () => REQUEST_ID,
		});
		assertNarrowCalls(fetchMock, true);
		const body = JSON.parse(String(actionCalls(fetchMock)[0][1]?.body));
		expect(body.address).toEqual(address);
		expect(body.authority.playback_number).toBe(1);
	});

	it("strictly accepts a replayed no-change outcome with no event", async () => {
		const fetchMock = cueDeletionFetch({
			outcome: noChangeOutcome(),
			actionEtag: '"7"',
		});
		await expect(
			deleteCue(api(), poolIntent(), {
				fetch: fetchMock as typeof fetch,
				requestId: () => REQUEST_ID,
			}),
		).resolves.toMatchObject({
			status: "no_change",
			replayed: true,
			showRevision: 7,
			cueList: { objectRevision: 4 },
			showEventSequence: null,
			persistenceWarning: "persistence retry queued",
		});
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("rejects a strict revision conflict and preserves its authority metadata", async () => {
		const fetchMock = cueDeletionFetch({
			actionStatus: 409,
			actionEtag: '"8"',
			outcome: {
				kind: "conflict",
				error: "the addressed Cuelist changed",
				current_revision: 8,
				current_related_revision: 5,
				retryable: false,
			},
		});
		const failure = deleteCue(api(), poolIntent(), {
			fetch: fetchMock as typeof fetch,
		});
		await expect(failure).rejects.toBeInstanceOf(CueDeletionActionError);
		await expect(failure).rejects.toMatchObject(
			expect.objectContaining({
				name: "CueDeletionActionError",
				kind: "conflict",
				status: 409,
				currentRevision: 8,
				currentRelatedRevision: 5,
				retryable: false,
			}),
		);
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("rejects foreign, mixed-revision, and ambiguous authority before mutation", async () => {
		for (const options of [
			{ active: playbackRuntime(SHOW_ID, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa") },
			{ playbackShowRevision: 8 },
			{ playbackObjects: [playbackObject(), playbackObject("duplicate-playback")] },
			{ cueListObjects: [cueListObject(), cueListObject("duplicate-cuelist")] },
		] satisfies FetchOptions[]) {
			const fetchMock = cueDeletionFetch(options);
			await expect(
				deleteCue(api(), poolIntent(), { fetch: fetchMock as typeof fetch }),
			).rejects.toThrow();
			expect(actionCalls(fetchMock)).toEqual([]);
		}
	});

	it("rejects malformed outcome identity, revision, event, and persistence fields", async () => {
		const cases = [
			{ ...changedOutcome(), request_id: crypto.randomUUID() },
			{ ...changedOutcome(), correlation_id: "not-a-uuid" },
			{ ...changedOutcome(), show_id: crypto.randomUUID() },
			{ ...changedOutcome(), show_revision: 9 },
			{ ...changedOutcome(), cue_list: { ...projection(), cue_list_id: crypto.randomUUID() } },
			{ ...changedOutcome(), deleted_cue: { id: CUE_ONE_ID, number: 2 } },
			without(changedOutcome(), "show_event_sequence"),
			{ ...changedOutcome(), persistence_warning: { message: "bad" } },
			{ ...changedOutcome(), legacy_payload: {} },
		];
		for (const outcome of cases) {
			const fetchMock = cueDeletionFetch({ outcome });
			await expect(
				deleteCue(api(), poolIntent(), {
					fetch: fetchMock as typeof fetch,
					requestId: () => REQUEST_ID,
				}),
			).rejects.toThrow();
		}
	});

	it("drops a mutation when the session changes before sending", async () => {
		const driver = api();
		const fetchMock = cueDeletionFetch({
			onCueLists: () => {
				driver.session = replacementSession();
			},
		});
		await expect(
			deleteCue(driver, poolIntent(), { fetch: fetchMock as typeof fetch }),
		).rejects.toThrow(/session changed before mutation/);
		expect(actionCalls(fetchMock)).toEqual([]);
	});

	it("drops a late response after session replacement", async () => {
		const driver = api();
		const fetchMock = cueDeletionFetch({
			onAction: () => {
				driver.session = replacementSession();
			},
		});
		await expect(
			deleteCue(driver, poolIntent(), { fetch: fetchMock as typeof fetch }),
		).rejects.toThrow(/session changed after response/);
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});
});

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = session();
	return driver;
}

function session() {
	return {
		session_id: "session",
		client_id: "client",
		token: "token",
		user: { id: USER_ID, name: "Operator" },
		desk: { id: DESK_ID, osc_alias: "main" },
	};
}

function replacementSession() {
	return { ...session(), session_id: "replacement", token: "replacement-token" };
}

function poolIntent(): DeleteCueIntent {
	return {
		surface: "api",
		address: { type: "pool", playbackNumber: 1 },
		cueNumber: 2,
	};
}

interface FetchOptions {
	active?: ReturnType<typeof playbackRuntime>;
	playbackShowRevision?: number;
	cueListShowRevision?: number;
	pageShowRevision?: number;
	playbackObjects?: unknown[];
	cueListObjects?: unknown[];
	pageObjects?: unknown[];
	outcome?: unknown;
	actionStatus?: number;
	actionEtag?: string;
	onCueLists?: () => void;
	onAction?: () => void;
}

function cueDeletionFetch(options: FetchOptions = {}) {
	return vi.fn(async (input: RequestInfo | URL, _init?: RequestInit) => {
		const url = String(input);
		if (url.includes("playback-runtime/snapshot"))
			return json(options.active ?? playbackRuntime());
		if (url.endsWith("/objects/playback"))
			return json(options.playbackObjects ?? [playbackObject()], 200, {
				etag: `"${options.playbackShowRevision ?? 7}"`,
			});
		if (url.endsWith("/objects/cue_list")) {
			options.onCueLists?.();
			return json(options.cueListObjects ?? [cueListObject()], 200, {
				etag: `"${options.cueListShowRevision ?? 7}"`,
			});
		}
		if (url.endsWith("/objects/playback_page"))
			return json(options.pageObjects ?? [pageObject()], 200, {
				etag: `"${options.pageShowRevision ?? 7}"`,
			});
		if (url.endsWith("/cues/delete")) {
			options.onAction?.();
			return json(options.outcome ?? changedOutcome(), options.actionStatus ?? 200, {
				etag: options.actionEtag ?? '"8"',
			});
		}
		throw new Error(`Unexpected request ${url}`);
	});
}

function actionCalls(fetchMock: ReturnType<typeof cueDeletionFetch>) {
	return fetchMock.mock.calls.filter(([input]) =>
		String(input).endsWith("/cues/delete"),
	);
}

function assertNarrowCalls(
	fetchMock: ReturnType<typeof cueDeletionFetch>,
	withPage: boolean,
) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toEqual([
		`http://desk.local/api/v2/desks/${DESK_ID}/playback-runtime/snapshot`,
		`http://desk.local/api/v1/shows/${SHOW_ID}/objects/playback`,
		`http://desk.local/api/v1/shows/${SHOW_ID}/objects/cue_list`,
		...(withPage
			? [`http://desk.local/api/v1/shows/${SHOW_ID}/objects/playback_page`]
			: []),
		`http://desk.local/api/v2/desks/${DESK_ID}/shows/${SHOW_ID}/cues/delete`,
	]);
	expect(urls.some((url) => /bootstrap|\/playbacks|programmers/.test(url))).toBe(false);
	expect(actionCalls(fetchMock)).toHaveLength(1);
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

function playbackObject(id = "stored-playback") {
	return {
		kind: "playback",
		id,
		revision: 3,
		updated_at: "2026-07-21T10:00:00Z",
		body: {
			number: 1,
			name: "Main Cuelist",
			target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
		},
	};
}

function cueListObject(id = "stored-cuelist") {
	return {
		kind: "cue_list",
		id,
		revision: 4,
		updated_at: "2026-07-21T10:00:00Z",
		body: cueListBody(true),
	};
}

function pageObject() {
	return {
		kind: "playback_page",
		id: "stored-page",
		revision: 2,
		updated_at: "2026-07-21T10:00:00Z",
		body: { number: 1, name: "Main", slots: { 3: 1 } },
	};
}

function cueListBody(withDeletedCue: boolean) {
	return {
		id: CUE_LIST_ID,
		name: "Main Cuelist",
		priority: 0,
		mode: "sequence",
		looped: false,
		cues: [
			cue(CUE_ONE_ID, 1),
			...(withDeletedCue ? [cue(CUE_TWO_ID, 2)] : []),
			cue(CUE_THREE_ID, 3),
		],
	};
}

function cue(id: string, number: number) {
	return {
		id,
		number,
		name: `Cue ${number}`,
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "manual" },
		changes: [],
		group_changes: [],
		phasers: [],
	};
}

function changedOutcome() {
	return {
		status: "changed",
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: 8,
		cue_list: projection(),
		deleted_cue: { id: CUE_TWO_ID, number: 2 },
		show_event_sequence: 21,
		persistence_warning: null,
	};
}

function noChangeOutcome() {
	return {
		status: "no_change",
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		replayed: true,
		show_id: SHOW_ID,
		show_revision: 7,
		cue_list: projection(4, true),
		deleted_cue: { id: CUE_TWO_ID, number: 2 },
		persistence_warning: "persistence retry queued",
	};
}

function projection(objectRevision = 5, withDeletedCue = false) {
	return {
		cue_list_id: CUE_LIST_ID,
		object_id: "stored-cuelist",
		object_revision: objectRevision,
		body: cueListBody(withDeletedCue),
	};
}

function without(value: Record<string, unknown>, key: string) {
	const copy = { ...value };
	delete copy[key];
	return copy;
}

function json(
	value: unknown,
	status = 200,
	headers?: Record<string, string>,
) {
	return new Response(JSON.stringify(value), { status, headers });
}
