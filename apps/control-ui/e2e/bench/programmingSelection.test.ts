import { describe, expect, it, vi } from "vitest";
import { ApiDriver } from "./api";
import {
	gestureActiveProgrammingSelection,
	gestureProgrammingSelection,
	replaceActiveProgrammingSelection,
	replaceProgrammingSelection,
	selectProgrammingGroup,
} from "./programmingSelection";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";
const REQUEST_ID = "55555555-5555-4555-8555-555555555555";
const CORRELATION_ID = "66666666-6666-4666-8666-666666666666";

describe("Programming selection acceptance intents", () => {
	it.each([
		{
			name: "replacement",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				replaceProgrammingSelection(
					driver,
					{ surface: "api", showId: SHOW_ID, fixtures: [FIXTURE_ID] },
					dependencies(fetch),
				),
			action: {
				action: "replace",
				fixtures: [FIXTURE_ID],
				expected_revision: 8,
			},
		},
		{
			name: "fixture removal gesture",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				gestureProgrammingSelection(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						source: { type: "fixture", fixtureId: FIXTURE_ID },
						remove: true,
					},
					dependencies(fetch),
				),
			action: {
				action: "gesture",
				source: { type: "fixture", fixture_id: FIXTURE_ID },
				remove: true,
			},
		},
		{
			name: "live Group gesture",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				gestureProgrammingSelection(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						source: { type: "live_group", groupId: "4" },
					},
					dependencies(fetch),
				),
			action: {
				action: "gesture",
				source: { type: "live_group", group_id: "4" },
				remove: false,
			},
		},
		{
			name: "frozen Group selection",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				selectProgrammingGroup(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						groupId: "4",
						frozen: true,
						rule: { type: "every_nth", n: 2, offset: 1 },
					},
					dependencies(fetch),
				),
			action: {
				action: "select_group",
				group_id: "4",
				frozen: true,
				rule: { type: "every_nth", n: 2, offset: 1 },
				expected_revision: 8,
			},
		},
	])("sends one strict $name action against exact authority", async ({ apply, action }) => {
		const fetchMock = selectionFetch();
		const outcome = await apply(api(), fetchMock as typeof fetch);

		expect(outcome).toMatchObject({
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
			eventSequence: 19,
			replayed: false,
			selection: { revision: 9, selected: [FIXTURE_ID] },
		});
		expect(actionBodies(fetchMock)).toEqual([
			{ request_id: REQUEST_ID, ...action },
		]);
		assertExactCalls(fetchMock);
	});

	it("rejects foreign Show and desk authority before mutation", async () => {
		for (const playback of [
			playbackSnapshot("77777777-7777-4777-8777-777777777777"),
			playbackSnapshot(SHOW_ID, "88888888-8888-4888-8888-888888888888"),
		]) {
			const fetchMock = selectionFetch({ playback });
			await expect(
				replaceProgrammingSelection(
					api(),
					{ surface: "api", showId: SHOW_ID, fixtures: [] },
					dependencies(fetchMock as typeof fetch),
				),
			).rejects.toThrow(/foreign/);
			expect(actionCalls(fetchMock)).toHaveLength(0);
		}
	});

	it("captures the active Show for shared setup helpers without a bootstrap read", async () => {
		const fetchMock = selectionFetch();
		await replaceActiveProgrammingSelection(
			api(),
			{ surface: "api", fixtures: [] },
			dependencies(fetchMock as typeof fetch),
		);
		await gestureActiveProgrammingSelection(
			api(),
			{
				surface: "api",
				source: { type: "live_group", groupId: "4" },
			},
			dependencies(fetchMock as typeof fetch),
		);
		expect(actionBodies(fetchMock)).toEqual([
			{
				request_id: REQUEST_ID,
				action: "replace",
				fixtures: [],
				expected_revision: 8,
			},
			{
				request_id: REQUEST_ID,
				action: "gesture",
				source: { type: "live_group", group_id: "4" },
				remove: false,
			},
		]);
		expect(
			fetchMock.mock.calls.some(([input]) =>
				/bootstrap|\/api\/v1/.test(String(input)),
			),
		).toBe(false);
	});

	it("rejects foreign Programming snapshots and replaced sessions", async () => {
		const foreign = selectionFetch({
			programming: interactionSnapshot("99999999-9999-4999-8999-999999999999"),
		});
		await expect(
			replaceProgrammingSelection(
				api(),
				{ surface: "api", showId: SHOW_ID, fixtures: [] },
				dependencies(foreign as typeof fetch),
			),
		).rejects.toThrow(/requested desk/);
		expect(actionCalls(foreign)).toHaveLength(0);

		const driver = api();
		const replaced = selectionFetch({
			onProgramming: () => {
				if (!driver.session) throw new Error("expected session");
				driver.session = { ...driver.session, token: "replacement" };
			},
		});
		await expect(
			replaceProgrammingSelection(
				driver,
				{ surface: "api", showId: SHOW_ID, fixtures: [] },
				dependencies(replaced as typeof fetch),
			),
		).rejects.toThrow(/scope changed/);
		expect(actionCalls(replaced)).toHaveLength(0);
	});

	it("uses the production decoder and surfaces one HTTP conflict", async () => {
		const malformed = selectionFetch({
			outcome: { ...selectionOutcome(), event_sequence: "19" },
		});
		await expect(
			replaceProgrammingSelection(
				api(),
				{ surface: "api", showId: SHOW_ID, fixtures: [] },
				dependencies(malformed as typeof fetch),
			),
		).rejects.toThrow(/event_sequence/);

		const conflict = selectionFetch({
			status: 409,
			outcome: { error: "selection revision conflict" },
		});
		await expect(
			replaceProgrammingSelection(
				api(),
				{ surface: "api", showId: SHOW_ID, fixtures: [] },
				dependencies(conflict as typeof fetch),
			),
		).rejects.toMatchObject({
			name: "ProgrammingSelectionHttpError",
			status: 409,
			message: "selection revision conflict",
		});
		expect(actionCalls(conflict)).toHaveLength(1);
	});

	it("decodes replay and rejects a late outcome after session replacement", async () => {
		const replay = selectionFetch({
			outcome: { ...selectionOutcome(), replayed: true },
		});
		await expect(
			replaceProgrammingSelection(
				api(),
				{ surface: "api", showId: SHOW_ID, fixtures: [FIXTURE_ID] },
				dependencies(replay as typeof fetch),
			),
		).resolves.toMatchObject({ requestId: REQUEST_ID, replayed: true });

		const driver = api();
		const late = selectionFetch({
			onAction: () => {
				if (!driver.session) throw new Error("expected session");
				driver.session = { ...driver.session, token: "replacement" };
			},
		});
		await expect(
			replaceProgrammingSelection(
				driver,
				{ surface: "api", showId: SHOW_ID, fixtures: [FIXTURE_ID] },
				dependencies(late as typeof fetch),
			),
		).rejects.toThrow(/scope changed/);
		expect(actionCalls(late)).toHaveLength(1);
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

function dependencies(fetch: typeof globalThis.fetch) {
	return { fetch, requestId: () => REQUEST_ID };
}

interface FetchOptions {
	playback?: unknown;
	programming?: unknown;
	outcome?: unknown;
	status?: number;
	onProgramming?: () => void;
	onAction?: () => void;
}

function selectionFetch(options: FetchOptions = {}) {
	return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
		const url = String(input);
		if (url.endsWith("/playback-runtime/snapshot"))
			return json(options.playback ?? playbackSnapshot());
		if (url.endsWith("/programming-interaction/snapshot")) {
			options.onProgramming?.();
			return json(options.programming ?? interactionSnapshot());
		}
		if (url.endsWith("/programming-selection/actions")) {
			options.onAction?.();
			const action = JSON.parse(String(init?.body)).action as string;
			return json(
				options.outcome ?? selectionOutcome(acceptedAction(action)),
				options.status ?? 200,
			);
		}
		throw new Error(`Unexpected request ${url} ${init?.method ?? "GET"}`);
	});
}

