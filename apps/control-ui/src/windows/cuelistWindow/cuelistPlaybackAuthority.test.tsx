import { act, cleanup, render, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CueList, PlaybackDefinition } from "../../api/types";
import type {
	PlaybackDesk,
	PlaybackIdentity,
	PlaybackProjection,
	PlaybackSnapshot,
} from "../../features/playbackRuntime/contracts";
import { identityKey } from "../../features/playbackRuntime/contracts";
import { PlaybackRuntimeViewProvider } from "../../features/playbackRuntime/PlaybackRuntimeView";
import { PlaybackRuntimeStore } from "../../features/playbackRuntime/store";
import {
	CUE_LIST_ID,
	cueProjection,
	DESK_ID,
	deskProjection,
	SHOW_ID,
} from "../../features/playbackRuntime/testFixtures";
import type {
	PlaybackEventObserver,
	PlaybackEventScope,
	PlaybackEventTransport,
} from "../../features/playbackRuntime/transport";
import { CuelistWindow } from "../CuelistWindow";

/**
 * These tests hold the Cuelist window against v2 Playback authority only. The
 * legacy `server.playbacks` snapshot stays mocked and deliberately contradicts
 * the scoped projections, so any surviving fallback read fails a test instead of
 * quietly reappearing.
 */

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	recordCue: vi.fn(),
	saveTopologyCueList: vi.fn(),
	executeCommandLine: vi.fn(),
	setCommandLine: vi.fn(),
	refresh: vi.fn(),
	resetCommandLine: vi.fn(),
	showObjectView: vi.fn(),
	showObjectKindsView: vi.fn(),
	state: {
		activeDeskId: "desk-1",
		paneSettingsId: null as string | null,
		presetFamily: "Mixed" as const,
		storeArmed: false,
		updateArmed: false,
		cueListSetArmed: false,
		cueListSetTarget: null as number | null,
		cuelistBuiltInView: "pool" as "pool" | "cues",
		cuelistBuiltInNumber: null as number | null,
		desks: [],
	},
	/** Contradictory legacy authority. Nothing under test may read it. */
	legacyPlaybacks: {
		pool: [] as PlaybackDefinition[],
		active: [] as Array<Record<string, unknown>>,
		pages: [] as Array<Record<string, unknown>>,
		cue_lists: [] as CueList[],
		active_page: 1,
		selected_playback: null as number | null,
	},
	pool: [] as PlaybackDefinition[],
	cueLists: [] as CueList[],
	pages: [] as Array<Record<string, unknown>>,
}));

vi.mock("../../api/ServerContext", () => ({
	useServer: () => ({
		playbacks: mocks.legacyPlaybacks,
		configuration: { speed_groups_bpm: [120, 90, 60, 30, 15] },
		patch: { fixtures: [], revision: 0 },
		stageLayout: null,
		groups: [],
		readVisualization: vi.fn(),
		executeCommandLine: mocks.executeCommandLine,
		setCommandLine: mocks.setCommandLine,
		refresh: mocks.refresh,
		resetCommandLine: mocks.resetCommandLine,
		cueObjects: [],
	}),
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => ({
		saveCueList: mocks.saveTopologyCueList,
	}),
}));
vi.mock("../../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	useCueLists: () =>
		mocks.cueLists.map((body) => ({
			kind: "cue_list",
			id: body.id,
			revision: 1,
			updated_at: "",
			body,
		})),
	usePlaybackDefinitions: () =>
		mocks.pool.map((body) => ({
			kind: "playback",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body,
		})),
	usePlaybackPages: () =>
		mocks.pages.map((body) => ({
			kind: "playback_page",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body,
		})),
}));
vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: (kind: string, enabled?: boolean, objectId?: string) =>
		mocks.showObjectView(kind, enabled ?? true, objectId),
	useShowObjectKindsView: (kinds: readonly string[], enabled?: boolean) =>
		mocks.showObjectKindsView([...kinds], enabled ?? true),
}));
vi.mock("../../features/server/useShowObjectsState", () => ({
	useGroups: () => [],
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../stage3dScene", () => ({
	cueVisualization: vi.fn(),
	migrateStagePosition: vi.fn(),
	renderStageThumbnail: vi.fn(),
}));

