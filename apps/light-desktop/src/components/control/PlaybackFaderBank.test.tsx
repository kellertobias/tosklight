import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	PlaybackFaderBank as PhysicalPlaybackFaderBank,
	playbackRowUnits,
} from "./PlaybackFaderBank";
import { UPDATE_TARGET_EVENT } from "./updateWorkflow";

describe("playback row height units", () => {
	const oneButton = {
		first_playback_slot: 1,
		has_fader: false,
		button_count: 1,
	};
	const multipleButtons = {
		first_playback_slot: 11,
		has_fader: false,
		button_count: 3,
	};
	const fader = { first_playback_slot: 21, has_fader: true, button_count: 1 };

	it("uses 1/1/2 units with attached hardware", () => {
		expect(
			[oneButton, multipleButtons, fader].map((row) =>
				playbackRowUnits(row, true),
			),
		).toEqual([1, 1, 2]);
	});

	it("uses 1/2/4 units for touch controls", () => {
		expect(
			[oneButton, multipleButtons, fader].map((row) =>
				playbackRowUnits(row, false),
			),
		).toEqual([1, 2, 4]);
	});
});

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	executeCommandLine: vi.fn(),
	refresh: vi.fn(),
	poolPlaybackAction: vi.fn(),
	resetCommandLine: vi.fn(),
	savePlaybackSlot: vi.fn(),
	clearPlaybackSlot: vi.fn(),
	mapExistingPlayback: vi.fn(),
	recordCue: vi.fn(),
	commandLine: "FIXTURE",
	error: null as string | null,
	hardwareConnected: false,
	topologyReady: true,
	runtimeReady: true,
	groupReady: true,
	showObjectView: vi.fn(),
	portableGroups: vi.fn(),
	topologyView: vi.fn(),
	runtimeProjectionMismatch: false,
	runtimeCueListIdOverride: null as string | null,
	state: {
		midiProfile: null,
		playbackColumns: 1,
		playbackRows: 1,
		playbackPage: 0,
		cueListSetTarget: 12 as number | null,
		cueListSetArmed: true,
		playbackSetArmed: false,
		shiftArmed: false,
		updateArmed: false,
		storeArmed: false,
		blackout: false,
	},
	playbacks: {
		active_page: 1,
		pages: [{ number: 1, name: "Main", slots: {} as Record<string, number> }],
		pool: [] as Array<Record<string, any>>,
		active: [] as Array<Record<string, any>>,
		cue_lists: [
			{
				id: "front",
				name: "Front sequence",
				cues: [] as Array<Record<string, any>>,
				mode: "sequence",
				priority: 0,
				looped: false,
			},
		],
		desk: { buttons: 3 },
		selected_playback: null as number | null,
	},
	scopedCueLists: [] as Array<Record<string, any>>,
}));

function PlaybackFaderBank(
	props: ComponentProps<typeof PhysicalPlaybackFaderBank>,
) {
	return (
		<PhysicalPlaybackFaderBank
			{...props}
			hardwareConnected={mocks.hardwareConnected}
		/>
	);
}

