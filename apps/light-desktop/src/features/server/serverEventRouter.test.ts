import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	BootstrapSnapshot,
	RuntimeCapabilityEvent,
	SessionResponse,
	VersionedObject,
} from "../../api/types";
import { registerControlSurfaceTarget } from "../controlSurfaceInteraction/registry";
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
): RuntimeCapabilityEvent {
	if (kind === "desk_action")
		return {
			type: "operator_notification",
			notification: {
				type: "desk_action",
				revision,
				notification: {
					action: (payload.action as string | undefined) ?? null,
					control: (payload.control as string | undefined) ?? null,
					value: (payload.value as string | undefined) ?? null,
					request_id: (payload.request_id as string | undefined) ?? null,
					session_id: (payload.session_id as string | undefined) ?? null,
					desk_id: (payload.desk_id as string | undefined) ?? null,
					desk_alias: (payload.desk_alias as string | undefined) ?? null,
				},
			},
		};
	if (kind === "file_input")
		return {
			type: "operator_notification",
			notification: {
				type: "file_input",
				revision,
				notification: {
					action: payload.action as string,
					instance_id: payload.instance_id as string,
					session_id: payload.session_id as string,
					source_session_id: null,
					desk_id: payload.desk_id as string,
					operation: payload.operation as string,
					source: payload.source as string,
				},
			},
		};
	if (kind === "update_target_requested") {
		const target = payload.target as {
			family: { type: "cue" | "preset" | "group" };
			object_id: string;
		};
		return {
			type: "operator_notification",
			notification: {
				type: "update_workflow",
				revision,
				notification: {
					type: "target_requested",
					desk_id: payload.desk_id as string,
					target: {
						...target,
						family: target.family.type,
						playback_number: null,
						cue_id: null,
						cue_number: null,
						validate_active_context: null,
					},
				},
			},
		};
	}
	if (kind === "show_opened")
		return {
			type: "show_library_changed",
			change: { revision, kind: "show_opened" },
		};
	if (kind === "show_renamed")
		return {
			type: "show_library_changed",
			change: { revision, kind: "show_renamed" },
		};
	if (kind === "playback_page_changed")
		return {
			type: "screens_changed",
			change: { revision, kind: "playback_page" },
		};
	if (kind === "server_configuration_changed")
		return {
			type: "server_configuration_changed",
			change: { revision },
		};
	if (kind === "hardware_connection_changed")
		return {
			type: "hardware_connection_changed",
			change: { revision, connected: payload.connected === true },
		};
	if (kind === "highlight_changed")
		return {
			type: "highlight_changed",
			change: {
				revision,
				desk_id: payload.desk_id as string,
				user_id: payload.user_id as string,
				action: null,
				source: null,
				state: payload.state as never,
			},
		};
	if (kind === "programming_values_changed")
		return {
			type: "programming_values_changed",
			change: {
				user_id: session.user.id,
				revision,
				fixture_values: [],
				removed_fixture_values: [],
				group_values: [],
				removed_group_values: [],
				dynamic_definitions: [],
				dynamic_values: [],
				removed_dynamic_values: [],
			},
		};
	throw new Error(`Unsupported typed test event: ${kind}`);
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
		programmingInteractionSnapshot: vi.fn().mockResolvedValue({
			cursor: 1,
			projection: {
				deskId: "desk-1",
				commandLine: {
					text: "FIXTURE",
					target: "FIXTURE",
					pristine: true,
					revision: 1,
					pendingChoice: null,
				},
				selection: {
					selected: [],
					expression: null,
					revision: 1,
					gestureOpen: false,
				},
			},
		}),
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
	const api = {
		runtime: { bootstrap: client.bootstrap },
		desk: {
			configuration: client.configuration,
			programmers: client.programmers,
		},
		programming: {
			programmingInteractionSnapshot: client.programmingInteractionSnapshot,
		},
		fixtures: {
			patch: client.patch,
			fixtureLibrary: client.fixtureLibrary,
			fixtureProfiles: client.fixtureProfiles,
			fixtureProfileWarnings: client.fixtureProfileWarnings,
		},
		mediaOutput: {
			highlight: client.highlight,
			mediaServers: client.mediaServers,
		},
		playback: { screens: client.screens },
		showObjects: {
			object: client.object,
			objects: client.objects,
		},
		shows: { shows: client.shows },
	};
	const state = {
		api,
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
		beginDeskLoading: vi.fn(() => 3),
		finishDeskLoading: vi.fn(),
	} as unknown as ServerState;
	return {
		api,
		loadShowObjects,
		state,
		unexpectedLegacyPlaybackRead,
		route: createServerEventRouter(() => state, session, loadShowObjects),
	};
}