const MAIN_CUE_LIST = "main";
const ENCORE_CUE_LIST = "encore";

function cuelistPlayback(number: number, name: string, cueListId: string) {
	return {
		number,
		name,
		target: { type: "cue_list", cue_list_id: cueListId },
		buttons: ["go", "go_minus", "flash"],
		fader: "master",
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
	} satisfies PlaybackDefinition;
}

function cueListBody(id: string, name: string, cueNames: string[]): CueList {
	return {
		id,
		name,
		priority: 10,
		mode: "sequence",
		looped: false,
		cues: cueNames.map((cueName, index) => ({
			id: `${id}-cue-${index + 1}`,
			number: index + 1,
			name: cueName,
			fade_millis: 0,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		})),
	};
}

interface CuelistRuntime {
	playbackNumber: number;
	cueIndex: number;
	master: number;
}

/** Builds a Cuelist-target runtime projection for the requested identity. */
function cueListRuntimeProjection(
	requested: PlaybackIdentity,
	cueListId: string,
	runtime: CuelistRuntime,
): PlaybackProjection {
	const base = cueProjection(runtime.playbackNumber, runtime.cueIndex);
	if (base.target !== "cue_list" || !base.runtime)
		throw new Error("The shared fixture must produce a live Cuelist runtime");
	const live = base.runtime;
	return {
		...base,
		requested,
		cue_list_id: cueListId,
		runtime: { ...live, master: runtime.master },
	};
}

/**
 * The authoritative v2 desk and runtime state this harness serves over the
 * snapshot loader and the event stream.
 */
class PlaybackAuthority {
	sequence = 10;
	selectedPlayback: number | null = null;
	readonly cuelists = new Map<string, CuelistRuntime>();
	readonly snapshotRequests: PlaybackIdentity[][] = [];
	readonly subscriptions: PlaybackEventScope[] = [];
	private observer: PlaybackEventObserver | null = null;
	private pending = false;

	/** Makes every later snapshot load hang, modelling loading desk authority. */
	suspend() {
		this.pending = true;
	}

	desk(): PlaybackDesk {
		return { ...deskProjection(1), selected_playback: this.selectedPlayback };
	}

	cuelistProjection(cueListId: string): PlaybackProjection | null {
		const runtime = this.cuelists.get(cueListId);
		if (!runtime) return null;
		return cueListRuntimeProjection(
			{ kind: "cue_list", cue_list_id: cueListId },
			cueListId,
			runtime,
		);
	}

	playbackProjection(playbackNumber: number): PlaybackProjection | null {
		for (const [cueListId, runtime] of this.cuelists)
			if (runtime.playbackNumber === playbackNumber)
				return cueListRuntimeProjection(
					{ kind: "playback", playback_number: playbackNumber },
					cueListId,
					runtime,
				);
		return null;
	}

	loadSnapshot = (
		identities: PlaybackIdentity[],
	): Promise<PlaybackSnapshot> => {
		this.snapshotRequests.push([...identities]);
		if (this.pending) return new Promise<PlaybackSnapshot>(() => undefined);
		const projections = identities
			.map((identity) =>
				identity.kind === "cue_list"
					? this.cuelistProjection(identity.cue_list_id)
					: this.playbackProjection(identity.playback_number),
			)
			.filter((projection): projection is PlaybackProjection => !!projection);
		return Promise.resolve({
			cursor: { sequence: this.sequence },
			desk: this.desk(),
			projections,
		});
	};

	transport: PlaybackEventTransport = {
		subscribe: (
			_deskId: string,
			scope: PlaybackEventScope,
			_afterSequence: number | null,
			observer: PlaybackEventObserver,
		) => {
			this.subscriptions.push({
				identities: [...scope.identities],
				desk: scope.desk,
			});
			this.observer = observer;
			return { close: () => undefined, repair: () => undefined };
		},
	};

