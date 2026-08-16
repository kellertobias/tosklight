import {
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CueList, PlaybackDefinition } from "../api/types";
import { exclusionFenceForSlot } from "../components/control/virtualPlayback/VirtualPlaybackGrid";
import { PaneSettingsModal } from "../components/modals/PaneSettingsModal";
import { PaneChromeProvider } from "../components/shell/PaneChromeContext";
import { registerControlSurfaceTarget } from "../features/controlSurfaceInteraction/registry";
import { cueProjection } from "../features/playbackRuntime/testFixtures";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";

describe("Virtual Playback exclusion-zone fences", () => {
	it("joins orthogonal neighbors and outlines disconnected islands", () => {
		const zones = [{ playbackNumbers: [1001, 1002, 1004, 1005, 1009] }];

		expect(exclusionFenceForSlot(zones, 1, 1, 3, 9)).toEqual({
			top: true,
			right: false,
			bottom: false,
			left: true,
		});
		expect(exclusionFenceForSlot(zones, 1, 5, 3, 9)).toEqual({
			top: false,
			right: true,
			bottom: true,
			left: false,
		});
		expect(exclusionFenceForSlot(zones, 1, 9, 3, 9)).toEqual({
			top: true,
			right: true,
			bottom: true,
			left: true,
		});
	});

	it("treats overlapping zones as one connected visual boundary", () => {
		const zones = [
			{ playbackNumbers: [1001, 1002] },
			{ playbackNumbers: [1002, 1003] },
		];

		expect(exclusionFenceForSlot(zones, 1, 2, 3, 3)).toEqual({
			top: true,
			right: false,
			bottom: true,
			left: false,
		});
	});
});

const portalTargets: HTMLElement[] = [];
const render = (ui: Parameters<typeof rtlRender>[0]) => {
	const toolbar = document.createElement("span");
	toolbar.className = "pane-chrome-toolbar-target";
	document.body.append(toolbar);
	portalTargets.push(toolbar);
	const rendered = rtlRender(
		<PaneChromeProvider value={{ info: null, toolbar }}>
			{ui}
		</PaneChromeProvider>,
		{ wrapper: ModalProvider },
	);
	return {
		...rendered,
		rerender(next: Parameters<typeof rendered.rerender>[0]) {
			rendered.rerender(
				<PaneChromeProvider value={{ info: null, toolbar }}>
					{next}
				</PaneChromeProvider>,
			);
		},
	};
};

