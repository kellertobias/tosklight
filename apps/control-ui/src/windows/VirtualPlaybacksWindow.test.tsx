import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../api/types";
import { PaneSettingsModal } from "../components/modals/PaneSettingsModal";
import { cueProjection } from "../features/playbackRuntime/testFixtures";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";

const mocks = vi.hoisted(() => {
	const loadSurface = vi.fn();
	const saveSurface = vi.fn();
	const zoneSurfaces = new Map<string, readonly unknown[]>();
	const zoneSaving = new Set<string>();
	const zoneListeners = new Map<string, Set<() => void>>();
	const notifyZone = (surfaceId: string) => {
		for (const listener of zoneListeners.get(surfaceId) ?? []) listener();
	};
	const publishZones = (surfaceId: string, zones: readonly unknown[]) => {
		zoneSurfaces.set(surfaceId, zones);
		notifyZone(surfaceId);
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
	const page = {
		number: 1,
		name: "Main",
		slots: { "1": 7 } as Record<string, number>,
	};
	const cueList = {
		id: "cue-1",
		name: "Front sequence",
		cues: [],
		mode: "sequence" as const,
		priority: 0,
		looped: false,
	};
	return {
		dispatch: vi.fn(),
		useServer: vi.fn(() => ({ playbacks: { pool: [] } })),
		configureSlot: vi.fn(),
		clearMappedPlayback: vi.fn(),
		topologyActionError: null as Error | null,
		poolPlaybackAction: vi.fn(),
		loadSurface,
		saveSurface,
		zoneSurfaces,
		zoneSaving,
		zoneListeners,
		topologyEnabled: [] as boolean[],
		deskEnabled: [] as boolean[],
		runtimeSelections: [] as number[][],
		topology: {
			ready: true,
			error: null as Error | null,
			playbacks: [
				{ id: "7", revision: 2, updated_at: "", body: playback },
			],
			pages: [{ id: "1", revision: 3, updated_at: "", body: page }],
			cueLists: [
				{ id: "cue-1", revision: 4, updated_at: "", body: cueList },
			],
		},
		playback,
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
		runtimes: new Map<number, ReturnType<typeof cueProjection>>(),
		zoneCapability: {
			authorityId: "session-a" as string | null,
			authorityGeneration: 1,
			available: true,
			error: null as string | null,
			getSurface: vi.fn((surfaceId: string) =>
				zoneSurfaces.get(surfaceId) ?? null,
			),
			isSavingSurface: vi.fn((surfaceId: string) =>
				zoneSaving.has(surfaceId),
			),
			subscribeSurface: vi.fn(
				(surfaceId: string, listener: () => void) => {
					const listeners = zoneListeners.get(surfaceId) ?? new Set();
					listeners.add(listener);
					zoneListeners.set(surfaceId, listeners);
					return () => listeners.delete(listener);
				},
			),
			loadSurface: vi.fn(async (surfaceId: string) => {
				const zones = await loadSurface(surfaceId);
				if (zones) publishZones(surfaceId, zones);
				return zones;
			}),
			saveSurface: vi.fn(async (surfaceId: string, zones: readonly unknown[]) => {
				zoneSaving.add(surfaceId);
				notifyZone(surfaceId);
				try {
					const saved = await saveSurface(surfaceId, zones);
					if (saved) publishZones(surfaceId, saved);
					return saved;
				} finally {
					zoneSaving.delete(surfaceId);
					notifyZone(surfaceId);
				}
			}),
			clearError: vi.fn(),
		},
		state: {
			activeDeskId: "desk-1",
			paneSettingsId: null as string | null,
			playbackPage: 98,
			playbackSetArmed: false,
			cueListSetArmed: false,
			cueListSetTarget: null as number | null,
			shiftArmed: false,
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
							virtualPlaybackCells: [
								{ playbackNumber: 999, action: "toggle" },
							],
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
		clearMappedPlayback: mocks.clearMappedPlayback,
		saveCueList: vi.fn(),
	}),
}));
vi.mock("../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: (enabled: boolean) => {
		mocks.deskEnabled.push(enabled);
		return enabled ? mocks.desk : null;
	},
	usePlaybackProjectionMap: (numbers: number[]) => {
		mocks.runtimeSelections.push(numbers);
		return mocks.runtimes;
	},
	usePlaybackRuntimeActions: () => ({
		poolPlaybackAction: mocks.poolPlaybackAction,
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
	usePortableGroups: () => [],
}));

