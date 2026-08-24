import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	BootstrapSnapshot,
	ScreenSnapshot,
	SessionResponse,
} from "../../api/types";
import {
	bootstrapConnection,
	deferredConnectionWarmupTasks,
	removeDisconnectedOtherClients,
	SINGLE_CLIENT_MODE_STORAGE_KEY,
} from "./connectionBootstrap";
import type { ServerState } from "./useServerState";

const user = { id: "user-1", name: "Operator", enabled: true };
const session = {
	session_id: "session-1",
	client_id: "current-client",
	token: "token-1",
	user,
	desk: { id: "desk-1" },
} as SessionResponse;

function bootstrap(): BootstrapSnapshot {
	return {
		users: [user],
		active_show: { id: "show-1", name: "Show" },
		active_programmers: [],
	} as unknown as BootstrapSnapshot;
}

function createHarness() {
	const initial = bootstrap();
	const screens: ScreenSnapshot = {
		screens: [],
		active_pages: {},
		programmer_control_surface: {
			owner_screen_id: null,
			visible_encoders: 6,
		},
	};
	const unexpectedLegacyPlaybackRead = vi.fn();
	const clientMethods = {
		bootstrap: vi.fn().mockResolvedValue(initial),
		login: vi.fn().mockResolvedValue(session),
		deskLock: vi.fn().mockResolvedValue({ locked: false }),
		patch: vi.fn().mockResolvedValue({ revision: 1, fixtures: [], routes: [] }),
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
		shows: vi.fn().mockResolvedValue([]),
		configuration: vi.fn().mockResolvedValue({ configuration: {}, matter: {} }),
		mediaServers: vi.fn().mockResolvedValue({ fixtures: [] }),
		fixtureLibrary: vi.fn().mockResolvedValue([]),
		fixtureProfiles: vi.fn().mockResolvedValue([]),
		fixtureProfileWarnings: vi.fn().mockResolvedValue([]),
		screens: vi.fn().mockResolvedValue(screens),
		removeClient: vi.fn().mockResolvedValue(undefined),
		commandHistory: vi.fn().mockResolvedValue([]),
	};
	const client = new Proxy(clientMethods, {
		get(target, property, receiver) {
			if (property === "playbacks") {
				unexpectedLegacyPlaybackRead();
				return vi.fn().mockRejectedValue(new Error("legacy Playback read"));
			}
			return Reflect.get(target, property, receiver);
		},
	});
	const state = {
		api: {
			runtime: {
				bootstrap: client.bootstrap,
				login: client.login,
			},
			desk: {
				commandHistory: client.commandHistory,
				configuration: client.configuration,
				deskLock: client.deskLock,
				programmers: client.programmers,
			},
			programming: {
				programmingInteractionSnapshot: client.programmingInteractionSnapshot,
			},
			fixtures: {
				fixtureLibrary: client.fixtureLibrary,
				fixtureProfiles: client.fixtureProfiles,
				fixtureProfileWarnings: client.fixtureProfileWarnings,
				patch: client.patch,
			},
			mediaOutput: { mediaServers: client.mediaServers },
			playback: {
				screens: client.screens,
				removeClient: client.removeClient,
			},
			shows: { shows: client.shows },
		},
		commandTargetModeRef: { current: "FIXTURE" },
		setBootstrap: vi.fn(),
		setSession: vi.fn(),
		setConnectionGeneration: vi.fn(),
		setCommandHistory: vi.fn(),
		deskLockStore: { install: vi.fn() },
		setShows: vi.fn(),
		setConfiguration: vi.fn(),
		setMatter: vi.fn(),
		setMediaServers: vi.fn(),
		setFixtureLibrary: vi.fn(),
		setFixtureProfiles: vi.fn(),
		setFixtureProfileWarnings: vi.fn(),
		setScreens: vi.fn(),
		setCommandTargetMode: vi.fn(),
		setCommandLineState: vi.fn(),
		setCommandLinePristine: vi.fn(),
		setSelectedFixtures: vi.fn(),
	} as unknown as ServerState;
	return { clientMethods, state, screens, unexpectedLegacyPlaybackRead };
}