afterEach(() => vi.restoreAllMocks());

describe("server event routing", () => {
	it("routes attached keypad actions to the owning focused Macro Editor", () => {
		const received: unknown[] = [];
		const listener = ((incoming: CustomEvent<unknown>) => {
			received.push(incoming.detail);
		}) as EventListener;
		window.addEventListener("light:macro-editor-input", listener);
		try {
			routeOperatorEvent(
				event("file_input", {
					action: "digit-7",
					instance_id: "macro-editor:acceptance",
					session_id: session.session_id,
					desk_id: session.desk.id,
					operation: "macro_edit",
					source: "osc",
				}),
				session,
				{} as ServerState,
			);
			expect(received).toEqual([
				expect.objectContaining({
					action: "digit-7",
					instance_id: "macro-editor:acceptance",
				}),
			]);
		} finally {
			window.removeEventListener("light:macro-editor-input", listener);
		}
	});

	it("routes desk actions only to their matching desk", () => {
		const received: unknown[] = [];
		const release = registerControlSurfaceTarget({
			id: "shortcut-test",
			priority: 1,
			accepts: ({ type }) => type === "desk_shortcut",
			handle: (intent) => received.push(intent),
		});
		routeOperatorEvent(
			event("desk_action", {
				action: "shift-clear",
				desk_id: "another-desk",
			}),
			session,
			{} as ServerState,
		);
		routeOperatorEvent(
			event("desk_action", {
				action: "shift-clear",
				desk_id: session.desk.id,
			}),
			session,
			{} as ServerState,
		);
		expect(received).toEqual([
			{ type: "desk_shortcut", source: "osc", action: "shift_clear" },
		]);
		release();
	});

	it("routes encoder and navigation controls only for the matching OSC desk", () => {
		const received: Array<{ control: string; value: string }> = [];
		const listener = ((incoming: CustomEvent) => {
			received.push(incoming.detail);
		}) as EventListener;
		window.addEventListener("light:encoder-action", listener);
		try {
			for (const payload of [
				{ control: "nav", value: "down", desk_alias: "other" },
				{
					control: "nav",
					value: "down",
					desk_alias: "main",
					session_id: "another-session",
					desk_id: "another-desk",
				},
				{ control: "nav", value: "down", desk_alias: "main" },
				{
					control: "encode/2",
					value: "press",
					request_id: "hardware-encoder-2",
					desk_alias: "main",
				},
			])
				routeOperatorEvent(
					event("desk_action", payload),
					session,
					{} as ServerState,
				);
			expect(received).toEqual([
				{ control: "nav", value: "down", desk_alias: "main" },
				{
					control: "encode/2",
					value: "press",
					request_id: "hardware-encoder-2",
					desk_alias: "main",
				},
			]);
		} finally {
			window.removeEventListener("light:encoder-action", listener);
		}
	});

	it("routes canonical extension page and desk commands through their owning surfaces", () => {
		const commands: unknown[] = [];
		const steps: number[] = [];
		const release = registerControlSurfaceTarget({
			id: "extension-desk-command-test",
			priority: 1,
			accepts: ({ type }) => type === "desk_command",
			handle: (intent) => commands.push(intent),
		});
		const listener = ((event: CustomEvent<number>) => {
			steps.push(event.detail);
		}) as EventListener;
		window.addEventListener("light:playback-page-step", listener);
		try {
			routeOperatorEvent(
				event("desk_action", {
					action: "desk-stage",
					desk_id: session.desk.id,
				}),
				session,
				{} as ServerState,
			);
			routeOperatorEvent(
				event("desk_action", {
					action: "desk-dynamics",
					desk_id: session.desk.id,
				}),
				session,
				{} as ServerState,
			);
			routeOperatorEvent(
				event("desk_action", {
					control: "nav",
					value: "page-up",
					desk_alias: session.desk.osc_alias,
				}),
				session,
				{} as ServerState,
			);
			expect(commands).toEqual([
				{ type: "desk_command", source: "hardware", command: "stage" },
				{ type: "desk_command", source: "hardware", command: "dynamics" },
			]);
			expect(steps).toEqual([1]);
		} finally {
			release();
			window.removeEventListener("light:playback-page-step", listener);
		}
	});

	it("routes attached ALIGN to the active parameter controller", () => {
		const received: unknown[] = [];
		const listener = ((event: CustomEvent<unknown>) => {
			received.push(event.detail);
		}) as EventListener;
		window.addEventListener("light:align-action", listener);
		try {
			routeOperatorEvent(
				event("desk_action", {
					action: "align",
					request_id: "align-gesture-1",
					desk_id: session.desk.id,
				}),
				session,
				{} as ServerState,
			);
			expect(received).toEqual([
				expect.objectContaining({
					action: "align",
					request_id: "align-gesture-1",
				}),
			]);
		} finally {
			window.removeEventListener("light:align-action", listener);
		}
	});

	it("preserves the attached Shift action for the software command owner", () => {
		const received: string[] = [];
		const listener = ((event: CustomEvent<string>) => {
			received.push(event.detail);
		}) as EventListener;
		window.addEventListener("light:programmer-key", listener);
		try {
			routeOperatorEvent(
				event("desk_action", {
					action: "shift-enter",
					desk_id: session.desk.id,
				}),
				session,
				{} as ServerState,
			);
			expect(received).toEqual(["shift-enter"]);
		} finally {
			window.removeEventListener("light:programmer-key", listener);
		}
	});

	it("opens page selection for the attached page chord without stepping", () => {
		vi.useFakeTimers();
		const steps: number[] = [];
		let menus = 0;
		const stepListener = ((event: CustomEvent<number>) => {
			steps.push(event.detail);
		}) as EventListener;
		const menuListener = () => {
			menus += 1;
		};
		window.addEventListener("light:playback-page-step", stepListener);
		window.addEventListener("light:playback-page-menu", menuListener);
		try {
			for (const action of ["page-up", "page-down"])
				routeOperatorEvent(
					event("desk_action", {
						action,
						value: "down",
						desk_id: session.desk.id,
					}),
					session,
					{} as ServerState,
				);
			vi.advanceTimersByTime(200);
			expect(menus).toBe(1);
			expect(steps).toEqual([]);
			for (const action of ["page-up", "page-down"])
				routeOperatorEvent(
					event("desk_action", {
						action,
						value: "up",
						desk_id: session.desk.id,
					}),
					session,
					{} as ServerState,
				);
		} finally {
			window.removeEventListener("light:playback-page-step", stepListener);
			window.removeEventListener("light:playback-page-menu", menuListener);
			vi.useRealTimers();
		}
	});

	it("routes Update requests through the typed interaction owner", () => {
		const received: unknown[] = [];
		const release = registerControlSurfaceTarget({
			id: "update-test",
			priority: 1,
			accepts: ({ type }) => type === "update_target",
			handle: (intent) => received.push(intent),
		});
		const target = { family: { type: "cue" }, object_id: "cue-list-1" };
		routeOperatorEvent(
			event("update_target_requested", { desk_id: session.desk.id, target }),
			session,
			{} as ServerState,
		);
		expect(received).toEqual([
			{ type: "update_target", source: "osc", target },
		]);
		release();
	});

	it("does not broad-reload Playback runtime for typed Programmer events", async () => {
		const harness = createHarness();
		harness.route(event("programming_values_changed", {}));
		await Promise.resolve();
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
	});
});

