import { describe, expect, it, vi } from "vitest";
import type { ApiDriver } from "./api";
import {
	batchProgrammerValues,
	clearProgrammerValues,
	type ProgrammerValueTiming,
	releaseProgrammerFixtureValue,
	releaseProgrammerGroupValue,
	setProgrammerFixtureValue,
	setProgrammerGroupValue,
} from "./programmerValues";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const FIXTURE_ID = "44444444-4444-4444-8444-444444444444";
const REQUEST_ID = "55555555-5555-4555-8555-555555555555";
const CORRELATION_ID = "66666666-6666-4666-8666-666666666666";
const timing: ProgrammerValueTiming = {
	fade: true,
	fadeMillis: 1_250,
	delayMillis: 75,
};

describe("Programmer values acceptance intents", () => {
	it.each([
		{
			name: "fixture set",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				setProgrammerFixtureValue(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						fixtureId: FIXTURE_ID,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.5 },
						timing,
					},
					dependencies(fetch),
				),
			action: {
				type: "set_fixture",
				fixture_id: FIXTURE_ID,
				attribute: "intensity",
				value: { kind: "normalized", value: 0.5 },
				timing: {
					fade: true,
					fade_millis: 1_250,
					delay_millis: 75,
				},
			},
		},
		{
			name: "fixture release",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				releaseProgrammerFixtureValue(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						fixtureId: FIXTURE_ID,
						attribute: "intensity",
					},
					dependencies(fetch),
				),
			action: {
				type: "release_fixture",
				fixture_id: FIXTURE_ID,
				attribute: "intensity",
			},
		},
		{
			name: "stored-empty Group set",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				setProgrammerGroupValue(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						groupId: "4",
						attribute: "pan",
						value: { kind: "spread", value: [0.2, 0.8] },
						timing,
					},
					dependencies(fetch),
				),
			action: {
				type: "set_group",
				group_id: "4",
				attribute: "pan",
				value: { kind: "spread", value: [0.2, 0.8] },
				timing: {
					fade: true,
					fade_millis: 1_250,
					delay_millis: 75,
				},
			},
		},
		{
			name: "Group release",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				releaseProgrammerGroupValue(
					driver,
					{
						surface: "api",
						showId: SHOW_ID,
						groupId: "4",
						attribute: "pan",
					},
					dependencies(fetch),
				),
			action: {
				type: "release_group",
				group_id: "4",
				attribute: "pan",
			},
		},
		{
			name: "clear",
			apply: (driver: ApiDriver, fetch: typeof globalThis.fetch) =>
				clearProgrammerValues(
					driver,
					{ surface: "api", showId: SHOW_ID },
					dependencies(fetch),
				),
			action: { type: "clear" },
		},
	])("sends one narrow $name action against exact authority", async ({
		apply,
		action,
	}) => {
		const fetchMock = programmerFetch(noChangeOutcome());
		const outcome = await apply(api(), fetchMock as typeof fetch);

		expect(outcome).toEqual({
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
			revision: 7,
			captureModeRevision: 3,
			status: "no_change",
			replayed: false,
			warning: null,
		});
		assertExactAuthorityCalls(fetchMock);
		expect(actionBodies(fetchMock)).toEqual([
			{
				request_id: REQUEST_ID,
				expected_revision: 7,
				expected_capture_mode_revision: 3,
				action,
			},
		]);
	});

	it("preserves batch order and timing in one application action", async () => {
		const fetchMock = programmerFetch(changedOutcome());
		const mutations = [
			{
				action: "set_fixture" as const,
				fixtureId: FIXTURE_ID,
				attribute: "intensity",
				value: { kind: "normalized" as const, value: 0.25 },
				timing,
			},
			{
				action: "set_group" as const,
				groupId: "4",
				attribute: "pan",
				value: { kind: "normalized" as const, value: 0.75 },
				timing: { fade: false, fadeMillis: null, delayMillis: null },
			},
			{
				action: "release_fixture" as const,
				fixtureId: FIXTURE_ID,
				attribute: "tilt",
			},
		];
		const outcome = await batchProgrammerValues(
			api(),
			{ surface: "api", showId: SHOW_ID, mutations },
			dependencies(fetchMock as typeof fetch),
		);

		expect(outcome).toMatchObject({
			status: "changed",
			revision: 8,
			eventSequence: 19,
		});
		expect(actionBodies(fetchMock)).toEqual([
			{
				request_id: REQUEST_ID,
				expected_revision: 7,
				expected_capture_mode_revision: 3,
				action: {
					type: "batch",
					mutations: [
						{
							type: "set_fixture",
							fixture_id: FIXTURE_ID,
							attribute: "intensity",
							value: { kind: "normalized", value: 0.25 },
							timing: {
								fade: true,
								fade_millis: 1_250,
								delay_millis: 75,
							},
						},
						{
							type: "set_group",
							group_id: "4",
							attribute: "pan",
							value: { kind: "normalized", value: 0.75 },
							timing: {
								fade: false,
								fade_millis: null,
								delay_millis: null,
							},
						},
						{
							type: "release_fixture",
							fixture_id: FIXTURE_ID,
							attribute: "tilt",
						},
					],
				},
			},
		]);
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it("decodes replayed outcomes and rejects materialized no-change projections", async () => {
		const replayFetch = programmerFetch({
			...changedOutcome(),
			replayed: true,
		});
		await expect(
			clearProgrammerValues(
				api(),
				{ surface: "api", showId: SHOW_ID },
				dependencies(replayFetch as typeof fetch),
			),
		).resolves.toMatchObject({
			status: "changed",
			replayed: true,
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
		});

		const malformedFetch = programmerFetch({
			...noChangeOutcome(),
			projection: valuesProjection(7),
		});
		await expect(
			clearProgrammerValues(
				api(),
				{ surface: "api", showId: SHOW_ID },
				dependencies(malformedFetch as typeof fetch),
			),
		).rejects.toThrow(/no projection/);
	});

	it("surfaces a typed revision conflict without retrying", async () => {
		const fetchMock = programmerFetch(
			{
				kind: "conflict",
				error: "Programmer values revision conflict",
				current_revision: 8,
				current_capture_mode_revision: 4,
				retryable: false,
			},
			409,
		);
		await expect(
			clearProgrammerValues(
				api(),
				{ surface: "api", showId: SHOW_ID },
				dependencies(fetchMock as typeof fetch),
			),
		).rejects.toMatchObject({
			name: "ProgrammerValuesActionError",
			kind: "conflict",
			status: 409,
			currentRevision: 8,
			currentCaptureModeRevision: 4,
			retryable: false,
		});
		expect(actionCalls(fetchMock)).toHaveLength(1);
	});

	it.each([
		[
			"Show",
			{ playback: playbackSnapshot("77777777-7777-4777-8777-777777777777") },
		],
		[
			"desk",
			{
				playback: playbackSnapshot(
					SHOW_ID,
					"88888888-8888-4888-8888-888888888888",
				),
			},
		],
		[
			"user",
			{ values: valuesSnapshot("99999999-9999-4999-8999-999999999999") },
		],
	] as const)("rejects a foreign %s authority before mutation", async (_scope, options) => {
		const fetchMock = programmerFetch(noChangeOutcome(), 200, options);
		await expect(
			clearProgrammerValues(
				api(),
				{ surface: "api", showId: SHOW_ID },
				dependencies(fetchMock as typeof fetch),
			),
		).rejects.toThrow(/foreign|requested user/);
		expect(actionCalls(fetchMock)).toHaveLength(0);
	});

	it("rejects a replaced session and Preload capture before mutation", async () => {
		const driver = api();
		const replacedFetch = programmerFetch(noChangeOutcome(), 200, {
			onPlayback: () => {
				const session = driver.session;
				if (!session) throw new Error("expected an authenticated test session");
				driver.session = { ...session, token: "replacement-token" };
			},
		});
		await expect(
			clearProgrammerValues(
				driver,
				{ surface: "api", showId: SHOW_ID },
				dependencies(replacedFetch as typeof fetch),
			),
		).rejects.toThrow(/scope changed/);
		expect(actionCalls(replacedFetch)).toHaveLength(0);

		const preloadFetch = programmerFetch(noChangeOutcome(), 200, {
			captureMode: captureModeSnapshot(true),
		});
		await expect(
			clearProgrammerValues(
				api(),
				{ surface: "api", showId: SHOW_ID },
				dependencies(preloadFetch as typeof fetch),
			),
		).rejects.toThrow(/Preload capture/);
		expect(actionCalls(preloadFetch)).toHaveLength(0);
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
	values?: unknown;
	captureMode?: unknown;
	onPlayback?: () => void;
}

function programmerFetch(
	outcome: unknown,
	status = 200,
	options: FetchOptions = {},
) {
	return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
		const url = String(input);
		if (url.endsWith("/playback-runtime/snapshot")) {
			options.onPlayback?.();
			return json(options.playback ?? playbackSnapshot());
		}
		if (url.endsWith("/programmer-values/snapshot"))
			return json(options.values ?? valuesSnapshot());
		if (url.endsWith("/programmer-capture-mode/snapshot"))
			return json(options.captureMode ?? captureModeSnapshot());
		if (url.endsWith("/programmer-values/actions"))
			return json(outcome, status);
		throw new Error(`Unexpected request ${url} ${init?.method ?? "GET"}`);
	});
}

function assertExactAuthorityCalls(
	fetchMock: ReturnType<typeof programmerFetch>,
) {
	const urls = fetchMock.mock.calls.map(([input]) => String(input));
	expect(urls).toHaveLength(4);
	expect(urls).toEqual(
		expect.arrayContaining([
			`http://desk.local/api/v2/desks/${DESK_ID}/playback-runtime/snapshot`,
			`http://desk.local/api/v2/users/${USER_ID}/programmer-values/snapshot`,
			`http://desk.local/api/v2/users/${USER_ID}/programmer-capture-mode/snapshot`,
			`http://desk.local/api/v2/users/${USER_ID}/programmer-values/actions`,
		]),
	);
	expect(
		urls.some((url) => /bootstrap|\/api\/v1|\/playbacks|programmers/.test(url)),
	).toBe(false);
	const playbackCall = fetchMock.mock.calls.find(([input]) =>
		String(input).endsWith("/playback-runtime/snapshot"),
	);
	expect(playbackCall?.[1]?.method).toBe("POST");
	expect(JSON.parse(String(playbackCall?.[1]?.body))).toEqual({
		identities: [],
	});
	expect(actionCalls(fetchMock)).toHaveLength(1);
}

function actionCalls(fetchMock: ReturnType<typeof programmerFetch>) {
	return fetchMock.mock.calls.filter(([input]) =>
		String(input).endsWith("/programmer-values/actions"),
	);
}

function actionBodies(fetchMock: ReturnType<typeof programmerFetch>) {
	return actionCalls(fetchMock).map(([, init]) =>
		JSON.parse(String(init?.body)),
	);
}

function valuesProjection(revision: number, userId = USER_ID) {
	return {
		user_id: userId,
		revision,
		fixture_values: [],
		group_values: [],
	};
}

function valuesSnapshot(userId = USER_ID) {
	return { cursor: { sequence: 18 }, projection: valuesProjection(7, userId) };
}

function captureModeSnapshot(preload = false) {
	return {
		cursor: { sequence: 17 },
		projection: {
			user_id: USER_ID,
			revision: 3,
			blind: preload,
			preview: false,
			preload_capture_programmer: preload,
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

function noChangeOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		revision: 7,
		capture_mode_revision: 3,
		status: "no_change",
		replayed: false,
		warning: null,
	};
}

function changedOutcome() {
	return {
		...noChangeOutcome(),
		revision: 8,
		status: "changed",
		projection: valuesProjection(8),
		event_sequence: 19,
	};
}

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), { status });
}
