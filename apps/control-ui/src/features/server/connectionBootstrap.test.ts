import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	BootstrapSnapshot,
	ScreenSnapshot,
	SessionResponse,
} from "../../api/types";
import { bootstrapConnection } from "./connectionBootstrap";
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
	const screens = { screens: [], active_pages: {} } as ScreenSnapshot;
	const unexpectedLegacyPlaybackRead = vi.fn();
	const clientMethods = {
		bootstrap: vi.fn().mockResolvedValue(initial),
		login: vi.fn().mockResolvedValue(session),
		deskLock: vi.fn().mockResolvedValue({ locked: false }),
		patch: vi.fn().mockResolvedValue({ revision: 1, fixtures: [], routes: [] }),
		programmers: vi.fn().mockResolvedValue([]),
		shows: vi.fn().mockResolvedValue([]),
		configuration: vi
			.fn()
			.mockResolvedValue({ configuration: {}, matter: {} }),
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
		client,
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
	it("loads retained resources without the broad Playback snapshot", async () => {
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
		expect(harness.clientMethods.programmers).toHaveBeenCalledOnce();
		expect(harness.clientMethods.screens).toHaveBeenCalledOnce();
		expect(harness.state.setScreens).toHaveBeenCalledWith(harness.screens);
		expect(loadShowObjects).toHaveBeenCalledWith("show-1", "user-1");
	});
});