describe("broad state hydration boundaries", () => {
	it("does not broadly hydrate or reload show objects for typed Programmer values", async () => {
		const harness = createHarness();
		harness.route(event("programming_values_changed", {}, 1));
		await Promise.resolve();
		expect(harness.api.runtime.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
		expect(harness.api.showObjects.objects).not.toHaveBeenCalled();
	});

	it("retains a full show-object load when a show opens", async () => {
		const harness = createHarness("show-a");
		harness.api.runtime.bootstrap.mockResolvedValueOnce(bootstrap("show-b"));
		harness.route(event("show_opened", { show_id: "show-b" }, 1));
		await vi.waitFor(() =>
			expect(harness.loadShowObjects).toHaveBeenCalledWith("show-b", "user-1"),
		);
		expect(harness.loadShowObjects).toHaveBeenCalledOnce();
		expect(harness.state.beginDeskLoading).toHaveBeenCalledWith(
			"Loading show Show…",
			"Installing the show engine snapshot and preparing control surfaces",
		);
		expect(harness.state.finishDeskLoading).toHaveBeenCalledWith(3);
		expect(harness.api.playback.screens).toHaveBeenCalledOnce();
		expect(harness.api.desk.programmers).not.toHaveBeenCalled();
		expect(
			harness.api.programming.programmingInteractionSnapshot,
		).toHaveBeenCalledWith("desk-1");
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
	});

	it("repairs a renamed Show without broad Bootstrap or catalog amplification", async () => {
		const harness = createHarness("show-a");
		harness.api.shows.shows.mockResolvedValueOnce([
			{ id: "show-a", name: "Renamed" },
		]);
		harness.route(event("show_renamed", {}, 2));

		await vi.waitFor(() =>
			expect(harness.api.shows.shows).toHaveBeenCalledOnce(),
		);
		expect(harness.api.runtime.bootstrap).not.toHaveBeenCalled();
		expect(harness.api.playback.screens).not.toHaveBeenCalled();
		expect(harness.api.mediaOutput.mediaServers).not.toHaveBeenCalled();
		expect(harness.api.fixtures.fixtureLibrary).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("refreshes only Screens for a Playback Page desk event", async () => {
		const harness = createHarness();
		harness.route(event("playback_page_changed", { page: 2 }, 1));
		await vi.waitFor(() =>
			expect(harness.api.playback.screens).toHaveBeenCalledOnce(),
		);
		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
		expect(harness.api.runtime.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("refreshes only Configuration for its typed system event", async () => {
		const harness = createHarness();
		harness.route(event("server_configuration_changed", {}, 4));
		await vi.waitFor(() =>
			expect(harness.api.desk.configuration).toHaveBeenCalledOnce(),
		);
		expect(harness.api.runtime.bootstrap).not.toHaveBeenCalled();
		expect(harness.api.playback.screens).not.toHaveBeenCalled();
		expect(harness.api.fixtures.fixtureLibrary).not.toHaveBeenCalled();
		expect(harness.api.showObjects.objects).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("installs Hardware connection state without refreshing Bootstrap", () => {
		const harness = createHarness();
		harness.route(event("hardware_connection_changed", { connected: true }, 5));

		expect(harness.state.bootstrap?.hardware_connected).toBe(true);
		expect(harness.api.runtime.bootstrap).not.toHaveBeenCalled();
		expect(harness.loadShowObjects).not.toHaveBeenCalled();
	});

	it("installs typed Highlight state without a follow-up read", async () => {
		const harness = createHarness();
		const state = {
			active: true,
			mode: "selection",
			output_enabled: true,
			capture_only: false,
			remembered: [],
			active_index: null,
			active_fixture: null,
			can_previous: false,
			can_next: false,
			owner_user_id: session.user.id,
			owner_user_name: session.user.name,
			message: null,
		};
		harness.route(
			event(
				"highlight_changed",
				{
					desk_id: session.desk.id,
					user_id: session.user.id,
					state,
				},
				8,
			),
		);

		expect(harness.state.setHighlight).toHaveBeenCalledWith(state);
		expect(harness.api.mediaOutput.highlight).not.toHaveBeenCalled();
		expect(harness.api.runtime.bootstrap).not.toHaveBeenCalled();
		expect(harness.api.showObjects.objects).not.toHaveBeenCalled();
	});
});