const mocks = vi.hoisted(() => {
	const loadSurface = vi.fn();
	const saveSurface = vi.fn();
	const zoneSurfaces = new Map<string, readonly unknown[]>();
	const zoneSaving = new Set<string>();
	const zoneListeners = new Map<string, Set<() => void>>();
	const notifyZone = () => {
		for (const listener of zoneListeners.get("global") ?? []) listener();
	};
	const publishZones = (zones: readonly unknown[]) => {
		zoneSurfaces.set("global", zones);
		notifyZone();
	};
	const playback: PlaybackDefinition = {
		number: 7,
		name: "Front Wash",
		target: { type: "cue_list" as const, cue_list_id: "cue-1" },
		buttons: ["toggle", "none", "none"],
		button_count: 1 as const,
		fader: "master" as const,
		has_fader: false,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#8b5cf6",
		flash_release: "release_all" as const,
		protect_from_swap: false,
	};
	const virtualPlayback: PlaybackDefinition = { ...playback, number: 1001 };
	const page = {
		number: 1,
		name: "Main",
		slots: { "1": 7 } as Record<string, number>,
		virtual_playbacks: {
			"1001": virtualPlayback,
		} as Record<string, PlaybackDefinition>,
	};
	const cueList: CueList = {
		id: "cue-1",
		name: "Front sequence",
		cues: [],
		mode: "sequence" as const,
		priority: 0,
		looped: false,
	};
	return {
		dispatch: vi.fn(),
		recordCue: vi.fn(),
		resetCommand: vi.fn(),
		useServer: vi.fn(() => ({ playbacks: { pool: [] } })),
		configureSlot: vi.fn(),
		configureVirtual: vi.fn(),
		clearMappedPlayback: vi.fn(),
		clearVirtual: vi.fn(),
		topologyActionError: null as Error | null,
		poolPlaybackAction: vi.fn(),
		loadSurface,
		saveSurface,
		zoneSurfaces,
		zoneSaving,
		zoneListeners,
		topologyEnabled: [] as boolean[],
		deskEnabled: [] as boolean[],
		runtimeSelections: [] as Array<
			Array<{ page: number; playbackNumber: number }>
		>,
		topology: {
			ready: true,
			error: null as Error | null,
			playbacks: [{ id: "7", revision: 2, updated_at: "", body: playback }],
			pages: [{ id: "1", revision: 3, updated_at: "", body: page }],
			cueLists: [{ id: "cue-1", revision: 4, updated_at: "", body: cueList }],
		},
		playback,
		virtualPlayback,
		page,
		cueList,
		desk: {
			scope: { show_id: "show-1", show_revision: 4 },
			desk_id: "desk-1",
			active_page: 1,
			selected_playback: null,
		} as Record<string, unknown> | null,
		runtimeStatus: {
			status: "ready" as "idle" | "loading" | "ready" | "error",
			error: null as Error | null,
		},
		runtimes: new Map<string, ReturnType<typeof cueProjection>>(),
		zoneCapability: {
			authorityId: "session-a" as string | null,
			authorityGeneration: 1,
			available: true,
			error: null as string | null,
			getZones: vi.fn(() => zoneSurfaces.get("global") ?? null),
			isSaving: vi.fn(() => zoneSaving.has("global")),
			subscribe: vi.fn((listener: () => void) => {
				const listeners = zoneListeners.get("global") ?? new Set();
				listeners.add(listener);
				zoneListeners.set("global", listeners);
				return () => listeners.delete(listener);
			}),
			activate: vi.fn(() => () => undefined),
			load: vi.fn(async () => {
				const zones = await loadSurface();
				if (zones) publishZones(zones);
				return zones;
			}),
			save: vi.fn(async (zones: readonly unknown[]) => {
				zoneSaving.add("global");
				notifyZone();
				try {
					const saved = await saveSurface(zones);
					if (saved) publishZones(saved);
					return saved;
				} finally {
					zoneSaving.delete("global");
					notifyZone();
				}
			}),
			clearError: vi.fn(),
		},
		state: {
			activeDeskId: "desk-1",
			paneSettingsId: null as string | null,
			virtualPlaybackZoneEdit: null as {
				zoneId: string;
				name: string;
				playbackNumbers: number[];
			} | null,
			playbackPage: 98,
			playbackSetArmed: false,
			cueListSetArmed: false,
			cueListSetTarget: null as number | null,
			shiftArmed: false,
			storeArmed: false,
			updateArmed: false,
			presetFamily: "Mixed" as const,
			desks: [
				{
					id: "desk-1",
					name: "Desk 1",
					panes: [
						{
							id: "virtual-1",
							kind: "virtual_playbacks" as const,
							title: "Virtual Playbacks",
							x: 1,
							y: 1,
							width: 6,
							height: 6,
							virtualPlaybackRows: 1,
							virtualPlaybackColumns: 2,
							virtualPlaybackPageMode: "follow_main" as
								| "follow_main"
								| "pinned",
							virtualPlaybackPinnedPage: 1,
							virtualPlaybackCells: [{ playbackNumber: 999, action: "toggle" }],
							virtualPlaybackExclusionZones: [],
						},
					],
				},
			],
		},
	};
});

