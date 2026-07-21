import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	BootstrapSnapshot,
	OutputRoute,
	ServerEvent,
	SessionResponse,
	VersionedObject,
} from "../../api/types";
import { routeOperatorEvent } from "./operatorEventRouting";
import { createServerEventRouter } from "./serverEventRouter";
import type { ServerState } from "./useServerState";

const session = {
	session_id: "session-1",
	user: { id: "user-1", name: "Operator", enabled: true },
	desk: { id: "desk-1", osc_alias: "main" },
} as SessionResponse;

function event(
	kind: string,
	payload: Record<string, unknown>,
	revision = 1,
): ServerEvent {
	return { revision, kind, payload };
}

function bootstrap(showId = "show-a"): BootstrapSnapshot {
	return {
		active_show: { id: showId, name: "Show" },
		active_programmers: [],
	} as unknown as BootstrapSnapshot;
}

function object<T = Record<string, unknown>>(
	kind: string,
	id: string,
	revision: number,
	body: T = {} as T,
): VersionedObject<T> {
	return { kind, id, revision, updated_at: "2026-07-19T00:00:00Z", body };
}

function apply<T>(current: T, next: T | ((value: T) => T)) {
	return typeof next === "function" ? (next as (value: T) => T)(current) : next;
}

function createHarness(showId = "show-a") {
	const loadShowObjects = vi.fn().mockResolvedValue(undefined);
	const unexpectedLegacyPlaybackRead = vi.fn();
	const clientMethods = {
		object: vi
			.fn<
				(
					showId: string,
					kind: string,
					id: string,
				) => Promise<VersionedObject<unknown>>
			>()
			.mockImplementation(async (_showId: string, kind: string, id: string) =>
				object(
					kind,
					id,
					3,
					kind === "group" ? { fixtures: ["fixture-3"] } : {},
				),
			),
		objects: vi.fn().mockResolvedValue([]),
		patch: vi.fn().mockResolvedValue({ revision: 3, fixtures: [], routes: [] }),
		bootstrap: vi.fn().mockResolvedValue(bootstrap(showId)),
		configuration: vi.fn().mockResolvedValue({ configuration: {}, matter: {} }),
		screens: vi.fn().mockResolvedValue({ screens: [], active_pages: {} }),
		shows: vi.fn().mockResolvedValue([]),
		mediaServers: vi.fn().mockResolvedValue({ fixtures: [] }),
		programmers: vi.fn().mockResolvedValue([]),
		fixtureLibrary: vi.fn().mockResolvedValue([]),
		fixtureProfiles: vi.fn().mockResolvedValue([]),
		fixtureProfileWarnings: vi.fn().mockResolvedValue([]),
		highlight: vi.fn().mockResolvedValue(null),
	};
	const client = new Proxy(clientMethods, {
		get(target, property, receiver) {
			if (property === "playbacks") {
				unexpectedLegacyPlaybackRead();
				return vi.fn().mockResolvedValue({ cue_lists: [], active: [] });
			}
			return Reflect.get(target, property, receiver);
		},
	});
	const state = {
		client,
		bootstrap: bootstrap(showId),
		cueObjects: [],
		outputRoutes: [],
		patchLayers: [],
		unresolvedMvrFixtures: [],
		deskLayout: null,
		stageLayout: null,
		selectedFixtures: [],
		selectedGroupId: null,
		commandLineWrite: { current: Promise.resolve() },
		commandLineEpoch: { current: 0 },
		commandTargetModeRef: { current: "FIXTURE" },
		highlightEpoch: { current: 0 },
		highlightWrite: { current: Promise.resolve() },
		highlightErrorSticky: { current: false },
		setBootstrap: vi.fn((next) => {
			state.bootstrap = apply(state.bootstrap, next) as BootstrapSnapshot;
		}),
		setCueObjects: vi.fn((next) => {
			state.cueObjects = apply(state.cueObjects, next);
		}),
		setOutputRoutes: vi.fn((next) => {
			state.outputRoutes = apply(state.outputRoutes, next);
		}),
		setPatchLayers: vi.fn((next) => {
			state.patchLayers = apply(state.patchLayers, next);
		}),
		setUnresolvedMvrFixtures: vi.fn((next) => {
			state.unresolvedMvrFixtures = apply(state.unresolvedMvrFixtures, next);
		}),
		setDeskLayout: vi.fn((next) => {
			state.deskLayout = apply(state.deskLayout, next);
		}),
		setStageLayout: vi.fn((next) => {
			state.stageLayout = apply(state.stageLayout, next);
		}),
		setSelectedFixtures: vi.fn((next) => {
			state.selectedFixtures = apply(state.selectedFixtures, next);
		}),
		setSelectedGroupId: vi.fn((next) => {
			state.selectedGroupId = apply(state.selectedGroupId, next);
		}),
		setCommandLineState: vi.fn(),
		setCommandLinePristine: vi.fn(),
		setConfiguration: vi.fn(),
		setMatter: vi.fn(),
		setScreens: vi.fn(),
		setShows: vi.fn(),
		setMediaServers: vi.fn(),
		setFixtureLibrary: vi.fn(),
		setFixtureProfiles: vi.fn(),
		setFixtureProfileWarnings: vi.fn(),
		setHighlight: vi.fn(),
		setHighlightError: vi.fn(),
	} as unknown as ServerState;
	return {
		client,
		loadShowObjects,
		state,
		unexpectedLegacyPlaybackRead,
		route: createServerEventRouter(() => state, session, loadShowObjects),
	};
}