afterEach(cleanup);

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((onResolve) => {
		resolve = onResolve;
	});
	return { promise, resolve };
}

beforeEach(() => {
	mocks.dispatch.mockReset();
	mocks.useServer.mockClear();
	mocks.configureSlot.mockReset().mockResolvedValue({ status: "changed" });
	mocks.clearMappedPlayback.mockReset().mockResolvedValue({ status: "changed" });
	mocks.topologyActionError = null;
	mocks.poolPlaybackAction.mockReset().mockResolvedValue(null);
	mocks.zoneSurfaces.clear();
	mocks.zoneSaving.clear();
	mocks.zoneListeners.clear();
	mocks.loadSurface.mockReset().mockResolvedValue([]);
	mocks.saveSurface
		.mockReset()
		.mockImplementation(async (_surfaceId, zones) => zones);
	mocks.zoneCapability.getSurface.mockClear();
	mocks.zoneCapability.isSavingSurface.mockClear();
	mocks.zoneCapability.subscribeSurface.mockClear();
	mocks.zoneCapability.loadSurface.mockClear();
	mocks.zoneCapability.saveSurface.mockClear();
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
	mocks.desk = {
		scope: { show_id: "show-1", show_revision: 4 },
		desk_id: "desk-1",
		active_page: 1,
		selected_playback: null,
	};
	mocks.runtimeStatus.status = "ready";
	mocks.runtimeStatus.error = null;
	mocks.page.slots = { "1": 7 };
	mocks.playback.buttons = ["toggle", "none", "none"];
	mocks.runtimes.clear();
	Object.assign(mocks.state, {
		paneSettingsId: null,
		playbackSetArmed: false,
		cueListSetArmed: false,
		cueListSetTarget: null,
		shiftArmed: false,
		updateArmed: false,
	});
	const pane = mocks.state.desks[0].panes[0];
	pane.virtualPlaybackRows = 1;
	pane.virtualPlaybackColumns = 2;
	pane.virtualPlaybackExclusionZones = [];
});