function assertExactCalls(fetchMock: ReturnType<typeof selectionFetch>) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toEqual(
		expect.arrayContaining([
			`http://desk.local/api/v2/desks/${DESK_ID}/playback-runtime/snapshot`,
			`http://desk.local/api/v2/desks/${DESK_ID}/programming-interaction/snapshot`,
			`http://desk.local/api/v2/desks/${DESK_ID}/programming-selection/actions`,
		]),
	);
	expect(urls).toHaveLength(3);
	expect(urls.some((url) => /bootstrap|\/api\/v1|playbacks|programmers/.test(url))).toBe(false);
	const playback = fetchMock.mock.calls.find(([input]) =>
		String(input).endsWith("/playback-runtime/snapshot"),
	);
	expect(playback?.[1]?.method).toBe("POST");
	expect(JSON.parse(String(playback?.[1]?.body))).toEqual({ identities: [] });
	expect(actionCalls(fetchMock)).toHaveLength(1);
}

function actionCalls(fetchMock: ReturnType<typeof selectionFetch>) {
	return fetchMock.mock.calls.filter(([input]) =>
		String(input).endsWith("/programming-selection/actions"),
	);
}

function actionBodies(fetchMock: ReturnType<typeof selectionFetch>) {
	return actionCalls(fetchMock).map(([, init]) => JSON.parse(String(init?.body)));
}

function playbackSnapshot(showId = SHOW_ID, deskId = DESK_ID) {
	return {
		cursor: { sequence: 17 },
		desk: {
			scope: { show_id: showId, show_revision: 12 },
			desk_id: deskId,
			active_page: 1,
			selected_playback: null,
		},
		projections: [],
	};
}

function interactionSnapshot(deskId = DESK_ID) {
	return {
		cursor: { sequence: 18 },
		projection: {
			desk_id: deskId,
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
				revision: 8,
				gesture_open: false,
			},
		},
	};
}

function acceptedAction(action: string) {
	if (action === "gesture") return "gesture_applied";
	if (action === "select_group") return "group_selected";
	return "replaced";
}

function selectionOutcome(action = "replaced") {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		action,
		applied: 1,
		selection: {
			selected: [FIXTURE_ID],
			expression: null,
			revision: 9,
			gesture_open: false,
		},
		event_sequence: 19,
		replayed: false,
		warning: null,
	};
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), { status });
}
