import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition, PlaybackPage } from "../../../api/types";
import { useCommandLineShortcuts } from "./useCommandLineShortcuts";

const appState = { regularNumberShortcuts: true, builtIn: "playback" };

let deskPage: number | null = 2;
let runtimeStatus: "ready" | "loading" | "error" = "ready";
let collectionsReady = true;
let pages: PlaybackPage[] = [];
let playbacks: PlaybackDefinition[] = [];

const kindsViewCalls: { kinds: readonly string[]; enabled: boolean }[] = [];
const deskViewCalls: boolean[] = [];

/** Resolvers for every in-flight runtime action, so ordering stays observable. */
let inFlight: (() => void)[] = [];
const poolPlaybackAction = vi.fn(
	() =>
		new Promise((resolve) => {
			inFlight.push(() => resolve({}));
		}),
);
const setActivePage = vi.fn(async () => true);
let runtimeActions: Record<string, unknown> | null = {
	poolPlaybackAction,
	setActivePage,
};

const createPage = vi.fn(async () => ({}));
const topologyActions = { createPage, error: null };

vi.mock("../../../state/AppContext", () => ({
	useApp: () => ({ state: appState, dispatch: vi.fn() }),
}));

// The shortcut path must never reach the broad Playback facade again.
vi.mock("../../../api/ServerContext", () => ({
	useServer: () => {
		throw new Error("shortcuts must not consult the broad Playback facade");
	},
}));

vi.mock("../../../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackRuntimeActions: () => runtimeActions,
	usePlaybackRuntimeStatus: () => ({ status: runtimeStatus, error: null }),
	usePlaybackDeskView: (enabled: boolean) => {
		deskViewCalls.push(enabled);
		return enabled && deskPage != null ? { active_page: deskPage } : null;
	},
}));

vi.mock("../../../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => topologyActions,
}));

vi.mock("../../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectKindsView: (kinds: readonly string[], enabled: boolean) => {
		kindsViewCalls.push({ kinds, enabled });
	},
}));

vi.mock("../../../features/showObjects/ShowObjectsState", () => ({
	useShowObjectCollectionsReady: (_kinds: unknown, enabled: boolean) =>
		enabled && collectionsReady,
	usePlaybackPages: (enabled: boolean) => (enabled ? pages.map(asObject) : []),
	usePlaybackDefinitions: (enabled: boolean) =>
		enabled ? playbacks.map(asObject) : [],
	useCueLists: () => [],
	useShowObjectsStatus: () => ({ status: "ready", error: null }),
}));

function asObject<T>(body: T) {
	return { kind: "x", id: "x", revision: 1, updated_at: "", body };
}

function playback(
	number: number,
	button: PlaybackDefinition["buttons"][number],
): PlaybackDefinition {
	return {
		number,
		name: `PB ${number}`,
		buttons: [button, "none", "none"],
	} as unknown as PlaybackDefinition;
}

function page(number: number, slots: Record<string, number>): PlaybackPage {
	return { number, name: `Page ${number}`, slots };
}

const callbacks = {
	completed: false,
	commandLine: "",
	commandTargetMode: "FIXTURE" as const,
	commandLinePristine: true,
	persistentError: null,
	replaceCommand: vi.fn(),
	execute: vi.fn(async () => undefined),
	armUpdateOrMenu: vi.fn(),
	dismissPersistentError: vi.fn(),
};

function mount(hardware = false) {
	return renderHook(
		({ hardware: mode }: { hardware: boolean }) =>
			useCommandLineShortcuts(mode, callbacks),
		{ initialProps: { hardware } },
	);
}

function press(code: string, extra: KeyboardEventInit = {}) {
	const event = new KeyboardEvent("keydown", {
		key: code,
		code,
		cancelable: true,
		...extra,
	});
	act(() => {
		window.dispatchEvent(event);
	});
	return event;
}