vi.mock("../../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingCommandLineView: () => ({
		text: mocks.commandLine,
		target: "FIXTURE",
		pristine: false,
		revision: 1,
		pendingChoice: null,
	}),
	useProgrammingCommandLineActions: () => ({
		reset: mocks.resetCommandLine,
	}),
	useProgrammingInteractionStatus: () => ({ status: "ready", error: null }),
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackTopologyView: () => {
		mocks.topologyView();
		return {
			ready: mocks.topologyReady,
			error: mocks.error ? new Error(mocks.error) : null,
			cueLists: mocks.scopedCueLists.map((body) => ({
			kind: "cue_list",
			id: body.storageId ?? `cue-list-object-${body.id}`,
			revision: 2,
			updated_at: "",
			body,
			})),
			playbacks: mocks.playbacks.pool.map((body) => ({
			kind: "playback",
			id: `playback-object-${body.number}`,
			revision: 5,
			updated_at: "",
			body,
			})),
			pages: mocks.playbacks.pages.map((body) => ({
			kind: "playback_page",
			id: `page-object-${body.number}`,
			revision: 3,
			updated_at: "",
			body,
			})),
		};
	},
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => ({
		configureSlot: mocks.savePlaybackSlot,
		clearMappedPlayback: mocks.clearPlaybackSlot,
		mapExistingPlayback: mocks.mapExistingPlayback,
		error: mocks.error ? new Error(mocks.error) : null,
	}),
}));
vi.mock("../../features/playbackRuntime/PlaybackRuntimeView", () => {
	const reference = (number: unknown) =>
		typeof number === "number" ? { id: `cue-${number}`, number } : null;
	const cueListProjection = (number: number, playback: Record<string, any>) => {
		const active = mocks.playbacks.active.find(
			(candidate) => candidate.playback_number === number,
		);
		return {
			scope: { show_id: "show-1", show_revision: 9 },
			requested: { kind: "playback", playback_number: number },
			playback_number: number,
			target: "cue_list",
			cue_list_id:
				mocks.runtimeCueListIdOverride ?? playback.target.cue_list_id,
			runtime: active
				? {
						cue_index: active.cue_index ?? -1,
						previous_index: active.previous_index ?? null,
						current: reference(
							active.current_cue_number ??
								mocks.scopedCueLists[0]?.cues?.[active.cue_index]?.number,
						),
						loaded: reference(active.loaded_cue_number),
						normal_next: reference(active.normal_next_cue_number),
						effective_next: reference(active.effective_next_cue_number),
						effective_next_is_loaded: Boolean(active.effective_next_is_loaded),
						paused: Boolean(active.paused),
						activated_at: active.activated_at ?? "",
						master: active.master ?? 0,
						fader_position: active.fader_position ?? active.master ?? 0,
						fader_pickup_required: Boolean(active.fader_pickup_required),
						flash: Boolean(active.flash),
						temporary: Boolean(active.temporary),
						temporary_active: Boolean(active.temporary_active),
						temporary_master: active.temporary_master ?? 0,
						swap_active: Boolean(active.swap_active),
						enabled: active.enabled ?? true,
						transition_timing_bypassed: Boolean(
							active.transition_timing_bypassed,
						),
						manual_xfade_position: active.manual_xfade_position ?? 0,
						manual_xfade_direction:
							active.manual_xfade_direction ?? "towards_high",
						manual_xfade_progress: active.manual_xfade_progress ?? 0,
					}
				: null,
		};
	};
	const projection = (number: number) => {
		const playback = mocks.playbacks.pool.find(
			(candidate) => candidate.number === number,
		);
		if (!playback)
			return {
				scope: { show_id: "show-1", show_revision: 9 },
				requested: { kind: "playback", playback_number: number },
				playback_number: number,
				target: "missing",
			};
		if (mocks.runtimeProjectionMismatch)
			return {
				scope: { show_id: "show-1", show_revision: 9 },
				requested: { kind: "playback", playback_number: number },
				playback_number: number,
				target: "missing",
			};
		if (playback.target.type === "cue_list")
			return cueListProjection(number, playback);
		return {
			scope: { show_id: "show-1", show_revision: 9 },
			requested: { kind: "playback", playback_number: number },
			playback_number: number,
			target: playback.target.type,
			...(playback.target.type === "group"
				? { group_id: playback.target.group_id, master: 1, flash_level: 1 }
				: {}),
		};
	};
	return {
		usePlaybackRuntimeActions: () => ({
			poolPlaybackAction: mocks.poolPlaybackAction,
		}),
		usePlaybackRuntimeStatus: () => ({
			status: mocks.runtimeReady ? "ready" : "loading",
			error: null,
		}),
		usePlaybackDeskView: () => ({
			scope: { show_id: "show-1", show_revision: 9 },
			desk_id: "desk-1",
			active_page: mocks.playbacks.active_page,
			selected_playback: mocks.playbacks.selected_playback,
		}),
		usePlaybackProjectionMap: (numbers: number[]) =>
			new Map(numbers.map((number) => [number, projection(number)])),
	};
});
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	usePortableGroups: (enabled: boolean) => {
		mocks.portableGroups(enabled);
		return [];
	},
	useShowObjectCollectionsReady: () => mocks.groupReady,
	useCueLists: () =>
		mocks.scopedCueLists.map((body) => ({
			kind: "cue_list",
			id: body.id,
			revision: 1,
			updated_at: "",
			body,
		})),
	usePlaybackDefinitions: () =>
		mocks.playbacks.pool.map((body) => ({
			kind: "playback",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body,
		})),
	usePlaybackPages: () =>
		mocks.playbacks.pages.map((body) => ({
			kind: "playback_page",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body,
		})),
}));
vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: mocks.showObjectView,
	useShowObjectKindsView: () => undefined,
}));
vi.mock("../../features/server/useShowObjectsState", () => ({
	useGroups: () => [],
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }),
}));