	/** Publishes the current desk projection as the authoritative desk event. */
	publishDesk() {
		this.observer?.message({
			type: "event",
			sequence: ++this.sequence,
			payload: { type: "desk", projection: this.desk() },
		});
	}

	publishCuelist(cueListId: string) {
		const projection = this.cuelistProjection(cueListId);
		if (!projection) return;
		this.observer?.message({
			type: "event",
			sequence: ++this.sequence,
			payload: { type: "runtime", projection },
		});
	}

	/** Identities the mounted views actually asked the server about. */
	requestedKeys() {
		return new Set(
			this.snapshotRequests.flat().map((identity) => identityKey(identity)),
		);
	}
}

let authority: PlaybackAuthority;

function renderCuelistWindow(
	element: React.ReactElement,
	options: { authorityKey?: string; store?: PlaybackRuntimeStore } = {},
) {
	const store = options.store ?? new PlaybackRuntimeStore();
	const authorityKey = options.authorityKey ?? "authority-1";
	const wrap = (child: React.ReactElement, key = authorityKey) => (
		<PlaybackRuntimeViewProvider
			showId={SHOW_ID}
			deskId={DESK_ID}
			authorityKey={key}
			store={store}
			transport={authority.transport}
			loadSnapshot={authority.loadSnapshot}
		>
			{child}
		</PlaybackRuntimeViewProvider>
	);
	const view = render(wrap(element));
	return {
		...view,
		store,
		rerenderWith: (child: React.ReactElement, key = authorityKey) =>
			view.rerender(wrap(child, key)),
	};
}

/** Lets the session's queued microtask refresh and snapshot promise settle. */
async function settle() {
	await act(async () => {
		await Promise.resolve();
		await Promise.resolve();
	});
}

beforeEach(() => {
	cleanup();
	vi.clearAllMocks();
	mocks.recordCue.mockResolvedValue({ status: "changed" });
	mocks.resetCommandLine.mockResolvedValue(true);
	mocks.executeCommandLine.mockResolvedValue(true);
	mocks.refresh.mockResolvedValue(undefined);
	mocks.state.storeArmed = false;
	mocks.state.updateArmed = false;
	mocks.state.cueListSetArmed = false;
	mocks.state.cueListSetTarget = null;
	mocks.state.paneSettingsId = null;
	mocks.pool = [
		cuelistPlayback(1, "Main", MAIN_CUE_LIST),
		cuelistPlayback(2, "Encore", ENCORE_CUE_LIST),
	];
	mocks.cueLists = [
		cueListBody(MAIN_CUE_LIST, "Main", ["Main opening", "Main chase step"]),
		cueListBody(ENCORE_CUE_LIST, "Encore", ["Encore look"]),
	];
	mocks.pages = [];
	mocks.legacyPlaybacks.pool = [];
	mocks.legacyPlaybacks.cue_lists = [];
	mocks.legacyPlaybacks.active = [];
	mocks.legacyPlaybacks.pages = [];
	mocks.legacyPlaybacks.selected_playback = null;
	authority = new PlaybackAuthority();
});

function followSelectionPane() {
	return (
		<CuelistWindow compact cueListTab="cues" cueListSource="follow-selection" />
	);
}