function showObjectEvent(
	kind: string,
	id: string,
	objectRevision = 3,
	eventRevision = 1,
	extra: Record<string, unknown> = {},
) {
	return event(
		"show_object_changed",
		{
			show_id: "show-a",
			kind,
			id,
			revision: objectRevision,
			...extra,
		},
		eventRevision,
	);
}

afterEach(() => vi.restoreAllMocks());

describe("server event routing", () => {
	it("routes desk actions only to their matching desk", () => {
		const received: string[] = [];
		window.addEventListener(
			"light:desk-action",
			((incoming: CustomEvent<string>) => {
				received.push(incoming.detail);
			}) as EventListener,
			{ once: true },
		);
		routeOperatorEvent(
			event("desk_action", { action: "clear", desk_id: "another-desk" }),
			session,
			{} as ServerState,
		);
		routeOperatorEvent(
			event("desk_action", { action: "go", desk_id: session.desk.id }),
			session,
			{} as ServerState,
		);
		expect(received).toEqual(["go"]);
	});

	it("routes Update requests through the desk-scoped UI event", () => {
		const received: unknown[] = [];
		window.addEventListener(
			"light:update-target",
			((incoming: CustomEvent) => {
				received.push(incoming.detail);
			}) as EventListener,
			{ once: true },
		);
		const target = { family: { type: "cue" }, object_id: "cue-list-1" };
		routeOperatorEvent(
			event("update_target_requested", { desk_id: session.desk.id, target }),
			session,
			{} as ServerState,
		);
		expect(received).toEqual([target]);
	});

	it("does not broad-reload Playback runtime for semantic playback events", async () => {
		const harness = createHarness();
		harness.route(event("playback_changed", {}));
		await Promise.resolve();
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
	});
});