afterEach(cleanup);

function resetPlaybackFaderMocks() {
	mocks.dispatch.mockReset();
	mocks.executeCommandLine.mockReset().mockResolvedValue(true);
	mocks.refresh.mockReset().mockResolvedValue(undefined);
	mocks.poolPlaybackAction.mockReset().mockResolvedValue({ status: "no_change" });
	mocks.resetCommandLine.mockReset();
	mocks.savePlaybackSlot.mockReset().mockResolvedValue(true);
	mocks.clearPlaybackSlot.mockReset().mockResolvedValue(true);
	mocks.mapExistingPlayback.mockReset().mockResolvedValue({ status: "changed" });
	mocks.recordCue.mockReset().mockResolvedValue({ status: "changed" });
	mocks.playbacks.cue_lists = [
		{
			id: "front",
			name: "Front sequence",
			cues: [],
			mode: "sequence",
			priority: 0,
			looped: false,
		},
	];
	mocks.scopedCueLists = mocks.playbacks.cue_lists.map((cueList) => ({
		...cueList,
		cues: [...cueList.cues],
	}));
	mocks.commandLine = "FIXTURE";
	mocks.error = null;
	mocks.hardwareConnected = false;
	mocks.topologyReady = true;
	mocks.runtimeReady = true;
	mocks.groupReady = true;
	mocks.showObjectView.mockReset();
	mocks.portableGroups.mockReset();
	mocks.topologyView.mockReset();
	mocks.runtimeProjectionMismatch = false;
	mocks.runtimeCueListIdOverride = null;
	Object.assign(mocks.state, {
		cueListSetTarget: 12,
		cueListSetArmed: true,
		playbackSetArmed: false,
		shiftArmed: false,
		updateArmed: false,
		storeArmed: false,
		blackout: false,
	});
	mocks.playbacks.pages[0].slots = {};
	mocks.playbacks.pool = [];
	mocks.playbacks.active = [];
	mocks.playbacks.selected_playback = null;
}

function playbackDefinition(
	number: number,
	overrides: Record<string, unknown> = {},
) {
	return {
		number,
		name: "Front Wash",
		target: { type: "cue_list", cue_list_id: "front" },
		buttons: ["go", "go_minus", "flash"],
		button_count: 3,
		fader: "master",
		has_fader: true,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#20c997",
		flash_release: "release_all",
		protect_from_swap: false,
		...overrides,
	};
}

function assignPlayback(overrides: Record<string, unknown> = {}) {
	mocks.playbacks.pages[0].slots = { "1": 7 };
	mocks.playbacks.pool = [playbackDefinition(7, overrides)];
	Object.assign(mocks.state, {
		cueListSetTarget: null,
		cueListSetArmed: false,
	});
}