vi.mock("../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));
vi.mock("../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../components/control/commandLine/useCommandLineSurface", () => ({
	useCommandLineSurface: () => ({ reset: mocks.resetCommand }),
}));
vi.mock("../api/ServerContext", () => ({ useServer: mocks.useServer }));
vi.mock("../features/playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackTopologyView: (enabled: boolean) => {
		mocks.topologyEnabled.push(enabled);
		return enabled
			? mocks.topology
			: { ready: false, error: null, playbacks: [], pages: [], cueLists: [] };
	},
}));
vi.mock("../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => ({
		error: mocks.topologyActionError,
		configureSlot: mocks.configureSlot,
		configureVirtual: mocks.configureVirtual,
		clearMappedPlayback: mocks.clearMappedPlayback,
		clearVirtual: mocks.clearVirtual,
		saveCueList: vi.fn(),
	}),
}));
vi.mock("../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: (enabled: boolean) => {
		mocks.deskEnabled.push(enabled);
		return enabled ? mocks.desk : null;
	},
	useVirtualPlaybackProjectionMap: (
		addresses: Array<{ page: number; playbackNumber: number }>,
	) => {
		mocks.runtimeSelections.push(addresses);
		return mocks.runtimes;
	},
	usePlaybackRuntimeActions: () => ({
		poolPlaybackAction: mocks.poolPlaybackAction,
		virtualPlaybackAction: (
			_page: number,
			_playbackNumber: number,
			action: string,
			input: unknown,
		) => mocks.poolPlaybackAction(7, action, input),
	}),
	usePlaybackRuntimeStatus: () => mocks.runtimeStatus,
}));
vi.mock("../features/virtualPlaybackZones/VirtualPlaybackZonesContext", () => ({
	useVirtualPlaybackZones: () => ({ ...mocks.zoneCapability }),
}));
vi.mock("../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: vi.fn(),
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	useCueLists: () => mocks.topology.cueLists,
	useDynamics: () => [],
	usePlaybackDefinitions: () => mocks.topology.playbacks,
	usePortableGroups: () => [],
	useShowObjectCollectionsReady: () => true,
}));

afterEach(() => {
	cleanup();
	for (const target of portalTargets.splice(0)) target.remove();
});

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

beforeEach(() => {
	mocks.dispatch.mockReset();
	mocks.recordCue.mockReset().mockResolvedValue({ status: "changed" });
	mocks.resetCommand.mockReset().mockResolvedValue(true);
	mocks.useServer.mockClear();
	mocks.configureSlot.mockReset().mockResolvedValue({ status: "changed" });
	mocks.configureVirtual.mockReset().mockResolvedValue({ status: "changed" });
	mocks.clearMappedPlayback
		.mockReset()
		.mockResolvedValue({ status: "changed" });
	mocks.clearVirtual.mockReset().mockResolvedValue({ status: "changed" });
	mocks.topologyActionError = null;
	mocks.poolPlaybackAction.mockReset().mockResolvedValue(null);
	mocks.zoneSurfaces.clear();
	mocks.zoneSaving.clear();
	mocks.zoneListeners.clear();
	mocks.loadSurface.mockReset().mockResolvedValue([]);
	mocks.saveSurface.mockReset().mockImplementation(async (zones) => zones);
	mocks.zoneCapability.getZones.mockClear();
	mocks.zoneCapability.isSaving.mockClear();
	mocks.zoneCapability.subscribe.mockClear();
	mocks.zoneCapability.activate.mockClear();
	mocks.zoneCapability.load.mockClear();
	mocks.zoneCapability.save.mockClear();
	mocks.zoneCapability.available = true;
	mocks.zoneCapability.authorityId = "session-a";
	mocks.zoneCapability.authorityGeneration = 1;
	mocks.zoneCapability.error = null;
	mocks.topologyEnabled.length = 0;
	mocks.deskEnabled.length = 0;
	mocks.runtimeSelections.length = 0;
	mocks.topology.ready = true;
	mocks.topology.error = null;
	mocks.topology.playbacks[0].revision = 2;
	mocks.topology.pages[0].revision = 3;
	mocks.topology.pages.splice(1);
	mocks.desk = {
		scope: { show_id: "show-1", show_revision: 4 },
		desk_id: "desk-1",
		active_page: 1,
		selected_playback: null,
	};
	mocks.runtimeStatus.status = "ready";
	mocks.runtimeStatus.error = null;
	mocks.page.slots = { "1": 7 };
	mocks.cueList.cues = [];
	mocks.virtualPlayback.buttons = ["toggle", "none", "none"];
	mocks.page.virtual_playbacks = { "1001": mocks.virtualPlayback };
	mocks.runtimes.clear();
	mocks.runtimes.set("virtual:1.1001", {
		...cueProjection(1001),
		requested: { kind: "virtual", page: 1, playback_number: 1001 },
	});
	Object.assign(mocks.state, {
		paneSettingsId: null,
		virtualPlaybackZoneEdit: null,
		playbackSetArmed: false,
		cueListSetArmed: false,
		cueListSetTarget: null,
		shiftArmed: false,
		storeArmed: false,
		updateArmed: false,
	});
	const pane = mocks.state.desks[0].panes[0];
	pane.virtualPlaybackRows = 1;
	pane.virtualPlaybackColumns = 2;
	pane.virtualPlaybackPageMode = "follow_main";
	pane.virtualPlaybackPinnedPage = 1;
	pane.virtualPlaybackExclusionZones = [];
});