describe("VirtualPlaybacksWindow", () => {
	it("uses only scoped page topology and emits virtual runtime metadata", () => {
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback page 1 cell 1 Front Wash",
		});
		expect(cell).toHaveTextContent("TOGGLE");
		fireEvent.click(cell);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", {
			button: 1,
			pressed: true,
			surface: "virtual",
		});
		expect(mocks.useServer).not.toHaveBeenCalled();
		expect(mocks.runtimeSelections.at(-1)).toEqual([7]);
	});

	it("renders authoritative runtime without a legacy active-playback fallback", () => {
		mocks.runtimes.set(7, cueProjection(7));
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const assigned = screen.getByRole("button", {
			name: "Virtual playback page 1 cell 1 Front Wash",
		});
		expect(assigned).toHaveClass("playback-colored", "running");
		expect(assigned).toHaveTextContent("Cue 1");
		expect(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 2 empty",
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
		expect(mocks.zoneCapability.loadSurface).not.toHaveBeenCalled();
		expect(mocks.topologyEnabled.at(-1)).toBe(false);
		expect(mocks.deskEnabled.at(-1)).toBe(false);
		expect(mocks.runtimeSelections.at(-1)).toEqual([]);

		mocks.topology.ready = false;
		rendered.rerender(
			<VirtualPlaybacksWindow paneId="virtual-1" active />,
		);
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
		expect(mocks.runtimeSelections.at(-1)).toEqual([]);
		expect(mocks.zoneCapability.loadSurface).not.toHaveBeenCalled();
	});

	it("opens the scoped one-button faderless configuration without mutation", () => {
		mocks.state.playbackSetArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 2 empty",
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
		const rendered = render(
			<VirtualPlaybacksWindow paneId="virtual-1" />,
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 1 Front Wash",
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
		const rendered = render(
			<VirtualPlaybacksWindow paneId="virtual-1" />,
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 1 Front Wash",
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
			expect(mocks.configureSlot).toHaveBeenCalledWith(
				1,
				1,
				expect.objectContaining({ name: "Edited against revision two" }),
				{
						expectedPageRevision: 3,
						expectedPageObjectId: "1",
						expectedPlaybackRevision: 2,
						expectedPlaybackObjectId: "7",
					},
			),
		);
	});

	it("assigns a scoped Cuelist source as one button and faderless", async () => {
		mocks.state.cueListSetArmed = true;
		mocks.state.cueListSetTarget = 7;
		mocks.page.slots = {};
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 1 empty",
			}),
		);
		await waitFor(() =>
			expect(mocks.configureSlot).toHaveBeenCalledWith(
				1,
				1,
				expect.objectContaining({
					number: 0,
					target: { type: "cue_list", cue_list_id: "cue-1" },
					buttons: ["toggle", "none", "none"],
					button_count: 1,
					has_fader: false,
				}),
			),
		);
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

	it("preserves Set Source and Add Target entry points", () => {
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		fireEvent.click(screen.getByRole("button", { name: "Set Source" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_TARGET",
			value: null,
		});
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_ARMED",
			value: true,
		});
		mocks.dispatch.mockClear();
		fireEvent.click(screen.getByRole("button", { name: "Add Target" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_ARMED",
			value: true,
		});
	});

	it("orders a held Flash release after its scoped press retry settles", async () => {
		mocks.playback.buttons = ["flash", "none", "none"];
		const press = deferred<null>();
		mocks.poolPlaybackAction
			.mockImplementationOnce(() => press.promise)
			.mockResolvedValue(null);
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback page 1 cell 1 Front Wash",
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
			expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(
				2,
				7,
				"button",
				{
					button: 1,
					pressed: false,
					surface: "virtual",
				},
			);
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
		mocks.playback.buttons = ["flash", "none", "none"];
		const rendered = render(
			<VirtualPlaybacksWindow paneId="virtual-1" active />,
		);
		const cell = screen.getByRole("button", {
			name: "Virtual playback page 1 cell 1 Front Wash",
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

	it("loads zones only while active and persists inert Shift selection", async () => {
		mocks.state.shiftArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		await waitFor(() =>
			expect(mocks.zoneCapability.loadSurface).toHaveBeenCalledWith("virtual-1"),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 1 Front Wash",
			}),
		);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 2 empty",
			}),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		fireEvent.click(
			await screen.findByRole("button", { name: "Create Exclusion Zone" }),
		);
		fireEvent.change(screen.getByLabelText("Zone name"), {
			target: { value: "Front alternates" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Create zone" }));
		await waitFor(() =>
			expect(mocks.zoneCapability.saveSurface).toHaveBeenCalledWith(
				"virtual-1",
				[
					expect.objectContaining({
						name: "Front alternates",
						slots: [1, 2],
					}),
				],
			),
		);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: false,
		});
	});

	it("reloads zones for authority replacement but not error-only rerenders", async () => {
		mocks.loadSurface
			.mockResolvedValueOnce([
				{ id: "zone-a", name: "Authority A", slots: [1, 2] },
			])
			.mockResolvedValueOnce([
				{ id: "zone-b", name: "Authority B", slots: [1, 2] },
			]);
		const rendered = render(
			<VirtualPlaybacksWindow paneId="virtual-1" />,
		);
		await waitFor(() =>
			expect(mocks.zoneCapability.loadSurface).toHaveBeenCalledTimes(1),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", {
					name: "Virtual playback page 1 cell 1 Front Wash",
				}),
			).toHaveAttribute("data-exclusion-zones", "Authority A"),
		);

		mocks.zoneCapability.error = "save failed";
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" />);
		expect(screen.getByRole("alert")).toHaveTextContent("save failed");
		expect(mocks.zoneCapability.loadSurface).toHaveBeenCalledTimes(1);
		expect(
			screen.getByRole("button", {
				name: "Virtual playback page 1 cell 1 Front Wash",
			}),
		).toHaveAttribute("data-exclusion-zones", "Authority A");

		mocks.zoneCapability.error = null;
		mocks.zoneCapability.authorityId = "session-b";
		mocks.zoneCapability.authorityGeneration = 2;
		mocks.zoneSurfaces.clear();
		rendered.rerender(<VirtualPlaybacksWindow paneId="virtual-1" />);
		await waitFor(() =>
			expect(mocks.zoneCapability.loadSurface).toHaveBeenCalledTimes(2),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("button", {
					name: "Virtual playback page 1 cell 1 Front Wash",
				}),
			).toHaveAttribute("data-exclusion-zones", "Authority B"),
		);
	});

	it("disables grid cells above the 127-slot desk domain", () => {
		const pane = mocks.state.desks[0].panes[0];
		pane.virtualPlaybackRows = 12;
		pane.virtualPlaybackColumns = 12;
		mocks.page.slots = { "1": 7, "128": 7 };
		mocks.state.playbackSetArmed = true;
		render(<VirtualPlaybacksWindow paneId="virtual-1" />);
		const cell = screen.getByRole("button", {
			name: "Virtual playback page 1 cell 128 unavailable",
		});
		expect(cell).toBeDisabled();
		expect(cell).toHaveTextContent("Unavailable");
		fireEvent.click(cell);
		expect(mocks.configureSlot).not.toHaveBeenCalled();
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});
});