describe("PlaybackFaderBank layout and configuration surfaces", () => {
	beforeEach(resetPlaybackFaderMocks);

	it("does not render stale topology while scoped authority is loading", () => {
		assignPlayback();
		mocks.topologyReady = false;
		render(<PlaybackFaderBank count={1} />);

		expect(screen.getByRole("status")).toHaveTextContent("Loading Playbacks…");
		expect(
			screen.queryByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		).not.toBeInTheDocument();
	});

	it("rejects a loaded runtime projection that does not match topology", () => {
		assignPlayback();
		mocks.runtimeProjectionMismatch = true;
		render(<PlaybackFaderBank count={1} />);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Playback runtime authority does not match the visible topology",
		);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("rejects a runtime Cuelist projection with a foreign semantic ID", () => {
		assignPlayback();
		mocks.runtimeCueListIdOverride = "foreign-cuelist";
		render(<PlaybackFaderBank count={1} />);

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Playback runtime authority does not match the visible topology",
		);
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("does not rerender for an unrelated parent-context change", () => {
		assignPlayback();
		function Parent({ label }: { label: string }) {
			return (
				<div data-unrelated={label}>
					<PlaybackFaderBank count={1} />
				</div>
			);
		}
		const { rerender } = render(<Parent label="before" />);
		const renders = mocks.topologyView.mock.calls.length;

		rerender(<Parent label="after" />);

		expect(mocks.topologyView).toHaveBeenCalledTimes(renders);
	});

	it("activates the Group collection only for a visible Group playback", () => {
		assignPlayback();
		const { unmount } = render(<PlaybackFaderBank count={1} />);
		expect(mocks.showObjectView).toHaveBeenCalledWith("group", false);
		expect(mocks.portableGroups).toHaveBeenCalledWith(false);
		unmount();
		mocks.showObjectView.mockReset();
		mocks.portableGroups.mockReset();
		assignPlayback({ target: { type: "group", group_id: "group-front" } });
		render(<PlaybackFaderBank count={1} />);

		expect(mocks.showObjectView).toHaveBeenCalledWith("group", true);
		expect(mocks.portableGroups).toHaveBeenCalledWith(true);
	});

	it("projects arbitrary row starts and fills touch height with weighted tracks", () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					playbacks_per_row: 1,
					rows: [
						{ first_playback_slot: 1, has_fader: false, button_count: 1 },
						{ first_playback_slot: 11, has_fader: false, button_count: 3 },
						{ first_playback_slot: 21, has_fader: true, button_count: 1 },
					],
				}}
			/>,
		);

		expect(container.querySelector(".playback-fader-bank")).toHaveStyle({
			gridTemplateRows: "minmax(0, 1fr) minmax(0, 2fr) minmax(0, 4fr)",
		});
		expect(
			[...container.querySelectorAll("[data-playback-slot]")].map((element) =>
				element.getAttribute("data-playback-slot"),
			),
		).toEqual(["1", "11", "21"]);
	});

	it("fills hardware height with faderless and fader row weights", () => {
		mocks.hardwareConnected = true;
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
		});
		const { container } = render(
			<PlaybackFaderBank
				playbackLayout={{
					playbacks_per_row: 1,
					rows: [
						{ first_playback_slot: 1, has_fader: false, button_count: 1 },
						{ first_playback_slot: 11, has_fader: false, button_count: 3 },
						{ first_playback_slot: 21, has_fader: true, button_count: 1 },
					],
				}}
			/>,
		);
		expect(container.querySelector(".playback-fader-bank")).toHaveStyle({
			gridTemplateRows: "minmax(0, 1fr) minmax(0, 1fr) minmax(0, 2fr)",
		});
	});

	it("assigns the selected Cuelist source to the touched physical page slot", async () => {
		mocks.playbacks.pool = [playbackDefinition(12)];
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Assign Cuelist 12 to page 1 playback 1",
			}),
		);
		await waitFor(() =>
			expect(mocks.mapExistingPlayback).toHaveBeenCalledWith(1, 1, 12, {
				expectedPageRevision: 3,
				expectedPageObjectId: "page-object-1",
				expectedPlaybackRevision: 5,
				expectedPlaybackObjectId: "playback-object-12",
			}),
		);
		expect(mocks.executeCommandLine).not.toHaveBeenCalled();
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_ARMED",
			value: false,
		});
	});

	it.each([
		[
			"software representation",
			() =>
				screen.getByRole("button", {
					name: "Playback representation page 1 playback 1",
				}),
		],
		["top button", () => screen.getByRole("button", { name: "GO +" })],
		["middle button", () => screen.getByRole("button", { name: "GO −" })],
		["bottom button", () => screen.getByRole("button", { name: "FLASH" })],
		[
			"fader track and handle",
			() => screen.getByRole("slider", { name: "Master" }),
		],
	])("SET intercepts the %s without executing it and Close is inert", (_surface, target) => {
		assignPlayback();
		mocks.state.playbackSetArmed = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(target());
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-page", "1");
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-slot", "1");
		fireEvent.click(
			screen.getByRole("button", { name: "Close playback configuration" }),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(mocks.savePlaybackSlot).not.toHaveBeenCalled();
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
	});

	it("opens an empty slot without fabricating a playback number and allocates a changed draft on Apply", async () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			playbackSetArmed: true,
		});
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toHaveAttribute("data-topology", "3 buttons · fader");
		expect(screen.getByRole("radio", { name: "None" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "Apply" })).toBeDisabled();
		fireEvent.change(screen.getByLabelText("Playback name"), {
			target: { value: "New Playback" },
		});
		expect(screen.getByRole("button", { name: "Apply" })).toBeEnabled();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledOnce());
		expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(
			1,
			1,
			expect.objectContaining({ number: 0, button_count: 3, has_fader: true }),
			{
				expectedPageRevision: 3,
				expectedPageObjectId: "page-object-1",
				expectedPlaybackRevision: 0,
				expectedPlaybackObjectId: null,
			},
		);
	});

	it("uses the scoped Cuelist authority for an empty slot default", () => {
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			playbackSetArmed: true,
		});
		mocks.playbacks.cue_lists = [
			{ ...mocks.playbacks.cue_lists[0], id: "legacy", name: "Legacy" },
		];
		mocks.scopedCueLists = [
			{ ...mocks.playbacks.cue_lists[0], id: "scoped", name: "Scoped" },
		];

		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);

		expect(screen.getByRole("radio", { name: "Scoped" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		expect(screen.queryByRole("radio", { name: "Legacy" })).toBeNull();
	});
});