describe("VirtualPlaybacksWindow", () => {
	it("uses only scoped page topology and emits virtual runtime metadata", () => {
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback 1001 page 1 cell 1 Front Wash",
		});
		expect(cell).toHaveTextContent("TOGGLE");
		fireEvent.click(cell);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		expect(mocks.useServer).not.toHaveBeenCalled();
		expect(mocks.runtimeSelections.at(-1)).toEqual([
			{ page: 1, playbackNumber: 1001 },
		]);
	});

	it("keeps a Pinned surface on its configured page", () => {
		const pane = mocks.state.desks[0].panes[0];
		pane.virtualPlaybackPageMode = "pinned";
		pane.virtualPlaybackPinnedPage = 2;
		mocks.topology.pages.push({
			id: "2",
			revision: 1,
			updated_at: "",
			body: {
				number: 2,
				name: "Pinned",
				slots: { "1": 7 },
				virtual_playbacks: {
					"1301": { ...mocks.virtualPlayback, number: 1301 },
				},
			},
		});
		mocks.runtimes.set("virtual:2.1301", {
			...cueProjection(1301),
			requested: { kind: "virtual", page: 2, playback_number: 1301 },
		});

		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		expect(
			screen.getByRole("button", {
				name: "Virtual playback 1301 page 2 cell 1 Front Wash",
			}),
		).toBeInTheDocument();
		expect(mocks.desk?.active_page).toBe(1);
	});

	it("records the touched empty Virtual Playback while Record is armed", async () => {
		mocks.state.storeArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1002 page 1 cell 2 empty",
			}),
		);

		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledWith({
				target: { kind: "virtual", page: 1, playbackNumber: 1002 },
				operation: "overwrite",
				timing: {},
				cueOnly: false,
				capturePolicy: "current_capture",
				activationPolicy: "hold",
			}),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
	});

	it("offers Add, Merge, and Overwrite for a one-Cue Virtual Playback", async () => {
		mocks.state.storeArmed = true;
		mocks.cueList.cues = [
			{
				id: "cue-2-0",
				number: "2.0",
				name: "Only cue",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		);
		expect(screen.getByRole("dialog", { name: "Record Cue choice" })).toHaveTextContent(
			"Add CueMerge CueOverwrite Cue",
		);
		fireEvent.click(screen.getByRole("button", { name: "Merge Cue" }));

		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledWith({
				target: { kind: "virtual", page: 1, playbackNumber: 1001 },
				operation: "merge",
				cueNumber: "2.0",
				timing: {},
				cueOnly: false,
				capturePolicy: "current_capture",
				activationPolicy: "hold",
			}),
		);
	});

	it("renders authoritative runtime without a legacy active-playback fallback", () => {
		mocks.runtimes.set("playback:7", cueProjection(7));
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const assigned = screen.getByRole("button", {
			name: "Virtual playback 1001 page 1 cell 1 Front Wash",
		});
		expect(assigned).toHaveClass("playback-colored", "running");
		expect(assigned).toHaveTextContent("Cue 1");
		expect(
			screen.getByRole("button", {
				name: "Virtual playback 1002 page 1 cell 2 empty",
			}),
		).not.toHaveClass("playback-colored", "running");
	});

	it("stays dormant while inactive and shows loading without stale cells", () => {
		const rendered = render(
			<VirtualPlaybacksWindow paneId="virtual-1" active={false} />,
		);
		expect(screen.getByRole("status")).toHaveTextContent(
			"Loading Virtual Playbacks…",
		);
		expect(screen.queryByText("Front Wash")).not.toBeInTheDocument();
		expect(mocks.zoneCapability.load).not.toHaveBeenCalled();
		expect(mocks.topologyEnabled.at(-1)).toBe(false);
		expect(mocks.deskEnabled.at(-1)).toBe(false);
		expect(mocks.runtimeSelections.at(-1)).toEqual([]);

		mocks.topology.ready = false;
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" active />);
		expect(screen.getByRole("status")).toHaveTextContent(
			"Loading Virtual Playbacks…",
		);
		expect(screen.queryByText("Front Wash")).not.toBeInTheDocument();
	});

	it("does not render a seeded desk before scoped runtime authority is ready", () => {
		mocks.runtimeStatus.status = "loading";
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		expect(screen.getByRole("status")).toHaveTextContent(
			"Loading Virtual Playbacks…",
		);
		expect(screen.queryByText("Front Wash")).not.toBeInTheDocument();
		// The runtime subscription for the mapped playbacks stays active while loading:
		// it is the activation mechanism itself (gating it on authorityReady deadlocked
		// the pane, see useVirtualPlaybackController). Only rendering waits for authority.
		expect(mocks.runtimeSelections.at(-1)).toEqual([
			{ page: 1, playbackNumber: 1001 },
		]);
		expect(mocks.zoneCapability.load).not.toHaveBeenCalled();
	});

	it("opens the scoped one-button faderless configuration without mutation", () => {
		mocks.state.playbackSetArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1002 page 1 cell 2 empty",
			}),
		);
		const modal = screen.getByRole("dialog", {
			name: "Playback Configuration",
		});
		expect(modal).toHaveAttribute("data-page", "1");
		expect(modal).toHaveAttribute("data-slot", "2");
		expect(modal).toHaveAttribute("data-topology", "1 button · faderless");
		expect(
			screen
				.getByText("Presentation", { selector: "label", exact: true })
				.closest(".ui-form-field")
				?.querySelector(".ui-select-trigger"),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", {
				name: "Close playback configuration",
			}),
		);
		expect(mocks.configureSlot).not.toHaveBeenCalled();
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("drops an open configuration when the scoped page changes", () => {
		mocks.state.playbackSetArmed = true;
		const rendered = render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();

		if (mocks.desk) mocks.desk.active_page = 2;
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" />);

		expect(
			screen.queryByRole("dialog", { name: "Playback Configuration" }),
		).toBeNull();
	});

	it("submits the topology revisions captured when configuration opened", async () => {
		mocks.state.playbackSetArmed = true;
		const rendered = render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		);

		mocks.topology.playbacks[0].revision = 8;
		mocks.topology.pages[0].revision = 9;
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.change(screen.getByLabelText("Playback name"), {
			target: { value: "Edited against revision two" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));

		await waitFor(() =>
			expect(mocks.configureVirtual).toHaveBeenCalledWith(
				1,
				1001,
				expect.objectContaining({
					number: 1001,
					name: "Edited against revision two",
				}),
				{
					expectedPageRevision: 3,
					expectedPageObjectId: "1",
				},
			),
		);
	});

	it("opens normal Playback Configuration instead of copying a selected Cuelist", () => {
		mocks.state.cueListSetArmed = true;
		mocks.state.cueListSetTarget = 7;
		mocks.page.virtual_playbacks = {};
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 empty",
			}),
		);

		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();
		expect(mocks.configureVirtual).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_ARMED",
			value: false,
		});
	});

	it("shows an exact scoped topology action failure", () => {
		mocks.topologyActionError = new Error("stale Playback Page revision");
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"stale Playback Page revision",
		);
	});

	it("opens normal Playback Configuration on right-click", () => {
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.contextMenu(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		);

		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();
		expect(screen.queryByRole("button", { name: "Set Source" })).toBeNull();
		expect(screen.queryByRole("button", { name: "Add Target" })).toBeNull();
	});

	it("preserves configured rows and columns through the shared grid view", () => {
		const pane = mocks.state.desks[0].panes[0];
		pane.virtualPlaybackRows = 2;
		pane.virtualPlaybackColumns = 3;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		expect(document.querySelectorAll(".virtual-playback-box")).toHaveLength(6);
		expect(document.querySelector('[data-grid-position="5"]')).toHaveAttribute(
			"data-virtual-playback-slot",
			"6",
		);
	});

	it("routes Update to the mapped Cuelist without firing the playback", () => {
		mocks.state.updateArmed = true;
		const update = vi.fn();
		const release = registerControlSurfaceTarget({
			id: "update-test",
			priority: 1,
			accepts: ({ type }) => type === "update_target",
			handle: update,
		});
		try {
			render(<VirtualPlaybacksWindow paneId="virtual-1" />);
			fireEvent.click(
				screen.getByRole("button", {
					name: "Virtual playback 1001 page 1 cell 1 Front Wash",
				}),
			);
			expect(update).toHaveBeenCalledOnce();
			expect(update.mock.calls[0][0]).toEqual({
				type: "update_target",
				source: "touch",
				target: {
					family: { type: "cue" },
					object_id: "cue-1",
					playback_number: 1001,
					validate_active_context: true,
				},
			});
			expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		} finally {
			release();
		}
	});

	it("keeps momentary Shift-click zone selection inert", async () => {
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		await waitFor(() => expect(mocks.zoneCapability.load).toHaveBeenCalled());
		await waitFor(() =>
			expect(screen.queryByText(/Loading zones/u)).not.toBeInTheDocument(),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
			{ shiftKey: true },
		);

		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(
			screen.getByRole("button", { name: "Cancel Zone Selection" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		).toHaveAttribute("aria-pressed", "true");
	});

	it("orders a held Flash release after its scoped press retry settles", async () => {
		mocks.virtualPlayback.buttons = ["flash", "none", "none"];
		const press = deferred<null>();
		mocks.poolPlaybackAction
			.mockImplementationOnce(() => press.promise)
			.mockResolvedValue(null);
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback 1001 page 1 cell 1 Front Wash",
		});
		fireEvent.pointerDown(cell, { pointerId: 4 });
		fireEvent.pointerUp(cell, { pointerId: 4 });
		fireEvent.lostPointerCapture(cell, { pointerId: 4 });
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(1, 7, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		press.resolve(null);
		await waitFor(() => {
			expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(2, 7, "button", {
				button: 1,
				pressed: false,
				surface: "virtual",
			});
		});

		fireEvent.pointerDown(cell, { pointerId: 5 });
		fireEvent.pointerCancel(cell, { pointerId: 5 });
		fireEvent.lostPointerCapture(cell, { pointerId: 5 });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(4),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(4, 7, "button", {
			button: 1,
			pressed: false,
			surface: "virtual",
		});
	});

	it("releases a held action when the scoped grid unmounts", async () => {
		mocks.virtualPlayback.buttons = ["flash", "none", "none"];
		const rendered = render(
			<VirtualPlaybacksWindow paneId="virtual-1" active />,
		);
		const cell = screen.getByRole("button", {
			name: "Virtual playback 1001 page 1 cell 1 Front Wash",
		});
		fireEvent.pointerDown(cell, { pointerId: 6 });

		rendered.rerender(
			<VirtualPlaybacksWindow paneId="virtual-1" active={false} />,
		);

		await waitFor(() => {
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2);
			expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(7, "button", {
				button: 1,
				pressed: false,
				surface: "virtual",
			});
		});
	});

	it("keeps Swap as a matched virtual press and release", async () => {
		mocks.virtualPlayback.buttons = ["swap", "none", "none"];
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback 1001 page 1 cell 1 Front Wash",
		});

		fireEvent.pointerDown(cell, { pointerId: 7 });
		fireEvent.pointerUp(cell, { pointerId: 7 });

		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(1, 7, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(2, 7, "button", {
			button: 1,
			pressed: false,
			surface: "virtual",
		});
	});

	it("loads zones only while active and persists inert Shift selection", async () => {
		mocks.state.shiftArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		await waitFor(() => expect(mocks.zoneCapability.load).toHaveBeenCalled());
		expect(document.querySelector(".virtual-playback-toolbar")).toBeNull();
		expect(
			screen.queryByRole("button", { name: "Create Exclusion Zone" }),
		).toBeNull();
		expect(
			screen.queryByRole("button", { name: "Cancel Zone Selection" }),
		).toBeNull();
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		);
		expect(
			screen.queryByRole("button", { name: "Create Exclusion Zone" }),
		).toBeNull();
		expect(
			screen.getByRole("button", { name: "Cancel Zone Selection" }),
		).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1002 page 1 cell 2 empty",
			}),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(
			screen
				.getByRole("button", { name: "Create Exclusion Zone" })
				.closest(".pane-chrome-toolbar-target"),
		).not.toBeNull();
		fireEvent.click(
			await screen.findByRole("button", { name: "Create Exclusion Zone" }),
		);
		fireEvent.change(screen.getByLabelText("Zone name"), {
			target: { value: "Front alternates" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Create zone" }));
		await waitFor(() =>
			expect(mocks.zoneCapability.save).toHaveBeenCalledWith([
				expect.objectContaining({
					name: "Front alternates",
					playbackNumbers: [1001, 1002],
				}),
			]),
		);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: false,
		});
	});

	it("updates an existing zone from the title bar and preserves hidden members", async () => {
		const zone = {
			id: "zone-1",
			name: "Front alternates",
			playbackNumbers: [1001, 1002, 1144],
		};
		mocks.loadSurface.mockResolvedValue([zone]);
		mocks.state.virtualPlaybackZoneEdit = {
			zoneId: zone.id,
			name: zone.name,
			playbackNumbers: [...zone.playbackNumbers],
		};
		mocks.state.desks[0].panes[0].virtualPlaybackColumns = 3;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		const update = await screen.findByRole("button", {
			name: "Update Exclusion Zone",
		});
		expect(
			screen.getByRole("button", { name: "Cancel Edit" }),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Create Exclusion Zone" }),
		).toBeNull();
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback 1003 page 1 cell 3 empty",
			}),
			{ shiftKey: true },
		);
		fireEvent.click(update);

		await waitFor(() =>
			expect(mocks.zoneCapability.save).toHaveBeenCalledWith([
				{
					id: "zone-1",
					name: "Front alternates",
					playbackNumbers: [1001, 1002, 1003, 1144],
				},
			]),
		);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_VIRTUAL_PLAYBACK_ZONE_EDIT",
			edit: null,
		});
	});

	it("cancels zone editing without persisting", async () => {
		const zone = {
			id: "zone-1",
			name: "Front alternates",
			playbackNumbers: [1001, 1002],
		};
		mocks.loadSurface.mockResolvedValue([zone]);
		mocks.state.virtualPlaybackZoneEdit = {
			zoneId: zone.id,
			name: zone.name,
			playbackNumbers: [...zone.playbackNumbers],
		};
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);

		fireEvent.click(await screen.findByRole("button", { name: "Cancel Edit" }));

		expect(mocks.zoneCapability.save).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_VIRTUAL_PLAYBACK_ZONE_EDIT",
			edit: null,
		});
	});

	it("reloads zones for authority replacement but not error-only rerenders", async () => {
		mocks.loadSurface
			.mockResolvedValueOnce([
				{ id: "zone-a", name: "Authority A", playbackNumbers: [1001, 1002] },
			])
			.mockResolvedValueOnce([
				{ id: "zone-b", name: "Authority B", playbackNumbers: [1001, 1002] },
			]);
		const rendered = render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		await waitFor(() =>
			expect(mocks.zoneCapability.load).toHaveBeenCalledTimes(1),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", {
					name: "Virtual playback 1001 page 1 cell 1 Front Wash",
				}),
			).toHaveAttribute("data-exclusion-zones", "Authority A"),
		);

		mocks.zoneCapability.error = "save failed";
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" />);
		expect(screen.getByRole("alert")).toHaveTextContent("save failed");
		expect(mocks.zoneCapability.load).toHaveBeenCalledTimes(1);
		expect(
			screen.getByRole("button", {
				name: "Virtual playback 1001 page 1 cell 1 Front Wash",
			}),
		).toHaveAttribute("data-exclusion-zones", "Authority A");

		mocks.zoneCapability.error = null;
		mocks.zoneCapability.authorityId = "session-b";
		mocks.zoneCapability.authorityGeneration = 2;
		mocks.zoneSurfaces.clear();
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" />);
		await waitFor(() =>
			expect(mocks.zoneCapability.load).toHaveBeenCalledTimes(2),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", {
					name: "Virtual playback 1001 page 1 cell 1 Front Wash",
				}),
			).toHaveAttribute("data-exclusion-zones", "Authority B"),
		);
	});

	it("keeps Virtual Playback cells above the physical slot domain assignable", () => {
		const pane = mocks.state.desks[0].panes[0];
		pane.virtualPlaybackRows = 12;
		pane.virtualPlaybackColumns = 12;
		mocks.state.playbackSetArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback 1128 page 1 cell 128 empty",
		});
		expect(cell).toBeEnabled();
		expect(cell).toHaveAttribute("data-availability", "empty");
		expect(cell).toHaveTextContent("Empty");
		expect(mocks.configureVirtual).not.toHaveBeenCalled();
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});
});