beforeEach(() => {
	const values = new Map<string, string>();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value),
		removeItem: (key: string) => values.delete(key),
		clear: () => values.clear(),
	});
});

afterEach(() => vi.unstubAllGlobals());

describe("connection bootstrap resources", () => {
	it("removes only eligible disconnected clients by default and can be disabled", async () => {
		const harness = createHarness();
		const original = {
			...bootstrap(),
			clients: [
				{
					client_id: "current-client",
					connected: true,
					can_remove: false,
					desk: { id: "desk-current" },
				},
				{
					client_id: "connected-client",
					connected: true,
					can_remove: false,
					desk: { id: "desk-connected" },
				},
				{
					client_id: "retained-client",
					connected: false,
					can_remove: false,
					desk: { id: "desk-retained" },
				},
				{
					client_id: "old-client",
					connected: false,
					can_remove: true,
					desk: { id: "desk-old" },
				},
			],
		} as unknown as BootstrapSnapshot;
		const refreshed = { ...original, clients: [] } as BootstrapSnapshot;
		harness.clientMethods.bootstrap.mockResolvedValue(refreshed);

		await expect(
			removeDisconnectedOtherClients(
				harness.state.api,
				original,
				"current-client",
			),
		).resolves.toBe(refreshed);
		expect(harness.clientMethods.removeClient).toHaveBeenCalledOnce();
		expect(harness.clientMethods.removeClient).toHaveBeenCalledWith(
			"desk-old",
			"old-client",
		);

		localStorage.setItem(SINGLE_CLIENT_MODE_STORAGE_KEY, "false");
		harness.clientMethods.removeClient.mockClear();
		harness.clientMethods.bootstrap.mockClear();
		await expect(
			removeDisconnectedOtherClients(
				harness.state.api,
				original,
				"current-client",
			),
		).resolves.toBe(original);
		expect(harness.clientMethods.removeClient).not.toHaveBeenCalled();
		expect(harness.clientMethods.bootstrap).not.toHaveBeenCalled();
	});

	it("loads only interactive resources before making the desk available", async () => {
		const harness = createHarness();
		const loadShowObjects = vi.fn().mockResolvedValue(undefined);

		await bootstrapConnection(
			harness.state,
			loadShowObjects,
			() => false,
			"primary",
		);

		expect(harness.unexpectedLegacyPlaybackRead).not.toHaveBeenCalled();
		expect(harness.clientMethods.patch).not.toHaveBeenCalled();
		expect(harness.clientMethods.programmers).not.toHaveBeenCalled();
		expect(
			harness.clientMethods.programmingInteractionSnapshot,
		).toHaveBeenCalledWith("desk-1");
		expect(harness.clientMethods.commandHistory).toHaveBeenCalledOnce();
		expect(harness.clientMethods.fixtureLibrary).toHaveBeenCalledOnce();
		expect(harness.clientMethods.screens).not.toHaveBeenCalled();
		expect(harness.clientMethods.mediaServers).not.toHaveBeenCalled();
		expect(harness.clientMethods.fixtureProfiles).not.toHaveBeenCalled();
		expect(loadShowObjects).toHaveBeenCalledWith("show-1", "user-1");
	});

	it("defers non-interactive resources into cancellable warm-up tasks", async () => {
		const harness = createHarness();
		const controller = new AbortController();
		const tasks = deferredConnectionWarmupTasks(harness.state);

		expect(tasks.map(({ key }) => key)).toEqual([
			"legacy:shows",
			"legacy:screens",
			"legacy:media-servers",
			"legacy:fixture-profiles",
			"legacy:fixture-profile-warnings",
		]);
		for (const task of tasks) await task.run(controller.signal);

		expect(harness.clientMethods.screens).toHaveBeenCalledOnce();
		expect(harness.state.setScreens).toHaveBeenCalledWith(harness.screens);
		expect(harness.clientMethods.mediaServers).toHaveBeenCalledOnce();
		expect(harness.clientMethods.fixtureProfiles).toHaveBeenCalledOnce();
		expect(harness.clientMethods.fixtureProfileWarnings).toHaveBeenCalledOnce();
		expect(harness.clientMethods.shows).toHaveBeenCalledOnce();
	});
});