describe("Cuelist follow-selection desk authority", () => {
	it("follows only the exact desk projection's selected playback", async () => {
		authority.selectedPlayback = 1;
		const view = renderCuelistWindow(followSelectionPane());
		await settle();
		const ui = within(view.container);

		expect(ui.getByText("Main opening")).toBeInTheDocument();
		expect(ui.queryByText("Encore look")).not.toBeInTheDocument();

		authority.selectedPlayback = 2;
		await act(async () => {
			authority.publishDesk();
			await Promise.resolve();
		});
		await waitFor(() =>
			expect(ui.getByText("Encore look")).toBeInTheDocument(),
		);
		expect(ui.queryByText("Main opening")).not.toBeInTheDocument();

		authority.selectedPlayback = null;
		await act(async () => {
			authority.publishDesk();
			await Promise.resolve();
		});
		await waitFor(() =>
			expect(ui.getByText("No Cuelist selected")).toBeInTheDocument(),
		);
	});

	it("reports no selection while desk authority is still loading", async () => {
		authority.suspend();
		mocks.legacyPlaybacks.selected_playback = 1;
		mocks.legacyPlaybacks.pool = mocks.pool;
		mocks.legacyPlaybacks.cue_lists = mocks.cueLists;
		const view = renderCuelistWindow(followSelectionPane());
		await settle();
		const ui = within(view.container);

		expect(ui.getByText("No Cuelist selected")).toBeInTheDocument();
		expect(ui.queryByText("Main opening")).not.toBeInTheDocument();
	});

	it("ignores a contradictory legacy selected playback once desk authority exists", async () => {
		authority.selectedPlayback = 1;
		const view = renderCuelistWindow(followSelectionPane());
		await settle();
		const ui = within(view.container);
		expect(ui.getByText("Main opening")).toBeInTheDocument();

		mocks.legacyPlaybacks.selected_playback = 2;
		mocks.legacyPlaybacks.pool = mocks.pool;
		mocks.legacyPlaybacks.cue_lists = mocks.cueLists;
		view.rerenderWith(followSelectionPane());
		await settle();

		expect(ui.getByText("Main opening")).toBeInTheDocument();
		expect(ui.queryByText("Encore look")).not.toBeInTheDocument();
	});

	it("drops the previous selection when the desk authority is replaced", async () => {
		authority.selectedPlayback = 1;
		const view = renderCuelistWindow(followSelectionPane());
		await settle();
		const ui = within(view.container);
		expect(ui.getByText("Main opening")).toBeInTheDocument();

		authority.suspend();
		view.rerenderWith(followSelectionPane(), "authority-2");
		await settle();

		expect(ui.getByText("No Cuelist selected")).toBeInTheDocument();
		expect(ui.queryByText("Main opening")).not.toBeInTheDocument();
	});
});

describe("Cuelist runtime display authority", () => {
	it("drives the current Cue from the exact Cuelist runtime projection", async () => {
		authority.selectedPlayback = 1;
		authority.cuelists.set(MAIN_CUE_LIST, {
			playbackNumber: 1,
			cueIndex: 1,
			master: 1,
		});
		const view = renderCuelistWindow(followSelectionPane());
		await settle();
		const ui = within(view.container);

		await waitFor(() =>
			expect(ui.getByText("Main chase step").closest("tr")).toHaveClass(
				"current",
			),
		);
		expect(authority.requestedKeys()).toContain(`cuelist:${MAIN_CUE_LIST}`);
		expect(authority.requestedKeys()).not.toContain(
			`cuelist:${ENCORE_CUE_LIST}`,
		);

		authority.cuelists.set(MAIN_CUE_LIST, {
			playbackNumber: 1,
			cueIndex: 0,
			master: 1,
		});
		await act(async () => {
			authority.publishCuelist(MAIN_CUE_LIST);
			await Promise.resolve();
		});
		await waitFor(() =>
			expect(ui.getByText("Main opening").closest("tr")).toHaveClass("current"),
		);
		expect(ui.getByText("Main chase step").closest("tr")).not.toHaveClass(
			"current",
		);
	});

	it("shows no running Cue when only the legacy active snapshot claims one", async () => {
		authority.selectedPlayback = 1;
		mocks.legacyPlaybacks.active = [
			{
				playback_number: 1,
				cue_list_id: MAIN_CUE_LIST,
				cue_index: 1,
				paused: false,
				master: 1,
				flash: false,
			},
		];
		const view = renderCuelistWindow(followSelectionPane());
		await settle();
		const ui = within(view.container);

		await waitFor(() =>
			expect(ui.getByText("Main opening")).toBeInTheDocument(),
		);
		expect(ui.getByText("Main chase step").closest("tr")).not.toHaveClass(
			"current",
		);
		expect(ui.getByText("Main opening").closest("tr")).not.toHaveClass(
			"current",
		);
	});
});

