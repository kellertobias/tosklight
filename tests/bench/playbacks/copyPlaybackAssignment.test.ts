import { describe, expect, it, vi } from "vitest";
import { ApiDriver } from "../core/api";
import { copyPlaybackAssignment } from "./copyPlaybackAssignment";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SESSION_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const CUE_LIST_ID = "66666666-6666-4666-8666-666666666666";

describe("copy Playback assignment acceptance intent", () => {
	it("captures exact active Show, Page, and Playback authority for one action", async () => {
		const fetchMock = topologyFetch(changedOutcome());
		const outcome = await copyPlaybackAssignment(api(), intent(), {
			fetch: fetchMock as typeof fetch,
			requestId: () => REQUEST_ID,
		});

		expect(outcome).toMatchObject({
			requestId: REQUEST_ID,
			status: "changed",
			showRevision: 8,
			resolution: {
				kind: "playback",
				page: 1,
				position: 1,
			},
		});
		assertNarrowCalls(fetchMock);
		const action = actionCalls(fetchMock)[0];
		expect((action[1]?.headers as Headers).get("if-match")).toBe('"7"');
		expect(JSON.parse(String(action[1]?.body))).toEqual({
			request_id: REQUEST_ID,
			action: {
				type: "assign_playback",
				address: { page: 1, position: 1 },
				definition: {
					...definition(),
					footprint: { type: "normal" },
					presentation_icon: null,
					presentation_image: null,
				},
				expected_page_revision: 3,
				expected_page_object_id: "1",
			},
		});
	});

	it("accepts a strict replayed no-change outcome without another action", async () => {
		const fetchMock = topologyFetch(noChangeOutcome(), {
			page: pageSnapshot({ 1: definition() }),
		});
		await expect(
			copyPlaybackAssignment(api(), intent(), {
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
			copyPlaybackAssignment(api(), intent(), {
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
			copyPlaybackAssignment(api(), intent(), {
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
			playbackRuntime(SHOW_ID, "88888888-8888-4888-8888-888888888888"),
			/foreign desk/,
		],
	] as const)("rejects a foreign active %s before mutation", async (_, active, error) => {
		const fetchMock = topologyFetch(changedOutcome(), { active });
		await expect(
			copyPlaybackAssignment(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(error);
		expect(actionCalls(fetchMock)).toEqual([]);
	});

	it("rejects a mixed source Page revision before mutation", async () => {
		const fetchMock = topologyFetch(changedOutcome(), {
			playbackShowRevision: 8,
		});
		await expect(
			copyPlaybackAssignment(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(/Show authority changed/);
		expect(actionCalls(fetchMock)).toEqual([]);
	});

	it.each([
		false,
		true,
	])("rejects replaced session after response (action=%s)", async (late) => {
		const driver = api();
		const base = topologyFetch(changedOutcome());
		const fetchMock = vi.fn(
			async (input: RequestInfo | URL, init?: RequestInit) => {
				const response = await base(input);
				if (
					String(input).includes(
						late ? "playback-topology/actions" : "playback-runtime/snapshot",
					)
				)
					driver.session = { ...driver.session!, token: "replacement" };
				return response;
			},
		);
		await expect(
			copyPlaybackAssignment(driver, intent(), {
				fetch: fetchMock as typeof fetch,
				requestId: () => REQUEST_ID,
			}),
		).rejects.toThrow(/session changed/);
		expect(
			actionCalls(fetchMock as ReturnType<typeof topologyFetch>),
		).toHaveLength(late ? 1 : 0);
	});

	it("rejects a mixed Show revision snapshot before mutation", async () => {
		const fetchMock = topologyFetch(changedOutcome(), { pageShowRevision: 8 });
		await expect(
			copyPlaybackAssignment(api(), intent(), {
				fetch: fetchMock as typeof fetch,
			}),
		).rejects.toThrow(/Show authority changed/);
		expect(actionCalls(fetchMock)).toEqual([]);
	});
});

function api() {
	const driver = new ApiDriver("http://desk.local");
	driver.session = {
		session_id: "11111111-1111-4111-8111-111111111111",
		client_id: "client",
		token: "token",
		desk: { id: DESK_ID, name: "Desk" },
	};
	return driver;
}

function intent() {
	return {
		surface: "api" as const,
		showId: SHOW_ID,
		sourceAddress: { page: 2, position: 7 },
		targetAddress: { page: 1, position: 1 },
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
	return vi.fn(async (input: RequestInfo | URL, _init?: RequestInit) => {
		const url = String(input);
		if (url.includes("playback-runtime/snapshot"))
			return json(options.active ?? playbackRuntime());
		if (url.includes("/objects/playback_page/1"))
			return json({
				show_id: SHOW_ID,
				show_revision: options.pageShowRevision ?? 7,
				kind: "playback_page",
				object_id: "1",
				object: options.page ?? pageSnapshot(),
			});
		if (url.includes("/objects/playback_page/2"))
			return json({
				show_id: SHOW_ID,
				show_revision: options.playbackShowRevision ?? 7,
				kind: "playback_page",
				object_id: "2",
				object: pageSnapshot({ 7: definition() }, 2),
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
		"http://desk.local/api/v2/playback-runtime/snapshot",
		"http://desk.local/api/v2/objects/playback_page/2",
		"http://desk.local/api/v2/objects/playback_page/1",
		"http://desk.local/api/v2/playback-topology/actions",
	]);
	expect(
		urls.some((url) => /bootstrap|\/playbacks|programmers/.test(url)),
	).toBe(false);
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
			command_page: 1,
			selected_surface_id: "main",
			resolved_pages: { main: 1 },
			selected_playback: null,
		},
		projections: [],
	};
}

function pageSnapshot(
	assignments: Record<number, ReturnType<typeof definition>> = {},
	page = 1,
) {
	return {
		kind: "playback_page",
		id: String(page),
		revision: 3,
		updated_at: "2026-07-21T10:00:00Z",
		body: { number: page, name: "Main", assignments },
	};
}

function definition() {
	return {
		name: "Recorded Playback 1",
		target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
		buttons: ["flash", "go", "off"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}

function changedOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		show_revision: 8,
		resolution: {
			kind: "playback",
			page: 1,
			position: 1,
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
		body: {
			number: 1,
			name: "Main",
			assignments: { 1: definition() },
		},
	};
}

function isNoChange(outcome: unknown) {
	return (outcome as { status?: unknown })?.status === "no_change";
}

function json(value: unknown, status = 200, headers?: Record<string, string>) {
	return new Response(JSON.stringify(value), { status, headers });
}