function release(code: string) {
	act(() => {
		window.dispatchEvent(new KeyboardEvent("keyup", { key: code, code }));
	});
}

/** Lets the serialized send queue advance without settling in-flight sends. */
async function tick() {
	await act(async () => {
		for (let step = 0; step < 4; step += 1) await Promise.resolve();
	});
}

/** Settles every in-flight runtime action so a queued release can be sent. */
async function flush() {
	for (const resolve of inFlight.splice(0)) resolve();
	await tick();
}

beforeEach(() => {
	appState.regularNumberShortcuts = true;
	deskPage = 2;
	runtimeStatus = "ready";
	collectionsReady = true;
	pages = [page(1, {}), page(2, { "1": 10, "8": 88 }), page(3, {})];
	playbacks = [playback(10, "go"), playback(88, "flash")];
	runtimeActions = { poolPlaybackAction, setActivePage };
	topologyActions.createPage = createPage;
	inFlight = [];
	kindsViewCalls.length = 0;
	deskViewCalls.length = 0;
	vi.clearAllMocks();
});

// A leaked listener would preventDefault ahead of the next test's mount.
afterEach(cleanup);

describe("useCommandLineShortcuts playback keys", () => {
	it("hydrates only Page and Playback definitions plus the desk projection", () => {
		mount();

		expect(kindsViewCalls[0]).toEqual({
			kinds: ["playback_page", "playback"],
			enabled: true,
		});
		expect(deskViewCalls).toContain(true);
	});

	it("resolves F1-F8 against the authoritative current Page", () => {
		mount();

		press("F1");

		expect(poolPlaybackAction).toHaveBeenCalledWith(10, "go", {
			surface: "physical",
		});
	});

	it("maps a configured underscore action to its wire action", () => {
		playbacks = [playback(10, "go_minus"), playback(88, "flash")];
		mount();

		press("F1");

		expect(poolPlaybackAction).toHaveBeenCalledWith(10, "go-minus", {
			surface: "physical",
		});
	});

	it("does not repeat a one-shot Playback action while a key is held", () => {
		mount();

		press("F1");
		press("F1", { repeat: true });

		expect(poolPlaybackAction).toHaveBeenCalledOnce();
	});

	it("ignores an unmapped slot and a configured action of none", () => {
		playbacks = [playback(10, "none"), playback(88, "flash")];
		mount();

		press("F1");
		press("F5");

		expect(poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("serializes a Flash release after its press and ignores repeats", async () => {
		mount();

		press("F8");
		press("F8", { repeat: true });
		await tick();
		expect(poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(poolPlaybackAction).toHaveBeenCalledWith(88, "flash", {
			pressed: true,
			surface: "physical",
		});

		release("F8");
		await tick();
		// The release must not overtake the still-pending press.
		expect(poolPlaybackAction).toHaveBeenCalledTimes(1);

		await flush();

		expect(poolPlaybackAction).toHaveBeenNthCalledWith(2, 88, "flash", {
			pressed: false,
			surface: "physical",
		});
	});

	it("treats Swap as a held action and serializes its release", async () => {
		playbacks = [playback(10, "swap"), playback(88, "flash")];
		mount();

		press("F1");
		await tick();
		expect(poolPlaybackAction).toHaveBeenCalledWith(10, "swap", {
			pressed: true,
			surface: "physical",
		});

		release("F1");
		await tick();
		expect(poolPlaybackAction).toHaveBeenCalledOnce();

		await flush();
		expect(poolPlaybackAction).toHaveBeenNthCalledWith(2, 10, "swap", {
			pressed: false,
			surface: "physical",
		});
	});

	it("retains the original Playback number when topology moves while held", async () => {
		const view = mount();

		press("F8");
		await tick();
		pages = [page(2, { "8": 99 })];
		playbacks = [playback(99, "flash")];
		act(() => {
			view.rerender({ hardware: false });
		});
		release("F8");
		await flush();

		expect(poolPlaybackAction).toHaveBeenNthCalledWith(2, 88, "flash", {
			pressed: false,
			surface: "physical",
		});
	});

	it("releases a held Flash when the runtime authority is replaced", async () => {
		const view = mount();

		press("F8");
		await tick();
		runtimeActions = { poolPlaybackAction, setActivePage };
		act(() => {
			view.rerender({ hardware: false });
		});
		await flush();

		expect(poolPlaybackAction).toHaveBeenNthCalledWith(2, 88, "flash", {
			pressed: false,
			surface: "physical",
		});
	});

	const teardowns: [string, (view: ReturnType<typeof mount>) => void][] = [
		["window blur", () => window.dispatchEvent(new Event("blur"))],
		["listener teardown", (view) => view.unmount()],
		["hardware mode taking the keys", (view) => view.rerender({ hardware: true })],
		[
			"shortcuts being disabled",
			(view) => {
				appState.regularNumberShortcuts = false;
				view.rerender({ hardware: false });
			},
		],
	];

	for (const [name, teardown] of teardowns)
		it(`releases a held Flash on ${name}`, async () => {
			const view = mount();

			press("F8");
			await tick();
			act(() => teardown(view));
			await flush();

			expect(poolPlaybackAction).toHaveBeenNthCalledWith(2, 88, "flash", {
				pressed: false,
				surface: "physical",
			});
		});
});

describe("useCommandLineShortcuts page keys", () => {
	it("steps to an existing Page through the scoped desk action", async () => {
		mount();

		press("PageUp");
		await tick();
		expect(setActivePage).toHaveBeenCalledWith(3);

		press("PageDown");
		await tick();
		expect(setActivePage).toHaveBeenCalledWith(1);
	});

	it("creates the next Page only after the last nonempty Page", async () => {
		pages = [page(1, {}), page(2, { "1": 10 })];
		deskPage = 2;
		mount();

		press("PageUp");
		await tick();

		expect(createPage).toHaveBeenCalledWith(3);
		expect(setActivePage).toHaveBeenCalledWith(3);
	});

	it("serializes rapid and repeated Page creation keys", async () => {
		pages = [page(1, {}), page(2, { "1": 10 })];
		deskPage = 2;
		let resolveCreate: (value: object) => void = () => undefined;
		topologyActions.createPage = vi.fn(
			() =>
				new Promise((resolve) => {
					resolveCreate = resolve;
				}),
		);
		mount();

		press("PageUp");
		press("PageUp");
		press("PageUp", { repeat: true });
		await tick();

		expect(topologyActions.createPage).toHaveBeenCalledOnce();
		resolveCreate({});
		await tick();
		expect(setActivePage).toHaveBeenCalledOnce();
		expect(setActivePage).toHaveBeenCalledWith(3);
	});

	it("does not select a Page created by a replaced topology writer", async () => {
		pages = [page(1, {}), page(2, { "1": 10 })];
		deskPage = 2;
		let resolveCreate: (value: object) => void = () => undefined;
		topologyActions.createPage = vi.fn(
			() =>
				new Promise((resolve) => {
					resolveCreate = resolve;
				}),
		);
		const view = mount();
		press("PageUp");
		await tick();

		topologyActions.createPage = vi.fn(async () => ({}));
		act(() => view.rerender({ hardware: false }));
		resolveCreate({});
		await tick();

		expect(setActivePage).not.toHaveBeenCalled();
	});

	it("does not select a Page created for a replaced runtime writer", async () => {
		pages = [page(1, {}), page(2, { "1": 10 })];
		deskPage = 2;
		let resolveCreate: (value: object) => void = () => undefined;
		topologyActions.createPage = vi.fn(
			() =>
				new Promise((resolve) => {
					resolveCreate = resolve;
				}),
		);
		const view = mount();
		press("PageUp");
		await tick();

		const replacementSetActivePage = vi.fn(async () => true);
		runtimeActions = { poolPlaybackAction, setActivePage: replacementSetActivePage };
		act(() => view.rerender({ hardware: false }));
		resolveCreate({});
		await tick();

		expect(setActivePage).not.toHaveBeenCalled();
		expect(replacementSetActivePage).not.toHaveBeenCalled();
	});

	it("does not continue Page creation after shortcut teardown", async () => {
		pages = [page(1, {}), page(2, { "1": 10 })];
		deskPage = 2;
		let resolveCreate: (value: object) => void = () => undefined;
		topologyActions.createPage = vi.fn(
			() =>
				new Promise((resolve) => {
					resolveCreate = resolve;
				}),
		);
		const view = mount();
		press("PageUp");
		await tick();

		view.unmount();
		resolveCreate({});
		await tick();

		expect(setActivePage).not.toHaveBeenCalled();
	});

	it("refuses to create a Page after an empty last Page", async () => {
		pages = [page(1, { "1": 10 }), page(2, {})];
		deskPage = 2;
		mount();

		press("PageUp");
		await tick();

		expect(createPage).not.toHaveBeenCalled();
		expect(setActivePage).not.toHaveBeenCalled();
	});

	it("refuses to step below Page 1", () => {
		deskPage = 1;
		mount();

		press("PageDown");

		expect(setActivePage).not.toHaveBeenCalled();
		expect(createPage).not.toHaveBeenCalled();
	});
});

describe("useCommandLineShortcuts authority gating", () => {
	it("consumes keys but sends nothing while the desk is loading", () => {
		deskPage = null;
		mount();

		const playbackKey = press("F1");
		const pageKey = press("PageUp");

		expect(playbackKey.defaultPrevented).toBe(true);
		expect(pageKey.defaultPrevented).toBe(true);
		expect(poolPlaybackAction).not.toHaveBeenCalled();
		expect(setActivePage).not.toHaveBeenCalled();
		expect(createPage).not.toHaveBeenCalled();
	});

	it("consumes keys but ignores a retained desk while runtime repairs", () => {
		runtimeStatus = "loading";
		deskPage = 2;
		mount();

		expect(press("F1").defaultPrevented).toBe(true);
		expect(press("PageUp").defaultPrevented).toBe(true);
		expect(poolPlaybackAction).not.toHaveBeenCalled();
		expect(setActivePage).not.toHaveBeenCalled();
		expect(createPage).not.toHaveBeenCalled();
	});

	it("consumes keys but sends nothing while definitions are loading", () => {
		collectionsReady = false;
		mount();

		expect(press("F1").defaultPrevented).toBe(true);
		expect(press("PageUp").defaultPrevented).toBe(true);
		expect(poolPlaybackAction).not.toHaveBeenCalled();
		expect(setActivePage).not.toHaveBeenCalled();
		expect(createPage).not.toHaveBeenCalled();
	});

	it("sends nothing when the runtime authority is absent", () => {
		runtimeActions = null;
		mount();

		expect(press("F1").defaultPrevented).toBe(true);
		expect(press("PageUp").defaultPrevented).toBe(true);
		expect(poolPlaybackAction).not.toHaveBeenCalled();
	});
});

describe("useCommandLineShortcuts dormancy", () => {
	it("opens no subscription and consumes no keys in hardware mode", () => {
		mount(true);

		expect(kindsViewCalls.every((call) => !call.enabled)).toBe(true);
		expect(deskViewCalls.every((enabled) => !enabled)).toBe(true);
		expect(press("F1").defaultPrevented).toBe(false);
		expect(poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("opens no subscription while shortcuts are disabled", () => {
		appState.regularNumberShortcuts = false;
		mount();

		expect(kindsViewCalls.every((call) => !call.enabled)).toBe(true);
		expect(deskViewCalls.every((enabled) => !enabled)).toBe(true);
		expect(press("F1").defaultPrevented).toBe(false);
		expect(setActivePage).not.toHaveBeenCalled();
	});
});
