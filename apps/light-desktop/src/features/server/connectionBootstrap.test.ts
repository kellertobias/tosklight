import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	BootstrapSnapshot,
	ScreenSnapshot,
	SessionResponse,
} from "../../api/types";
import {
	bootstrapConnection,
	deferredConnectionWarmupTasks,
} from "./connectionBootstrap";
import type { ServerState } from "./useServerState";

const user = { id: "user-1", name: "Operator", enabled: true };
const session = {
	session_id: "session-1",
	token: "token-1",
	user,
	desk: { id: "desk-1", osc_alias: "main" },
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
			playback: { screens: client.screens },
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