describe("Cuelist Pool master authority", () => {
	function poolPane(active = true) {
		return <CuelistWindow active={active} compact cueListTab="pool" />;
	}

	it("takes each master only from the scoped projection map", async () => {
		authority.cuelists.set(MAIN_CUE_LIST, {
			playbackNumber: 1,
			cueIndex: 0,
			master: 0.25,
		});
		mocks.legacyPlaybacks.active = [
			{
				playback_number: 1,
				cue_list_id: MAIN_CUE_LIST,
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
			},
			{
				playback_number: 2,
				cue_list_id: ENCORE_CUE_LIST,
				cue_index: 0,
				paused: false,
				master: 0.5,
				flash: false,
			},
		];
		const view = renderCuelistWindow(poolPane());
		await settle();
		const ui = within(view.container);

		const main = ui.getByText("Main").closest("button")!;
		const encore = ui.getByText("Encore").closest("button")!;
		await waitFor(() =>
			expect(within(main).getByText("Cuelist · 25%")).toBeInTheDocument(),
		);
		expect(main).toHaveClass("running");
		// Playback 2 has no scoped projection, so the legacy 50% never appears.
		expect(within(encore).getByText("Cuelist · Off")).toBeInTheDocument();
		expect(encore).not.toHaveClass("running");
	});

	it("preserves pool numbering, labels, and search over scoped authority", async () => {
		const view = renderCuelistWindow(poolPane());
		await settle();
		const ui = within(view.container);

		expect(ui.getAllByText("Empty")).toHaveLength(998);
		expect(ui.getByText("Main").closest("button")).toHaveClass(
			"pool-cell",
			"cuelist-card",
		);
		expect(
			within(ui.getByText("Main").closest("button")!).getByText("1"),
		).toHaveClass("number");
	});

	it("opens no runtime or Show Object subscription for an inactive pane", async () => {
		renderCuelistWindow(poolPane(false));
		await settle();

		expect(authority.snapshotRequests.flat()).toHaveLength(0);
		expect(authority.subscriptions).toHaveLength(0);
		for (const call of mocks.showObjectKindsView.mock.calls)
			expect(call[1]).toBe(false);
		for (const call of mocks.showObjectView.mock.calls)
			expect(call[1]).toBe(false);
	});

	it("activates runtime and Show Object views only for the displayed pool", async () => {
		renderCuelistWindow(poolPane());
		await settle();

		expect(authority.requestedKeys()).toEqual(
			new Set(["playback:1", "playback:2"]),
		);
		expect(mocks.showObjectKindsView).toHaveBeenCalledWith(
			["cue_list", "playback", "playback_page"],
			true,
		);
	});
});

describe("Cuelist Pool workflows over scoped authority", () => {
	it("keeps Store, Set, and long-press settings behavior intact", async () => {
		mocks.state.storeArmed = true;
		const view = renderCuelistWindow(
			<CuelistWindow compact cueListTab="pool" />,
		);
		await settle();
		const ui = within(view.container);

		const { fireEvent } = await import("@testing-library/react");
		fireEvent.click(ui.getByText("Main").closest("button")!);
		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledWith({
				target: { kind: "pool", playbackNumber: 1 },
				operation: "overwrite",
				timing: {},
				cueOnly: false,
				capturePolicy: "current_capture",
				activationPolicy: "hold",
			}),
		);
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});

		mocks.state.storeArmed = false;
		mocks.state.cueListSetArmed = true;
		view.rerenderWith(<CuelistWindow compact cueListTab="pool" />);
		fireEvent.click(ui.getByText("Encore").closest("button")!);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_TARGET",
			value: 2,
		});
	});
});