describe("Virtual Playback Pane Settings", () => {
	const zones = [
		{
			id: "zone-1",
			name: "Front alternates",
			playbackNumbers: [1001, 1002, 1004],
		},
	];

	beforeEach(() => {
		mocks.state.paneSettingsId = "virtual-1";
		mocks.loadSurface.mockResolvedValue(zones);
	});

	it("keeps layout concise and loads zones only in their own tab", async () => {
		render(<PaneSettingsModal />);
		expect(mocks.zoneCapability.load).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("tab", { name: "Virtual Playbacks" }));
		expect(screen.getByLabelText("Rows")).toBeInTheDocument();
		expect(screen.getByLabelText("Columns")).toBeInTheDocument();
		expect(
			screen.getByText("2 of 300 available Virtual Playback positions."),
		).toBeInTheDocument();
		expect(screen.queryByText(/Set Source/)).toBeNull();
		expect(screen.queryByText(/Add Target/)).toBeNull();
		expect(screen.queryByText("Cue List Colors")).toBeNull();
		expect(screen.queryByText("Type Colors")).toBeNull();
		expect(screen.queryByText("Individual Colors")).toBeNull();
		expect(mocks.zoneCapability.load).not.toHaveBeenCalled();

		fireEvent.click(screen.getByRole("tab", { name: "Exclusion Zones" }));
		await waitFor(() => expect(mocks.zoneCapability.load).toHaveBeenCalled());
		expect(
			await screen.findByText(/Virtual Playbacks 1001, 1002, 1004/),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Edit Zone" }),
		).toBeInTheDocument();
		expect(mocks.useServer).not.toHaveBeenCalled();
	});

	it("renames and deletes zones through the scoped capability", async () => {
		render(<PaneSettingsModal />);
		fireEvent.click(screen.getByRole("tab", { name: "Exclusion Zones" }));
		await screen.findByLabelText("Name for Front alternates");
		fireEvent.change(screen.getByLabelText("Name for Front alternates"), {
			target: { value: "Front choice" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save name" }));
		await waitFor(() =>
			expect(mocks.zoneCapability.save).toHaveBeenCalledWith([
				{
					id: "zone-1",
					name: "Front choice",
					playbackNumbers: [1001, 1002, 1004],
				},
			]),
		);

		mocks.zoneCapability.save.mockClear();
		fireEvent.click(screen.getByRole("button", { name: "Delete zone" }));
		await waitFor(() =>
			expect(mocks.zoneCapability.save).toHaveBeenCalledWith([]),
		);
	});

	it("closes settings and hands the selected zone to live-grid editing", async () => {
		render(<PaneSettingsModal />);
		fireEvent.click(screen.getByRole("tab", { name: "Exclusion Zones" }));
		fireEvent.click(await screen.findByRole("button", { name: "Edit Zone" }));

		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_VIRTUAL_PLAYBACK_ZONE_EDIT",
			edit: {
				zoneId: "zone-1",
				name: "Front alternates",
				playbackNumbers: [1001, 1002, 1004],
			},
		});
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_SETTINGS",
			id: null,
		});
	});
});
