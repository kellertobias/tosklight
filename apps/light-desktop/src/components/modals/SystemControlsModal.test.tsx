import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SystemControlsModal } from "./SystemControlsModal";
import type { RunningCueListSource } from "./systemControls/runningPlaybackAuthority";

const dispatch = vi.fn();
const clearProgrammer = vi.fn().mockResolvedValue(undefined);
const release = vi.fn().mockResolvedValue(null);
const authorityCalls: boolean[] = [];
const appState = { systemControlsOpen: true };
const lifecycle = vi.hoisted(() => ({
	projection: {
		revision: 1,
		programmers: [
			{
				programmerId: "programmer-1",
				userId: "operator",
				connected: true,
				selectedFixtureCount: 1,
				normalValueCount: 3,
				sessions: [{ sessionId: "session-1" }],
			},
		],
	} as ProgrammerProjection | null,
}));
const dynamicsRuntime = vi.hoisted(() => ({
	off: vi.fn().mockResolvedValue(true),
	authority: {
		ready: true,
		loading: false,
		error: null,
		rows: [
			{
				key: "dynamic-instance:dynamic-controller",
				instanceId: "dynamic-instance",
				dynamicId: "dynamic-7",
				poolNumber: 7,
				name: "Circle",
				targets: ["fixture-1"],
				pending: false,
				instancePaused: false,
				speedSource: "Speed Group A",
				controllerId: "dynamic-controller",
				source: "Playback 12",
				priority: 1,
				size: 1,
				speedMultiplier: 1,
				phaseOffsetDegrees: 0,
				paused: false,
				winning: true,
				releasing: false,
				activationMix: 1,
			},
		],
		stoppingControllerIds: new Set<string>(),
		canStop: true,
		off: vi.fn(),
	},
}));
dynamicsRuntime.authority.off = dynamicsRuntime.off;

interface ProgrammerProjection {
	revision: number;
	programmers: Array<{
		programmerId: string;
		userId: string;
		connected: boolean;
		selectedFixtureCount: number;
		normalValueCount: number;
		sessions: Array<{ sessionId: string }>;
	}>;
}

function runningSource(
	playbackNumber: number | null,
	cueListId: string,
	label: string,
	options: { paused?: boolean; master?: number; dynamic?: boolean } = {},
): RunningCueListSource {
	const identity =
		playbackNumber == null
			? ({ kind: "cue_list", cue_list_id: cueListId } as const)
			: ({ kind: "playback", playback_number: playbackNumber } as const);
	return {
		key:
			identity.kind === "cue_list"
				? `cuelist:${cueListId}`
				: `playback:${playbackNumber}`,
		identity,
		cueListId,
		playbackNumber,
		label,
		runtime: {
			cue_index: 0,
			current: {
				id: `${cueListId}-cue-1`,
				number: playbackNumber == null ? 3 : 1,
			},
			master: options.master ?? 1,
			paused: options.paused ?? false,
		} as RunningCueListSource["runtime"],
		cueList: {
			id: cueListId,
			name: playbackNumber == null ? "Virtual Cuelist" : "Main Cuelist",
			cues: [
				{
					id: `${cueListId}-cue-1`,
					number: playbackNumber == null ? 3 : 1,
				} as RunningCueListSource["cue"],
			],
		} as RunningCueListSource["cueList"],
		cue: {
			id: `${cueListId}-cue-1`,
			number: playbackNumber == null ? 3 : 1,
		} as RunningCueListSource["cue"],
	};
}