describe("PlaybackFaderBank configuration shortcuts", () => {
	beforeEach(resetPlaybackFaderMocks);

	it("opens configuration when SHIFT is followed by the first playback button", () => {
		assignPlayback();
		mocks.state.shiftArmed = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("routes Update before playback execution with concrete playback and current Cue context", () => {
		assignPlayback();
		mocks.state.updateArmed = true;
		mocks.scopedCueLists[0].cues = [
			{
				id: "cue-2",
				number: 2,
				name: "Look",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		mocks.playbacks.active = [
			{
				playback_number: 7,
				cue_list_id: "front",
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
			},
		];
		const selected = vi.fn();
		window.addEventListener(UPDATE_TARGET_EVENT, selected);
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		expect((selected.mock.calls[0][0] as CustomEvent).detail).toEqual({
			family: { type: "cue" },
			object_id: "front",
			playback_number: 7,
			cue_id: "cue-2",
			cue_number: 2,
			validate_active_context: true,
		});
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		window.removeEventListener(UPDATE_TARGET_EVENT, selected);
	});
});

describe("PlaybackFaderBank selection and Record targets", () => {
	beforeEach(resetPlaybackFaderMocks);
	it("uses SELECT then any playback touch without firing the mapped button", async () => {
		assignPlayback();
		mocks.commandLine = "SELECT";
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "select", {
				surface: "physical",
			}),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(mocks.refresh).not.toHaveBeenCalled();
		expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
	});

	it("makes the hardware card one display-only Cuelist selection surface", async () => {
		assignPlayback();
		mocks.hardwareConnected = true;
		mocks.scopedCueLists[0].cues = [
			{
				id: "cue-1",
				number: 1,
				name: "Opening",
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		];
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(
			screen.queryByRole("button", { name: /Playback representation/ }),
		).not.toBeInTheDocument();
		const card = container.querySelector(".hardware-playback-card")!;
		fireEvent.click(card.querySelector("header b")!);
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "select", {
				surface: "physical",
			}),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN_CUELIST",
			number: 7,
		});
	});

	it("separates configured-color running, explicit selection, and empty states", () => {
		assignPlayback({ color: "#f6e58d" });
		mocks.playbacks.active = [
			{
				playback_number: 7,
				enabled: true,
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
			},
		];
		mocks.playbacks.selected_playback = 7;
		const { container } = render(<PlaybackFaderBank count={2} />);
		expect(container.querySelector('[data-playback-slot="1"]')).toHaveClass(
			"playback-colored",
			"running",
			"selected",
		);
		expect(container.querySelector('[data-playback-slot="1"]')).toHaveStyle({
			"--playback-color": "#f6e58d",
		});
		expect(container.querySelector('[data-playback-slot="2"]')).toHaveClass(
			"empty",
		);
		expect(container.querySelector('[data-playback-slot="2"]')).not.toHaveClass(
			"playback-colored",
			"running",
			"selected",
		);
	});

	it("does not select the hardware card when a real button or fader operates", () => {
		assignPlayback();
		mocks.hardwareConnected = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		fireEvent.input(
			screen.getByRole("slider", { name: "Page 1 playback 1 fader" }),
			{ target: { value: "42" } },
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "master", {
			value: 0.42,
			surface: "physical",
		});
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalledWith(7, "select", {
			surface: "physical",
		});
	});
});

describe("PlaybackFaderBank Record targets", () => {
	beforeEach(resetPlaybackFaderMocks);

	it.each([
		["touch", false],
		["hardware-connected", true],
	] as const)("makes the entire %s playback area one explicit-page Record target", async (_surface, hardware) => {
		assignPlayback();
		mocks.hardwareConnected = hardware;
		mocks.state.storeArmed = true;
		mocks.playbacks.pages.push({
			number: 3,
			name: "Page 3",
			slots: { "1": 7 },
		});
		const { container } = render(
			<PlaybackFaderBank pageNumber={3} count={1} />,
		);
		const card = container.querySelector("article")!;
		expect(card).toHaveClass("store-target");
		const surfaces = hardware
			? [
					card.querySelector("header")!,
					screen.getByRole("button", { name: "GO +" }),
					screen.getByRole("button", { name: "GO −" }),
					screen.getByRole("button", { name: "FLASH" }),
					screen.getByRole("slider", { name: "Page 3 playback 1 fader" }),
				]
			: [
					screen.getByRole("button", {
						name: "Playback representation page 3 playback 1",
					}),
					screen.getByRole("button", { name: "GO +" }),
					screen.getByRole("button", { name: "GO −" }),
					screen.getByRole("button", { name: "FLASH" }),
					screen.getByRole("slider", { name: "Master" }),
				];
		for (const surface of surfaces) {
			fireEvent.pointerDown(surface, { pointerId: 4 });
			fireEvent.click(surface);
		}
		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledTimes(surfaces.length),
		);
		for (const call of mocks.recordCue.mock.calls)
			expect(call).toEqual([
				{
					target: { kind: "page_slot", page: 3, slot: 1 },
					operation: "overwrite",
					timing: {},
					cueOnly: false,
					capturePolicy: "current_capture",
					activationPolicy: "go_to_if_normal",
				},
			]);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledTimes(surfaces.length);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
		mocks.playbacks.pages.pop();
	});

	it.each([
		["touch", false],
		["hardware-connected", true],
	] as const)("records an empty %s playback card instead of requiring a child control", async (_surface, hardware) => {
		mocks.hardwareConnected = hardware;
		Object.assign(mocks.state, {
			cueListSetTarget: null,
			cueListSetArmed: false,
			storeArmed: true,
		});
		const { container } = render(
			<PlaybackFaderBank pageNumber={4} count={1} />,
		);
		fireEvent.click(container.querySelector("article")!);
		await waitFor(() =>
			expect(mocks.recordCue).toHaveBeenCalledWith({
				target: { kind: "page_slot", page: 4, slot: 1 },
				operation: "overwrite",
				timing: {},
				cueOnly: false,
				capturePolicy: "current_capture",
				activationPolicy: "go_to_if_normal",
			}),
		);
		expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
	});

	it("keeps Record armed and the command intact when the typed action fails", async () => {
		mocks.state.storeArmed = true;
		mocks.recordCue.mockResolvedValueOnce(null);
		const { container } = render(
			<PlaybackFaderBank pageNumber={4} count={1} />,
		);

		fireEvent.click(container.querySelector("article")!);
		await waitFor(() => expect(mocks.recordCue).toHaveBeenCalledOnce());
		expect(mocks.dispatch).not.toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
		expect(mocks.resetCommandLine).not.toHaveBeenCalled();
	});
});