describe("show object event reconciliation", () => {
	const objectCases = [
		["cue_list", "cue-list-1", "setCueObjects"],
		["patch_layer", "layer-1", "setPatchLayers"],
		["unresolved_mvr_fixture", "mvr-1", "setUnresolvedMvrFixtures"],
		["user_layout", "user-1", "setDeskLayout"],
		["stage_layout", "main", "setStageLayout"],
	] as const;

	it.each(
		objectCases,
	)("reads only the changed %s object", async (kind, id, setter) => {
		const harness = createHarness();
		harness.route(showObjectEvent(kind, id));
		await vi.waitFor(() =>
			expect(harness.client.object).toHaveBeenCalledOnce(),
		);
		expect(harness.client.object).toHaveBeenCalledWith("show-a", kind, id);
		expect(harness.state[setter]).toHaveBeenCalledOnce();
		expect(harness.client.patch).not.toHaveBeenCalled();
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("reloads only the coupled route projection for a route object event", async () => {
		const harness = createHarness();
		const route = object<OutputRoute>("route", "route-1", 3, {
			protocol: "art_net",
			logical_universe: 1,
			destination_universe: 1,
			delivery_mode: "broadcast",
			destination: null,
			enabled: true,
			minimum_slots: 0,
		});
		harness.client.objects.mockResolvedValueOnce([route]);
		harness.route(showObjectEvent("route", "route-1"));
		await vi.waitFor(() =>
			expect(harness.client.objects).toHaveBeenCalledOnce(),
		);
		expect(harness.client.objects).toHaveBeenCalledWith("show-a", "route");
		expect(harness.state.outputRoutes).toEqual([route]);
		expect(harness.client.object).not.toHaveBeenCalled();
		expect(harness.client.patch).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("leaves patched Fixture events to the scoped Patch authority", async () => {
		const harness = createHarness();
		harness.route(showObjectEvent("patched_fixture", "fixture-1"));
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(harness.client.patch).not.toHaveBeenCalled();
		expect(harness.client.object).not.toHaveBeenCalled();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it.each(["playback", "playback_page"])(
		"leaves %s object events to scoped stores",
		async (kind) => {
			const harness = createHarness();
			harness.route(showObjectEvent(kind, "1"));
			await Promise.resolve();
			await Promise.resolve();
			expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
			expect(harness.client.object).not.toHaveBeenCalled();
			expect(harness.client.objects).not.toHaveBeenCalled();
			expect(harness.client.bootstrap).not.toHaveBeenCalled();
			expect(harness.loadShowObjects).not.toHaveBeenCalled();
		},
	);

	it("ignores malformed, unknown, other-show, and other-user object events", async () => {
		const harness = createHarness();
		harness.route(event("show_object_changed", { kind: "group" }, 1));
		harness.route(showObjectEvent("future_kind", "future-1", 1, 2));
		harness.route(
			event(
				"show_object_changed",
				{ show_id: "show-b", kind: "group", id: "1", revision: 1 },
				3,
			),
		);
		harness.route(showObjectEvent("user_layout", "user-2", 1, 4));
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.object).not.toHaveBeenCalled();
		expect(harness.client.patch).not.toHaveBeenCalled();
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("leaves migrated Group/Preset events to the view-scoped v2 store", async () => {
		const harness = createHarness();
		harness.route(showObjectEvent("group", "3", 2, 1));
		harness.route(showObjectEvent("preset", "2.1", 2, 2));
		harness.route(
			event(
				"preset_stored",
				{
					show_id: "show-a",
					revision: 2,
					preset_address: { family: "Color", number: 1 },
				},
				3,
			),
		);
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.object).not.toHaveBeenCalled();
		expect(harness.client.objects).not.toHaveBeenCalled();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("reconciles a route deletion from its small coupled projection", async () => {
		const harness = createHarness();
		const route = object<OutputRoute>("route", "route-1", 4, {
			protocol: "art_net",
			logical_universe: 1,
			destination_universe: 1,
			delivery_mode: "broadcast",
			destination: null,
			enabled: true,
			minimum_slots: 0,
		});
		harness.state.outputRoutes = [route];
		harness.route(
			showObjectEvent("route", "route-1", 5, 10, { deleted: true }),
		);
		await vi.waitFor(() =>
			expect(harness.state.setOutputRoutes).toHaveBeenCalledOnce(),
		);
		expect(harness.client.object).not.toHaveBeenCalled();
		expect(harness.client.objects).toHaveBeenCalledWith("show-a", "route");
		expect(harness.state.outputRoutes).toEqual([]);
	});

	it("applies an explicit generic-object deletion without a read", async () => {
		const harness = createHarness();
		harness.state.cueObjects = [object("cue_list", "cue-list-1", 4)] as never;
		harness.route(
			showObjectEvent("cue_list", "cue-list-1", 5, 10, { deleted: true }),
		);
		await vi.waitFor(() => expect(harness.state.cueObjects).toEqual([]));
		expect(harness.client.object).not.toHaveBeenCalled();
		expect(harness.client.objects).not.toHaveBeenCalled();
	});

	it("does not install an object response older than the announced revision", async () => {
		const harness = createHarness();
		harness.client.object.mockResolvedValueOnce(
			object("cue_list", "cue-list-1", 4),
		);
		harness.route(showObjectEvent("cue_list", "cue-list-1", 5, 10));
		await vi.waitFor(() =>
			expect(harness.client.object).toHaveBeenCalledOnce(),
		);
		await Promise.resolve();
		expect(harness.state.cueObjects).toEqual([]);
	});

	it("coalesces a same-object burst to the newest revision", async () => {
		const harness = createHarness();
		harness.route(showObjectEvent("cue_list", "cue-list-1", 1, 1));
		harness.route(showObjectEvent("cue_list", "cue-list-1", 2, 2));
		harness.route(showObjectEvent("cue_list", "cue-list-1", 3, 3));
		await vi.waitFor(() => expect(harness.state.cueObjects).toHaveLength(1));
		expect(harness.client.object).toHaveBeenCalledOnce();
		expect(harness.state.cueObjects[0].revision).toBe(3);
	});

	it("does not install an older response after a newer event arrives", async () => {
		const harness = createHarness();
		let resolveFirst!: (
			value: VersionedObject<Record<string, unknown>>,
		) => void;
		const first = new Promise<VersionedObject<Record<string, unknown>>>(
			(resolve) => {
				resolveFirst = resolve;
			},
		);
		harness.client.object
			.mockImplementationOnce(() => first)
			.mockResolvedValueOnce(
				object("cue_list", "cue-list-1", 2, { name: "new" }),
			);
		harness.route(showObjectEvent("cue_list", "cue-list-1", 1, 1));
		await vi.waitFor(() =>
			expect(harness.client.object).toHaveBeenCalledOnce(),
		);
		harness.route(showObjectEvent("cue_list", "cue-list-1", 2, 2));
		resolveFirst(object("cue_list", "cue-list-1", 1, { name: "old" }));
		await vi.waitFor(() =>
			expect(harness.client.object).toHaveBeenCalledTimes(2),
		);
		await vi.waitFor(() =>
			expect(harness.state.cueObjects[0]?.revision).toBe(2),
		);
		expect((harness.state.cueObjects[0].body as { name: string }).name).toBe(
			"new",
		);
	});

	it("accepts a recreate whose object revision restarted after deletion", async () => {
		const harness = createHarness();
		harness.state.cueObjects = [
			object("cue_list", "cue-list-1", 9, { name: "old" }),
		] as never;
		harness.client.object.mockResolvedValueOnce(
			object("cue_list", "cue-list-1", 1, { name: "new" }),
		);
		harness.route(
			showObjectEvent("cue_list", "cue-list-1", 10, 10, { deleted: true }),
		);
		harness.route(showObjectEvent("cue_list", "cue-list-1", 1, 11));
		await vi.waitFor(() =>
			expect(harness.state.cueObjects[0]?.revision).toBe(1),
		);
		expect(harness.client.object).toHaveBeenCalledOnce();
	});

	it("routes legacy stored Cue values to one affected resource", async () => {
		const harness = createHarness();
		harness.route(
			event(
				"preload_stored",
				{ target: "cue", target_id: "cue-list-1", revision: 3 },
				1,
			),
		);
		await vi.waitFor(() =>
			expect(harness.client.object).toHaveBeenCalledOnce(),
		);
		expect(harness.client.object).toHaveBeenCalledWith(
			"show-a",
			"cue_list",
			"cue-list-1",
		);
	});
});

describe("broad state hydration boundaries", () => {
	it.each([
		["normal", ["values"]],
		["Preload", ["preload_values"]],
		["Preload playback queue", ["preload_playback_queue"]],
		["combined", ["values", "preload_values"]],
		[
			"combined with playback queue",
			["values", "preload_values", "preload_playback_queue"],
		],
	])("leaves own %s value changes to scoped stores", async (_label, changes) => {
		const harness = createHarness();
		harness.route(
			event("programmer_changed", { user_id: session.user.id, changes }),
		);
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
	});

	it("does not hydrate serially unrepresentable transient controls", async () => {
		const harness = createHarness();
		harness.route(
			event("programmer_changed", { changes: ["transient_control"] }),
		);
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
	});

	it("leaves same-user peer-desk value changes to the shared scoped store", async () => {
		const harness = createHarness();
		harness.route(
			event("programmer_changed", {
				user_id: session.user.id,
				session_id: "peer-session",
				desk_id: "peer-desk",
				changes: ["values"],
			}),
		);
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
	});

	it("leaves an own queue transition with interaction to both scoped stores", async () => {
		const harness = createHarness();
		harness.route(
			event("programmer_changed", {
				user_id: session.user.id,
				command: "preload.go",
				changes: ["interaction", "preload_playback_queue"],
			}),
		);
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
	});

	it("leaves categorized command-line edits to the scoped interaction store", async () => {
		const harness = createHarness();
		harness.route(
			event(
				"programmer_changed",
				{
					command: "programmer.command_line",
					changes: ["interaction"],
				},
				1,
			),
		);
		await Promise.resolve();
		await Promise.resolve();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
		expect(harness.client.programmers).not.toHaveBeenCalled();
	});

	it("retains compatibility hydration for uncategorized Programmer changes", async () => {
		const harness = createHarness();
		harness.route(
			event(
				"programmer_changed",
				{
					command: "programmer.execute",
					changes: ["interaction", "values", "runtime"],
				},
				1,
			),
		);
		await vi.waitFor(() =>
			expect(harness.client.bootstrap).toHaveBeenCalledOnce(),
		);
	});

	it.each([
		["foreign", { user_id: "user-2", changes: ["values"] }],
		[
			"foreign playback queue",
			{ user_id: "user-2", changes: ["preload_playback_queue"] },
		],
		["unowned", { changes: ["values"] }],
		["mixed", { user_id: session.user.id, changes: ["values", "runtime"] }],
		["duplicated", { user_id: session.user.id, changes: ["values", "values"] }],
		["empty", { user_id: session.user.id, changes: [] }],
	])("retains compatibility hydration for %s value events", async (_label, payload) => {
		const harness = createHarness();
		harness.route(event("programmer_changed", payload));
		await vi.waitFor(() =>
			expect(harness.client.bootstrap).toHaveBeenCalledOnce(),
		);
	});

	it.each([
		["missing", undefined],
		["malformed", "interaction"],
		["expanded", ["interaction", "values"]],
		["duplicated", ["interaction", "interaction"]],
	])("retains compatibility hydration for %s change categories", async (_label, changes) => {
		const harness = createHarness();
		harness.route(
			event(
				"programmer_changed",
				{ command: "programmer.command_line", changes },
				1,
			),
		);

		await vi.waitFor(() =>
			expect(harness.client.bootstrap).toHaveBeenCalledOnce(),
		);
	});

	it.each([
		"programmer_changed",
		"programmer_cleared",
	])("does not reload all show objects for %s", async (kind) => {
		const harness = createHarness();
		harness.route(event(kind, {}, 1));
		await vi.waitFor(() =>
			expect(harness.client.bootstrap).toHaveBeenCalledOnce(),
		);
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
		expect(harness.client.objects).not.toHaveBeenCalled();
	});

	it("retains a full show-object load when a show opens", async () => {
		const harness = createHarness("show-a");
		harness.client.bootstrap.mockResolvedValueOnce(bootstrap("show-b"));
		harness.route(event("show_opened", { show_id: "show-b" }, 1));
		await vi.waitFor(() =>
			expect(harness.loadShowObjects).toHaveBeenCalledWith("show-b", "user-1"),
		);
		expect(harness.loadShowObjects).toHaveBeenCalledOnce();
		expect(harness.client.screens).toHaveBeenCalledOnce();
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
	});

	it("refreshes only Screens for a Playback Page desk event", async () => {
		const harness = createHarness();
		harness.route(event("playback_page_changed", { page: 2 }, 1));
		await vi.waitFor(() => expect(harness.client.screens).toHaveBeenCalledOnce());
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
		expect(harness.client.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});
});