const mapped = runningSource(12, "cue-list-1", "Main playback", {
	master: 0.75,
	dynamic: true,
});
const direct = runningSource(null, "cue-list-2", "Virtual Cuelist", {
	paused: true,
});
const playbackAuthority = {
	ready: true,
	loading: false,
	canRelease: true,
	sources: [mapped, direct] as readonly RunningCueListSource[],
	mappedSources: [mapped] as readonly RunningCueListSource[],
	virtualSources: [direct] as readonly RunningCueListSource[],
	dynamics: [{ source: mapped, index: 0 }],
	release,
};
const outputAuthority = vi.hoisted(() => ({
	viewCalls: [] as boolean[],
	actionCalls: [] as boolean[],
	view: {
		projection: {
			showId: "show-a",
			identity: "global_master" as const,
			revision: 1,
			grandMaster: 1,
			blackout: false,
		},
		status: "ready" as const,
		error: null,
		repairRequired: false,
		pending: false,
		ready: true,
	},
	actions: { setOutput: vi.fn().mockResolvedValue(null) } as {
		setOutput: ReturnType<typeof vi.fn>;
	} | null,
}));
let legacyReads = 0;
const server = {
	setProgrammer: vi.fn(),
	selectedFixtures: [],
	patch: { fixtures: [] },
	session: { user: { id: "operator", name: "Operator" } },
	get bootstrap() {
		legacyReads += 1;
		throw new Error("System Controls must not read bootstrap Programmers");
	},
	get playbacks() {
		legacyReads += 1;
		throw new Error(
			"System Controls must not read the legacy Playback snapshot",
		);
	},
	clearProgrammer,
	controlFixtureAction: vi.fn().mockResolvedValue(undefined),
};
const preloadLifecycleCalls: boolean[] = [];
const preloadLifecycle = {
	ready: true,
	armed: false,
	active: false,
	pending: false,
	phase: "idle" as const,
	error: null,
	actions: {
		enter: vi.fn().mockResolvedValue(null),
		go: vi.fn().mockResolvedValue(null),
		clearPending: vi.fn().mockResolvedValue(null),
		release: vi.fn().mockResolvedValue(null),
	},
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useSessionSnapshot: () => server.session,
}));
vi.mock(
	"../../features/programmerActions/ProgrammerActionsContext",
	async (importOriginal) => ({
		...(await importOriginal<object>()),
		useProgrammerActions: () => ({
			clearProgrammer: server.clearProgrammer,
			controlFixtureAction: server.controlFixtureAction,
		}),
	}),
);
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: appState, dispatch }),
}));
vi.mock("../../features/outputRuntime/OutputRuntimeView", () => ({
	useOutputRuntimeView: (enabled = true) => {
		outputAuthority.viewCalls.push(enabled);
		return outputAuthority.view;
	},
	useOutputRuntimeActions: (enabled = true) => {
		outputAuthority.actionCalls.push(enabled);
		return enabled ? outputAuthority.actions : null;
	},
}));
vi.mock("../../features/programmerLifecycle/ProgrammerLifecycleView", () => ({
	useProgrammerLifecycleView: () => lifecycle.projection,
}));
vi.mock(
	"../../features/programmerPreloadLifecycle/ProgrammerPreloadLifecycleView",
	() => ({
		useProgrammerPreloadLifecycleView: (enabled = true) => {
			preloadLifecycleCalls.push(enabled);
			return preloadLifecycle;
		},
	}),
);
vi.mock("./systemControls/runningPlaybackAuthority", () => ({
	useRunningPlaybackAuthority: (enabled: boolean) => {
		authorityCalls.push(enabled);
		return playbackAuthority;
	},
}));
vi.mock("./systemControls/runningDynamicsAuthority", () => ({
	useRunningDynamicsAuthority: () => dynamicsRuntime.authority,
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	legacyReads = 0;
	authorityCalls.length = 0;
	preloadLifecycleCalls.length = 0;
	outputAuthority.viewCalls.length = 0;
	outputAuthority.actionCalls.length = 0;
	outputAuthority.actions = { setOutput: vi.fn().mockResolvedValue(null) };
	Object.assign(outputAuthority.view, {
		projection: {
			showId: "show-a",
			identity: "global_master",
			revision: 1,
			grandMaster: 1,
			blackout: false,
		},
		status: "ready",
		ready: true,
	});
	preloadLifecycle.ready = true;
	preloadLifecycle.active = false;
	Object.assign(dynamicsRuntime.authority, {
		ready: true,
		loading: false,
		rows: [dynamicsRuntime.authority.rows[0]],
		canStop: true,
	});
	appState.systemControlsOpen = true;
	Object.assign(playbackAuthority, {
		ready: true,
		loading: false,
		canRelease: true,
		sources: [mapped, direct],
		mappedSources: [mapped],
		virtualSources: [direct],
		dynamics: [{ source: mapped, index: 0 }],
	});
	lifecycle.projection = {
		revision: 1,
		programmers: [
			{
				programmerId: "programmer-1",
				userId: "operator",
				connected: true,
				selectedFixtureCount: 1,
				normalValueCount: 3,
				sessions: [{ sessionId: "session-1" }],
			},
		],
	};
});

describe("SystemControlsModal", () => {
	it("shows each scoped running source without reading broad Playback state", () => {
		render(<SystemControlsModal />);

		const titleBar = screen
			.getByRole("heading", {
				name: "Running & Output",
			})
			.closest(".ui-modal-titlebar");
		expect(titleBar).toHaveTextContent("3 active items");
		expect(titleBar).toContainElement(
			screen.getByRole("button", { name: "All Off" }),
		);
		expect(screen.getByRole("button", { name: "All Off" })).toHaveClass(
			"system-controls-all-off",
		);
		expect(titleBar).toContainElement(
			screen.getByRole("button", { name: "Active Programmers (1)" }),
		);
		expect(
			screen.queryByText("Shift + Clear / Shift + Delete"),
		).not.toBeInTheDocument();
		expect(screen.getByText("Main playback")).toBeInTheDocument();
		expect(screen.getByText("Virtual Cuelist")).toBeInTheDocument();
		expect(screen.getByText("Circle · Dynamic 7")).toBeInTheDocument();
		expect(screen.queryByText("Operator · Current user")).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Active Programmers (1)" }),
		);
		expect(screen.getByText("Operator · Current user")).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Close" }),
		).not.toBeInTheDocument();
		expect(
			screen.getByText("1 fixtures · 3 values · 1 session · Connected"),
		).toBeInTheDocument();
		expect(legacyReads).toBe(0);
	});

	it("releases the exact source selected by each control", () => {
		render(<SystemControlsModal />);

		fireEvent.click(
			screen.getByRole("button", { name: "Turn off Playback Main playback" }),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Turn off Virtual playback Virtual Cuelist",
			}),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Turn off Dynamic 7 Circle from Playback 12",
			}),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Active Programmers (1)" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Clear programmer operator" }),
		);

		expect(release).toHaveBeenNthCalledWith(1, mapped);
		expect(release).toHaveBeenNthCalledWith(2, direct);
		expect(dynamicsRuntime.off).toHaveBeenCalledWith(
			dynamicsRuntime.authority.rows[0],
		);
		expect(clearProgrammer).toHaveBeenCalledWith("session-1");
	});

	it("groups same-user desks and uses safe lifecycle counts for foreign users", () => {
		lifecycle.projection?.programmers[0].sessions.push({
			sessionId: "session-2",
		});
		lifecycle.projection?.programmers.push({
			programmerId: "programmer-2",
			userId: "other-user",
			connected: true,
			selectedFixtureCount: 0,
			normalValueCount: 3,
			sessions: [{ sessionId: "session-3" }],
		});

		render(<SystemControlsModal />);
		fireEvent.click(
			screen.getByRole("button", { name: "Active Programmers (2)" }),
		);

		expect(screen.getAllByText(/3 values/)).toHaveLength(2);
		expect(
			screen.getByText("1 fixtures · 3 values · 2 sessions · Connected"),
		).toBeInTheDocument();
		expect(
			screen.getByText("0 fixtures · 3 values · 1 session · Connected"),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Clear programmer other-user" }),
		);
		expect(clearProgrammer).toHaveBeenCalledWith("session-3");
	});

	it("never falls back to stale bootstrap Programmers while loading", () => {
		lifecycle.projection = null;
		render(<SystemControlsModal />);
		fireEvent.click(
			screen.getByRole("button", { name: "Active Programmers (0)" }),
		);

		expect(screen.getByText("Programmers loading…")).toBeInTheDocument();
		expect(screen.queryByText(/2 values/)).not.toBeInTheDocument();
		expect(legacyReads).toBe(0);
	});

	it("turns every distinct running source off without clearing Programmers", async () => {
		playbackAuthority.sources = [mapped, mapped, direct];
		preloadLifecycle.active = true;
		render(<SystemControlsModal />);
		fireEvent.click(screen.getByRole("button", { name: "All Off" }));

		await waitFor(() => expect(release).toHaveBeenCalledTimes(2));
		expect(release).toHaveBeenCalledWith(mapped);
		expect(release).toHaveBeenCalledWith(direct);
		expect(dynamicsRuntime.off).toHaveBeenCalledOnce();
		expect(clearProgrammer).not.toHaveBeenCalled();
		expect(preloadLifecycle.actions.release).toHaveBeenCalledOnce();
		expect(dispatch).not.toHaveBeenCalledWith({ type: "RELEASE_PRELOAD" });
	});

	it("refuses All Off while Playback authority is loading", () => {
		Object.assign(playbackAuthority, {
			ready: false,
			loading: true,
			canRelease: false,
			sources: [],
			mappedSources: [],
			virtualSources: [],
			dynamics: [],
		});
		render(<SystemControlsModal />);

		expect(screen.getByText("Running Playbacks loading…")).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "All Off" }),
		).toBeDisabled();
		expect(release).not.toHaveBeenCalled();
		expect(clearProgrammer).not.toHaveBeenCalled();
		expect(preloadLifecycle.actions.release).not.toHaveBeenCalled();
	});

	it("routes Grand Master and blackout only through scoped Output actions", async () => {
		render(<SystemControlsModal />);

		fireEvent.input(screen.getByRole("slider", { name: "Grand Master" }), {
			target: { value: "42" },
		});
		await waitFor(() =>
			expect(outputAuthority.actions?.setOutput).toHaveBeenCalledWith({
				grandMaster: 0.42,
			}),
		);
		fireEvent.click(screen.getByRole("button", { name: "BLACKOUT" }));

		expect(outputAuthority.actions?.setOutput).toHaveBeenNthCalledWith(1, {
			grandMaster: 0.42,
		});
		expect(outputAuthority.actions?.setOutput).toHaveBeenNthCalledWith(2, {
			blackout: true,
		});
	});

	it("refuses Output mutations while projection or writer authority is loading", () => {
		outputAuthority.view.ready = false;
		outputAuthority.actions = null;
		render(<SystemControlsModal />);

		const slider = screen.getByRole("slider", { name: "Grand Master" });
		const blackout = screen.getByRole("button", { name: "BLACKOUT" });
		expect(slider).toBeDisabled();
		expect(blackout).toBeDisabled();
		fireEvent.input(slider, { target: { value: "42" } });
		fireEvent.click(blackout);
		expect(outputAuthority.actions).toBeNull();
	});

	it("keeps scoped authority dormant while the modal is closed", () => {
		appState.systemControlsOpen = false;
		render(<SystemControlsModal />);

		expect(authorityCalls).toEqual([false]);
		expect(preloadLifecycleCalls).toEqual([false]);
		expect(outputAuthority.viewCalls).toEqual([false]);
		expect(outputAuthority.actionCalls).toEqual([false]);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
	});
});