describe("PlaybackFaderBank action dispatch and persistence", () => {
	beforeEach(resetPlaybackFaderMocks);
	it("dispatches the configured index and the concrete held Flash lifetime", async () => {
		assignPlayback();
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass(
			"action-count-3",
		);
		fireEvent.click(screen.getByRole("button", { name: "GO +" }));
		expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		const flash = screen.getByRole("button", { name: "FLASH" });
		fireEvent.pointerDown(flash, { pointerId: 4 });
		fireEvent.pointerUp(flash, { pointerId: 4 });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "flash", {
				pressed: true,
				surface: "physical",
			}),
		);
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "flash", {
				pressed: false,
				surface: "physical",
			}),
		);
	});

	it.each(["cancel", "lost capture"])(
		"releases a held Flash once after pointer %s",
		async (release) => {
			assignPlayback();
			render(<PlaybackFaderBank count={1} />);
			const flash = screen.getByRole("button", { name: "FLASH" });
			fireEvent.pointerDown(flash, { pointerId: 4 });
			if (release === "cancel")
				fireEvent.pointerCancel(flash, { pointerId: 4 });
			else fireEvent.lostPointerCapture(flash, { pointerId: 4 });
			fireEvent.lostPointerCapture(flash, { pointerId: 4 });

			await waitFor(() =>
				expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2),
			);
			expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(7, "flash", {
				pressed: false,
				surface: "physical",
			});
		},
	);

	it("releases a held Flash when the bank unmounts", async () => {
		assignPlayback();
		const { unmount } = render(<PlaybackFaderBank count={1} />);
		fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), {
			pointerId: 4,
		});
		unmount();

		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(7, "flash", {
			pressed: false,
			surface: "physical",
		});
	});

	it("orders release after a delayed press and retains its original semantic", async () => {
		assignPlayback();
		let resolvePress!: (value: { status: string }) => void;
		mocks.poolPlaybackAction
			.mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						resolvePress = resolve;
					}),
			)
			.mockResolvedValue({ status: "no_change" });
		const { rerender } = render(<PlaybackFaderBank count={1} />);
		fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), {
			pointerId: 4,
		});
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "flash", {
				pressed: true,
				surface: "physical",
			}),
		);
		mocks.playbacks.pool = [
			playbackDefinition(7, { buttons: ["go", "go_minus", "swap"] }),
		];
		mocks.hardwareConnected = true;
		rerender(<PlaybackFaderBank count={1} />);

		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
		resolvePress({ status: "changed" });
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenLastCalledWith(7, "flash", {
				pressed: false,
				surface: "physical",
			}),
		);
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2);
		expect(screen.getByRole("button", { name: "SWAP" })).toBeInTheDocument();
	});

	it("omits disabled touch buttons while preserving configured button indices", async () => {
		assignPlayback({ buttons: ["go", "none", "flash"], button_count: 3 });
		const { container } = render(<PlaybackFaderBank count={1} />);
		expect(
			screen.queryByRole("button", { name: "DISABLED" }),
		).not.toBeInTheDocument();
		expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass(
			"action-count-2",
		);
		expect(
			container.querySelectorAll(".vertical-touch-fader-actions .ui-button"),
		).toHaveLength(2);
		fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), {
			pointerId: 4,
		});
		await waitFor(() =>
			expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "flash", {
				pressed: true,
				surface: "physical",
			}),
		);
	});

	it("makes one configured faderless touch button fill its playback section", () => {
		assignPlayback({
			buttons: ["flash", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		const { container } = render(<PlaybackFaderBank count={1} />);
		const action = screen.getByRole("button", { name: "FLASH" });
		expect(action).toHaveClass("single-button-playback-action");
		expect(action).toHaveTextContent("1 · Front Wash");
		expect(action).toHaveTextContent("FLASH");
		expect(
			container.querySelector(".faderless-playback-actions"),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "DISABLED" }),
		).not.toBeInTheDocument();
	});

	it.each([
		["two", ["go", "flash", "none"], 2],
		["three", ["go", "go_minus", "flash"], 3],
	] as const)("lays out %s faderless touch buttons side by side", (_count, actions, visibleCount) => {
		assignPlayback({
			buttons: actions,
			button_count: visibleCount,
			has_fader: false,
		});
		const { container } = render(<PlaybackFaderBank count={1} />);
		const actionRow = container.querySelector(".faderless-playback-actions")!;
		expect(actionRow).toHaveStyle({
			"--playback-action-count": String(visibleCount),
		});
		expect(actionRow.querySelectorAll(".ui-button")).toHaveLength(visibleCount);
		expect(
			container.querySelector(".single-button-playback-action"),
		).not.toBeInTheDocument();
	});
});

