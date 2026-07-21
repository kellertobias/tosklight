import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RunningCueListSource } from "./systemControls/runningPlaybackAuthority";
import { SystemControlsModal } from "./SystemControlsModal";

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
			current: { id: `${cueListId}-cue-1`, number: playbackNumber == null ? 3 : 1 },
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
					phasers: options.dynamic ? [{}] : [],
				} as RunningCueListSource["cue"],
			],
		} as RunningCueListSource["cueList"],
		cue: {
			id: `${cueListId}-cue-1`,
			number: playbackNumber == null ? 3 : 1,
			phasers: options.dynamic ? [{}] : [],
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
let legacyReads = 0;
const server = {
	readVisualization: vi
		.fn()
		.mockResolvedValue({ grand_master: 1, blackout: false }),
	setMaster: vi.fn(),
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
		throw new Error("System Controls must not read the legacy Playback snapshot");
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
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: appState, dispatch }),
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

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	legacyReads = 0;
	authorityCalls.length = 0;
	preloadLifecycleCalls.length = 0;
	preloadLifecycle.ready = true;
	preloadLifecycle.active = false;
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

		expect(screen.getByText("Main playback")).toBeInTheDocument();
		expect(screen.getByText("Virtual Cuelist")).toBeInTheDocument();
		expect(screen.getByText("Operator · Current user")).toBeInTheDocument();
		expect(screen.getByText("Main Cuelist · Dynamic 1")).toBeInTheDocument();
		expect(
			screen.getByText("1 fixtures · 3 values · 1 session · Connected"),
		).toBeInTheDocument();
		expect(legacyReads).toBe(0);
	});

	it("releases the exact source selected by each control", () => {
		render(<SystemControlsModal />);

		fireEvent.click(
			screen.getByRole("button", { name: "Stop Playback Main playback" }),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Stop Virtual playback Virtual Cuelist",
			}),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Stop Dynamic 1 from Main Cuelist" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Clear programmer operator" }),
		);

		expect(release).toHaveBeenNthCalledWith(1, mapped);
		expect(release).toHaveBeenNthCalledWith(2, direct);
		expect(release).toHaveBeenNthCalledWith(3, mapped);
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

		expect(screen.getByText("Programmers loading…")).toBeInTheDocument();
		expect(screen.queryByText(/2 values/)).not.toBeInTheDocument();
		expect(legacyReads).toBe(0);
	});

	it("stops every distinct source, each Programmer, and Preload in one action", async () => {
		playbackAuthority.sources = [mapped, mapped, direct];
		render(<SystemControlsModal />);
		fireEvent.click(screen.getByRole("button", { name: "Stop everything" }));

		await waitFor(() => expect(release).toHaveBeenCalledTimes(2));
		expect(release).toHaveBeenCalledWith(mapped);
		expect(release).toHaveBeenCalledWith(direct);
		expect(clearProgrammer).toHaveBeenCalledWith("session-1");
		expect(preloadLifecycle.actions.release).toHaveBeenCalledOnce();
		expect(dispatch).not.toHaveBeenCalledWith({ type: "RELEASE_PRELOAD" });
	});

	it("refuses Stop everything while Playback authority is loading", () => {
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

		expect(screen.getByText("Playbacks loading…")).toBeInTheDocument();
		expect(screen.getByText("Virtual playbacks loading…")).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Stop everything" })).toBeDisabled();
		expect(release).not.toHaveBeenCalled();
		expect(clearProgrammer).not.toHaveBeenCalled();
		expect(preloadLifecycle.actions.release).not.toHaveBeenCalled();
	});

	it("keeps scoped authority dormant while the modal is closed", () => {
		appState.systemControlsOpen = false;
		render(<SystemControlsModal />);

		expect(authorityCalls).toEqual([false]);
		expect(preloadLifecycleCalls).toEqual([false]);
		expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
		expect(server.readVisualization).not.toHaveBeenCalled();
	});
});
