import { describe, expect, it, vi } from "vitest";
import { ApiDriver } from "./api";
import {
	clearPendingProgrammerPreload,
	enterProgrammerPreload,
	goProgrammerPreload,
	releaseProgrammerPreload,
} from "./programmerPreloadLifecycle";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";

describe("Programmer Preload lifecycle acceptance intents", () => {
	it.each([
		["enter", enterProgrammerPreload, changedEnterOutcome],
		["clear_pending", clearPendingProgrammerPreload, noChangeOutcome],
		["release", releaseProgrammerPreload, noChangeOutcome],
	] as const)(
		"captures exact authority and sends one %s lifecycle action",
		async (action, apply, outcome) => {
			const fetchMock = lifecycleFetch(outcome());
			const result = await apply(
				api(),
				{ surface: "api", showId: SHOW_ID },
				{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
			);

			expect(result).toMatchObject({
				requestId: REQUEST_ID,
				status: action === "enter" ? "changed" : "no_change",
			});
			assertExactAuthorityCalls(fetchMock, false);
			expect(actionBodies(fetchMock)).toEqual([
				{
					request_id: REQUEST_ID,
					expected_capture_mode_revision: 3,
					expected_values_revision: 5,
					expected_queue_revision: 6,
					expected_selection_revision: 7,
					action: { type: action },
				},
			]);
		},
	);

	it("captures the exact Show revision and Playback cursor for one replayed GO", async () => {
		const fetchMock = lifecycleFetch(changedGoOutcome());
		const outcome = await goProgrammerPreload(
			api(),
			{ surface: "api", showId: SHOW_ID },
			{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
		);

		expect(outcome).toMatchObject({
			status: "changed",
			replayed: true,
			commit: {
				showId: SHOW_ID,
				showRevision: 12,
				playbackEventSequenceBefore: 41,
				committedAt: "2026-07-21T10:00:00Z",
				executedPlaybackActions: 1,
				executed: [{ playbackNumber: 9, action: "go" }],
			},
		});
		assertExactAuthorityCalls(fetchMock, true);
		expect(playbackSnapshotBodies(fetchMock)).toEqual([{ identities: [] }]);
		expect(actionBodies(fetchMock)).toEqual([
			{
				request_id: REQUEST_ID,
				expected_capture_mode_revision: 3,
				expected_values_revision: 5,
				expected_queue_revision: 6,
				expected_selection_revision: 7,
				action: {
					type: "go",
					show_id: SHOW_ID,
					expected_show_revision: 12,
					expected_playback_event_sequence: 41,
				},
			},
		]);
	});

	it("uses the production strict decoder for lifecycle outcomes", async () => {
		const malformed = { ...changedEnterOutcome(), complete_values: [] };
		await expect(
			enterProgrammerPreload(
				api(),
				{ surface: "api", showId: SHOW_ID },
				{ fetch: lifecycleFetch(malformed) as typeof fetch },
			),
		).rejects.toThrow(/complete_values.*declared wire field/);
	});

	it("surfaces typed conflict metadata without retrying the action", async () => {
		const fetchMock = lifecycleFetch({
			kind: "conflict",
			error: "Preload values revision conflict",
			current_revision: 8,
			current_related_revision: 9,
			retryable: false,
		}, 409);
		await expect(
			releaseProgrammerPreload(
				api(),
				{ surface: "api", showId: SHOW_ID },
				{ fetch: fetchMock as typeof fetch, requestId: () => REQUEST_ID },
			),
		).rejects.toMatchObject({
			name: "ProgrammerPreloadLifecycleTransportError",
			kind: "conflict",
			status: 409,
			currentRevision: 8,
			currentRelatedRevision: 9,
			retryable: false,
		});
		expect(actionBodies(fetchMock)).toHaveLength(1);
	});

	it("rejects a foreign Playback scope before the lifecycle mutation", async () => {
		const foreignShow = "66666666-6666-4666-8666-666666666666";
		const fetchMock = lifecycleFetch(
			changedGoOutcome(),
			200,
			playbackSnapshot(foreignShow),
		);
		await expect(
			goProgrammerPreload(
				api(),
				{ surface: "api", showId: SHOW_ID },
				{ fetch: fetchMock as typeof fetch },
			),
		).rejects.toThrow(/foreign Show/);
		expect(actionBodies(fetchMock)).toEqual([]);
	});

	it("rejects a foreign Playback desk before the lifecycle mutation", async () => {
		const foreignDesk = "77777777-7777-4777-8777-777777777777";
		const fetchMock = lifecycleFetch(
			changedGoOutcome(),
			200,
			playbackSnapshot(SHOW_ID, foreignDesk),
		);
		await expect(
			goProgrammerPreload(
				api(),
				{ surface: "api", showId: SHOW_ID },
				{ fetch: fetchMock as typeof fetch },
			),
		).rejects.toThrow(/foreign desk/);
		expect(actionBodies(fetchMock)).toEqual([]);
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

function lifecycleFetch(
	outcome: unknown,
	status = 200,
	playback = playbackSnapshot(),
) {
	return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
		const url = String(input);
		if (url.includes("programmer-capture-mode/snapshot"))
			return json(captureModeSnapshot());
		if (url.includes("programmer-preload-values/snapshot"))
			return json(valuesSnapshot());
		if (url.includes("programmer-preload-playback-queue/snapshot"))
			return json(queueSnapshot());
		if (url.includes("programming-interaction/snapshot"))
			return json(interactionSnapshot());
		if (url.includes("playback-runtime/snapshot")) return json(playback);
		if (url.includes("programmer-preload/actions"))
			return json(outcome, status);
		throw new Error(`Unexpected request ${url} ${init?.method ?? "GET"}`);
	});
}

function assertExactAuthorityCalls(
	fetchMock: ReturnType<typeof lifecycleFetch>,
	go: boolean,
) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toHaveLength(go ? 6 : 5);
	expect(urls).toEqual(
		expect.arrayContaining([
			`http://desk.local/api/v2/users/${USER_ID}/programmer-capture-mode/snapshot`,
			`http://desk.local/api/v2/users/${USER_ID}/programmer-preload-values/snapshot`,
			`http://desk.local/api/v2/users/${USER_ID}/programmer-preload-playback-queue/snapshot`,
			`http://desk.local/api/v2/desks/${DESK_ID}/programming-interaction/snapshot`,
			`http://desk.local/api/v2/users/${USER_ID}/programmer-preload/actions`,
		]),
	);
	expect(urls.some((url) => /bootstrap|\/playbacks|programmers/.test(url))).toBe(
		false,
	);
	expect(
		urls.filter((url) => url.endsWith("/programmer-preload/actions")),
	).toHaveLength(1);
	expect(
		fetchMock.mock.calls.filter(
			([input, init]) =>
				String(input).endsWith("/programmer-preload/actions") &&
				init?.method === "POST",
		),
	).toHaveLength(1);
	expect(
		urls.filter((url) => url.endsWith("/playback-runtime/snapshot")),
	).toHaveLength(go ? 1 : 0);
}

function actionBodies(fetchMock: ReturnType<typeof lifecycleFetch>) {
	return fetchMock.mock.calls
		.filter(([input]) => String(input).endsWith("/programmer-preload/actions"))
		.map(([, init]) => JSON.parse(String(init?.body)));
}

function playbackSnapshotBodies(fetchMock: ReturnType<typeof lifecycleFetch>) {
	return fetchMock.mock.calls
		.filter(([input]) => String(input).endsWith("/playback-runtime/snapshot"))
		.map(([, init]) => JSON.parse(String(init?.body)));
}

function captureMode(revision: number) {
	return {
		user_id: USER_ID,
		revision,
		blind: revision > 3,
		preview: false,
		preload_capture_programmer: true,
	};
}

function captureModeSnapshot() {
	return { cursor: { sequence: 30 }, projection: captureMode(3) };
}

function valuesProjection(revision: number) {
	return {
		user_id: USER_ID,
		revision,
		fixture_values: [],
		group_values: [],
	};
}

function valuesSnapshot() {
	return { cursor: { sequence: 31 }, projection: valuesProjection(5) };
}

function queueProjection(revision: number) {
	return { user_id: USER_ID, revision, actions: [] };
}

function queueSnapshot() {
	return { cursor: { sequence: 32 }, projection: queueProjection(6) };
}

function interactionSnapshot() {
	return {
		cursor: { sequence: 33 },
		projection: {
			desk_id: DESK_ID,
			command_line: {
				text: "",
				target: "FIXTURE",
				pristine: true,
				revision: 2,
				pending_choice: null,
			},
			selection: {
				selected: [],
				expression: null,
				revision: 7,
				gesture_open: false,
			},
		},
	};
}

function playbackSnapshot(showId = SHOW_ID, deskId = DESK_ID) {
	return {
		cursor: { sequence: 41 },
		desk: {
			scope: { show_id: showId, show_revision: 12 },
			desk_id: deskId,
			active_page: 1,
			selected_playback: null,
		},
		projections: [],
	};
}

function outcomeBase() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		replayed: false,
		active: false,
		capture_mode: captureMode(3),
		values_revision: 5,
		queue_revision: 6,
		selection_revision: 7,
		warning: null,
	};
}

function noChangeOutcome() {
	return { ...outcomeBase(), status: "no_change" };
}

function changedEnterOutcome() {
	return {
		...outcomeBase(),
		status: "changed",
		capture_mode: captureMode(4),
		capture_mode_event_sequence: 42,
	};
}

function changedGoOutcome() {
	return {
		...outcomeBase(),
		replayed: true,
		status: "changed",
		active: true,
		capture_mode: captureMode(4),
		capture_mode_event_sequence: 42,
		values_revision: 6,
		values_projection: valuesProjection(6),
		values_event_sequence: 43,
		queue_revision: 7,
		queue_projection: queueProjection(7),
		queue_event_sequence: 44,
		commit: {
			show_id: SHOW_ID,
			show_revision: 12,
			playback_event_sequence_before: 41,
			playback_event_sequence_after: 45,
			committed_at: "2026-07-21T10:00:00Z",
			programmer_fade_millis: 2_000,
			executed_playback_actions: 1,
			executed: [
				{
					playback_number: 9,
					page: null,
					action: "go",
					surface: "physical",
				},
			],
			runtime_changes: [],
		},
	};
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), { status });
}