describe("PlaybackFaderBank faderless controls and runtime feedback", () => {
	beforeEach(resetPlaybackFaderMocks);

	it("dispatches TEMP as a press-to-toggle action on successive clicks", () => {
		assignPlayback({
			buttons: ["temp", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		render(<PlaybackFaderBank count={1} />);
		const temp = screen.getByRole("button", { name: "TEMP" });
		fireEvent.click(temp);
		fireEvent.click(temp);
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(1, 7, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(2, 7, "button", {
			button: 1,
			pressed: true,
			surface: "physical",
		});
		expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2);
	});

	it("renders only persisted controls for a one-button faderless playback", () => {
		assignPlayback({
			buttons: ["toggle", "none", "none"],
			button_count: 1,
			has_fader: false,
		});
		render(<PlaybackFaderBank count={1} />);
		expect(screen.getByRole("button", { name: "TOGGLE" })).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "GO −" }),
		).not.toBeInTheDocument();
		expect(screen.queryByRole("slider")).not.toBeInTheDocument();
	});

	it("shows X-fade direction/progress and safe-pickup feedback from runtime state", () => {
		assignPlayback({ fader: "x_fade" });
		mocks.playbacks.active = [
			{
				playback_number: 7,
				cue_list_id: "front",
				cue_index: 0,
				current_cue_number: 1,
				effective_next_cue_number: 2,
				enabled: true,
				master: 1,
				flash: false,
				fader_position: 0.25,
				fader_pickup_required: true,
				manual_xfade_position: 0.25,
				manual_xfade_direction: "towards_high",
				manual_xfade_progress: 0.25,
			},
		];
		render(<PlaybackFaderBank count={1} />);
		expect(screen.getByText("Pickup: lower to zero")).toBeInTheDocument();
		expect(screen.getByText("Cue 1 → 2 · 25%")).toBeInTheDocument();
	});

	it("recognizes the marked click produced by a playback right-click", () => {
		assignPlayback();
		const { container } = render(<PlaybackFaderBank count={1} />);
		const click = new MouseEvent("click", { bubbles: true, cancelable: true });
		Object.defineProperty(click, "lightSetShortcut", { value: true });
		fireEvent(container.querySelector("article")!, click);
		expect(
			screen.getByRole("dialog", { name: "Playback Configuration" }),
		).toBeInTheDocument();
	});

	it("clears atomically through None plus Apply", async () => {
		assignPlayback();
		mocks.state.playbackSetArmed = true;
		render(<PlaybackFaderBank count={1} />);
		fireEvent.click(
			screen.getByRole("button", {
				name: "Playback representation page 1 playback 1",
			}),
		);
		fireEvent.click(screen.getByRole("radio", { name: "None" }));
		expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(mocks.clearPlaybackSlot).toHaveBeenCalledWith(1, 1, {
				expectedPageRevision: 3,
				expectedPageObjectId: "page-object-1",
				expectedPlaybackRevision: 5,
				expectedPlaybackObjectId: "playback-object-7",
			}),
		);
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Playback Configuration" }),
			).not.toBeInTheDocument(),
		);
	});
});