describe("Virtual Playback Pane Settings", () => {
	const zones = [
		{ id: "zone-1", name: "Front alternates", slots: [1, 2, 4] },
	];

	beforeEach(() => {
		mocks.state.paneSettingsId = "virtual-1";
		mocks.loadSurface.mockResolvedValue(zones);
	});

	it("loads named zones only when its tab is active", async () => {
		render(<PaneSettingsModal />);
		expect(mocks.zoneCapability.loadSurface).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("tab", { name: "Virtual Playbacks" }));
		expect(screen.getByLabelText("Rows")).toBeInTheDocument();
		expect(screen.getByLabelText("Columns")).toBeInTheDocument();
		expect(screen.queryByText(/Cell 1 Cuelist/)).not.toBeInTheDocument();
		expect(screen.getByText(/Set Source/)).toBeInTheDocument();
		expect(screen.getByText(/Add Target/)).toBeInTheDocument();
		await waitFor(() =>
			expect(mocks.zoneCapability.loadSurface).toHaveBeenCalledWith("virtual-1"),
		);
		expect(
			await screen.findByText("1 hidden grid cell is retained:"),
		).toBeInTheDocument();
		expect(
			screen.getByRole("button", {
				name: "Front alternates hidden cell 4",
			}),
		).toBeInTheDocument();
		expect(mocks.useServer).not.toHaveBeenCalled();
	});

	it("renames, edits, and deletes zones through the scoped capability", async () => {
		render(<PaneSettingsModal />);
		fireEvent.click(screen.getByRole("tab", { name: "Virtual Playbacks" }));
		await screen.findByLabelText("Name for Front alternates");
		fireEvent.change(screen.getByLabelText("Name for Front alternates"), {
			target: { value: "Front choice" },
		});
		fireEvent.click(screen.getByRole("button", { name: "Save name" }));
		await waitFor(() =>
			expect(mocks.zoneCapability.saveSurface).toHaveBeenCalledWith(
				"virtual-1",
				[{ id: "zone-1", name: "Front choice", slots: [1, 2, 4] }],
			),
		);

		mocks.zoneCapability.saveSurface.mockClear();
		fireEvent.click(
			screen.getByRole("button", { name: "Front choice hidden cell 4" }),
		);
		await waitFor(() =>
			expect(mocks.zoneCapability.saveSurface).toHaveBeenCalledWith(
				"virtual-1",
				[{ id: "zone-1", name: "Front choice", slots: [1, 2] }],
			),
		);

		mocks.zoneCapability.saveSurface.mockClear();
		fireEvent.click(screen.getByRole("button", { name: "Delete zone" }));
		await waitFor(() =>
			expect(mocks.zoneCapability.saveSurface).toHaveBeenCalledWith(
				"virtual-1",
				[],
			),
		);
	});

	it("retains and removes a legacy cell above the assignable slot limit", async () => {
		mocks.loadSurface.mockResolvedValue([
			{ id: "legacy-zone", name: "Legacy zone", slots: [1, 2, 144] },
		]);
		render(<PaneSettingsModal />);
		fireEvent.click(screen.getByRole("tab", { name: "Virtual Playbacks" }));

		const retained = await screen.findByRole("button", {
			name: "Legacy zone hidden cell 144",
		});
		fireEvent.click(retained);

		await waitFor(() =>
			expect(mocks.zoneCapability.saveSurface).toHaveBeenCalledWith(
				"virtual-1",
				[{ id: "legacy-zone", name: "Legacy zone", slots: [1, 2] }],
			),
		);
	});
});
