import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackSnapshot,
} from "../api/types";
import { PaneSettingsModal } from "../components/modals/PaneSettingsModal";
import { createCommandLineTestAuthority } from "../features/programmingInteraction/testing/commandLineTestAuthority";
import { CuelistWindow } from "./CuelistWindow";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	executeCommandLine: vi.fn(),
	setCommandLine: vi.fn(),
	refresh: vi.fn(),
	resetCommandLine: vi.fn(),
	recordCue: vi.fn(),
	saveTopologyCueList: vi.fn(),
	activeSaveTopologyCueList: null as ((...args: any[]) => any) | null,
	state: {
		activeDeskId: "desk-1",
		paneSettingsId: null as string | null,
		presetFamily: "Mixed" as const,
		storeArmed: true,
		cueListSetArmed: false,
		cueListSetTarget: null as number | null,
		desks: [
			{
				id: "desk-1",
				name: "Desk 1",
				panes: [
					{
						id: "cues-1",
						kind: "cues" as const,
						title: "Cues · Main",
						x: 1,
						y: 1,
						width: 12,
						height: 12,
					},
				],
			},
		],
	},
	playbacks: {
		pool: [] as PlaybackDefinition[],
		active: [] as PlaybackSnapshot["active"],
		pages: [],
		cue_lists: [] as CueList[],
		active_page: 1,
		selected_playback: null as number | null,
	},
	cueObjects: [] as Array<Record<string, unknown>>,
}));

vi.mock("../api/ServerContext", () => ({
	useServer: () => ({
		playbacks: mocks.playbacks,
		patch: { fixtures: [], revision: 0 },
		stageLayout: null,
		groups: [],
		readVisualization: vi.fn(),
		executeCommandLine: mocks.executeCommandLine,
		setCommandLine: mocks.setCommandLine,
		refresh: mocks.refresh,
		resetCommandLine: mocks.resetCommandLine,
		cueObjects: mocks.cueObjects,
	}),
}));
vi.mock("../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => ({
		saveCueList:
			mocks.activeSaveTopologyCueList ?? mocks.saveTopologyCueList,
	}),
}));
vi.mock("../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	useCueLists: () =>
		mocks.cueObjects.length
			? mocks.cueObjects
			: mocks.playbacks.cue_lists.map((body) => ({
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
		mocks.playbacks.pages.map((body: Record<string, unknown>) => ({
			kind: "playback_page",
			id: String(body.number),
			revision: 1,
			updated_at: "",
			body,
		})),
}));
vi.mock("../features/server/useShowObjectsState", () => ({
	useGroups: () => [],
}));

vi.mock("../state/AppContext", () => ({
	useApp: () => ({
		state: mocks.state,
		dispatch: mocks.dispatch,
	}),
}));

vi.mock("./stage3dScene", () => ({
	cueVisualization: vi.fn(),
	migrateStagePosition: vi.fn(),
	renderStageThumbnail: vi.fn(),
}));

function resetCuelistWindowMocks() {
	cleanup();
	mocks.dispatch.mockReset();
	mocks.executeCommandLine.mockReset().mockResolvedValue(true);
	mocks.setCommandLine.mockReset();
	mocks.refresh.mockReset().mockResolvedValue(undefined);
	mocks.resetCommandLine.mockReset().mockResolvedValue(true);
	mocks.recordCue.mockReset().mockResolvedValue({ status: "changed" });
	mocks.activeSaveTopologyCueList = null;
	mocks.saveTopologyCueList.mockReset().mockImplementation(
		(
			_cueListId: string,
			expectedRevision: number,
			expectedObjectId: string,
			body: CueList,
		) =>
			Promise.resolve(
				savedCueListOutcome(expectedObjectId, expectedRevision + 1, body),
			),
	);
	mocks.state.storeArmed = true;
	mocks.state.paneSettingsId = null;
	mocks.state.cueListSetArmed = false;
	mocks.state.cueListSetTarget = null;
	mocks.playbacks.pool = [];
	mocks.playbacks.cue_lists = [];
	mocks.playbacks.active = [];
	mocks.playbacks.selected_playback = null;
	mocks.cueObjects = [];
}

function editableCueList(): CueList {
	return {
		id: "main",
		name: "Main",
		priority: 10,
		mode: "sequence",
		looped: false,
		cues: [
			{
				id: "cue-1",
				number: 1,
				name: "Opening",
				fade_millis: 2_500,
				delay_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		],
	};
}

function showEditableCueList(cueList = editableCueList()) {
	mocks.state.storeArmed = false;
	mocks.playbacks.pool = [
		{
			number: 1,
			name: "Main",
			target: { type: "cue_list", cue_list_id: cueList.id },
			buttons: ["go", "go_minus", "flash"],
			fader: "master",
			go_activates: true,
			auto_off: true,
			xfade_millis: 0,
		},
	];
	mocks.cueObjects = [
		{ id: "legacy-main", revision: 3, body: cueList },
	];
	return cueList;
}

function savedCueListOutcome(
	objectId: string,
	objectRevision: number,
	body: CueList,
) {
	return {
		status: "changed",
		objects: [
			{
				kind: "cue_list",
				state: "present",
				objectId,
				objectRevision,
				body,
			},
		],
	};
}

describe("CuelistWindow Cue settings", () => {
	beforeEach(resetCuelistWindowMocks);

	it("keeps Cue rows selection-only and exposes the compact Cue settings grid", () => {
		let measure: ResizeObserverCallback = () => undefined;
		vi.stubGlobal(
			"ResizeObserver",
			class {
				constructor(callback: ResizeObserverCallback) {
					measure = callback;
				}
				observe() {}
				disconnect() {}
				unobserve() {}
			},
		);
		mocks.state.storeArmed = false;
		mocks.playbacks.pool = [
			{
				number: 1,
				name: "Main",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
		];
		mocks.playbacks.cue_lists = [
			{
				id: "main",
				name: "Main",
				priority: 10,
				mode: "sequence",
				looped: false,
				cues: [
					{
						number: 1,
						name: "Opening",
						fade_millis: 1000,
						delay_millis: 0,
						trigger: { type: "manual" },
						changes: [],
					},
				],
			},
		];
		render(<CuelistWindow />);
		fireEvent.click(screen.getByText("Main").closest("button")!);
		expect(
			screen.getByText("Cuelist View · Cuelist 1 · Main"),
		).toBeInTheDocument();
		expect(
			screen.getAllByRole("columnheader").map((cell) => cell.textContent),
		).toEqual(["Preview", "No.", "Name", "Trigger", "Fade"]);
		fireEvent.click(screen.getByText("Opening"));
		expect(
			screen.queryByRole("button", { name: "GO −" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "TOGGLE" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "OFF" }),
		).not.toBeInTheDocument();
		expect(screen.getByLabelText("Title")).toHaveValue("Opening");
		expect(
			screen.queryByRole("heading", { name: "Cue Settings" }),
		).not.toBeInTheDocument();
		expect(screen.getByText("Selected Cue · 1")).toHaveClass(
			"cue-selected-label",
		);
		expect(
			[
				...document.querySelectorAll(
					".cue-settings-grid-measure > .ui-form-field > label",
				),
			].map((label) => label.textContent),
		).toEqual(["Title", "Fade", "Delay", "Trigger"]);
		expect(
			screen.getByLabelText("Title").closest(".ui-form-field"),
		).toContainElement(screen.getByRole("button", { name: "Open keyboard" }));
		expect(
			screen.getByLabelText("Fade").closest(".ui-form-field"),
		).toContainElement(
			screen.getAllByRole("button", { name: "Open number pad" })[0],
		);
		expect(
			screen.getByRole("button", { name: "Open Trigger picker" }),
		).toBeInTheDocument();

		const sidebar = document.querySelector(".cue-properties") as HTMLElement;
		const preview = document.querySelector(
			".cue-selected-preview",
		) as HTMLElement;
		const fields = document.querySelector(
			".cue-settings-grid-measure",
		) as HTMLElement;
		Object.defineProperty(sidebar, "clientHeight", {
			configurable: true,
			value: 150,
		});
		Object.defineProperty(preview, "offsetHeight", {
			configurable: true,
			value: 74,
		});
		Object.defineProperty(fields, "scrollHeight", {
			configurable: true,
			value: 180,
		});
		act(() => measure([], {} as ResizeObserver));
		expect(
			screen.getByText("Press SET, then press an attribute value to edit it."),
		).toBeInTheDocument();
		act(() =>
			window.dispatchEvent(
				new CustomEvent("light:desk-action", { detail: "set" }),
			),
		);
		expect(
			screen.getByText("SET is active. Press an attribute value to edit it."),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Set Cue Fade" }));
		expect(screen.getByRole("dialog", { name: "Fade" })).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Close input" }));
		vi.unstubAllGlobals();
	});
});

describe("CuelistWindow pane selection", () => {
	beforeEach(resetCuelistWindowMocks);

	it("hides the selected-Cue sidebar when the pane preference is disabled", () => {
		mocks.state.storeArmed = false;
		mocks.playbacks.cue_lists = [
			{
				id: "main",
				name: "Main",
				priority: 10,
				mode: "sequence",
				looped: false,
				cues: [
					{
						number: 1,
						name: "Opening",
						fade_millis: 1000,
						delay_millis: 0,
						trigger: { type: "manual" },
						changes: [],
					},
				],
			},
		];

		const { container } = render(
			<CuelistWindow compact cueListTab="cues" showCueSidebar={false} />,
		);

		expect(container.querySelector(".sequence-layout")).not.toHaveClass(
			"with-cue-properties",
		);
		expect(container.querySelector(".cue-properties")).not.toBeInTheDocument();
		expect(within(container).getByRole("table")).toBeInTheDocument();
	});
});

describe("CuelistWindow fixed and selected playback sources", () => {
	beforeEach(resetCuelistWindowMocks);

	it("shows a fixed Cuelist or follows the desk's selected Cuelist playback", () => {
		mocks.state.storeArmed = false;
		mocks.playbacks.pool = [
			{
				number: 1,
				name: "Main",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
			{
				number: 2,
				name: "Encore",
				target: { type: "cue_list", cue_list_id: "encore" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
		];
		mocks.playbacks.cue_lists = [
			{
				id: "main",
				name: "Main",
				priority: 10,
				mode: "sequence",
				looped: false,
				cues: [
					{
						number: 1,
						name: "Main opening",
						fade_millis: 0,
						delay_millis: 0,
						trigger: { type: "manual" },
						changes: [],
					},
					{
						number: 2,
						name: "Main chase step",
						fade_millis: 0,
						delay_millis: 0,
						trigger: { type: "manual" },
						changes: [],
					},
				],
			},
			{
				id: "encore",
				name: "Encore",
				priority: 10,
				mode: "sequence",
				looped: false,
				cues: [
					{
						number: 1,
						name: "Encore look",
						fade_millis: 0,
						delay_millis: 0,
						trigger: { type: "manual" },
						changes: [],
					},
				],
			},
		];

		const view = render(
			<CuelistWindow
				compact
				cueListTab="cues"
				cueListSource="fixed"
				fixedCueListNumber={2}
			/>,
		);
		expect(within(view.container).getByText("Encore look")).toBeInTheDocument();
		expect(
			within(view.container).queryByText("Main opening"),
		).not.toBeInTheDocument();

		// No Playback runtime authority is mounted here, so follow-selection has
		// no selected Playback even though the legacy snapshot names one. Desk
		// projection coverage lives in cuelistPlaybackAuthority.test.tsx.
		mocks.playbacks.selected_playback = 1;
		mocks.playbacks.active = [
			{
				playback_number: 1,
				cue_list_id: "main",
				cue_index: 1,
				paused: false,
				master: 1,
				flash: false,
			},
		];
		view.rerender(
			<CuelistWindow
				compact
				cueListTab="cues"
				cueListSource="follow-selection"
			/>,
		);
		expect(
			within(view.container).getByText("No Cuelist selected"),
		).toBeInTheDocument();
		expect(
			within(view.container).queryByText("Main opening"),
		).not.toBeInTheDocument();
	});
});

describe("CuelistWindow pane and Cuelist settings", () => {
	beforeEach(resetCuelistWindowMocks);

	it("offers the persisted sidebar switch in Cues pane settings", () => {
		mocks.state.paneSettingsId = "cues-1";
		mocks.playbacks.pool = [
			{
				number: 7,
				name: "Main",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
		];
		render(<PaneSettingsModal />);

		fireEvent.click(screen.getByRole("tab", { name: "Cues" }));
		fireEvent.click(screen.getByRole("radio", { name: "Follow selection" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_CUELIST",
			id: "cues-1",
			source: "follow-selection",
		});
		expect(
			screen.getByRole("button", { name: "7 · Main" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("switch", { name: "Show Cue sidebar" }));

		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_CUE_SIDEBAR",
			id: "cues-1",
			value: false,
		});
	});

	it("opens Cuelist Settings as a title-controlled modal and confirms dirty close", () => {
		mocks.state.storeArmed = false;
		const cueList: CueList = {
			id: "main",
			name: "Main",
			priority: 10,
			mode: "sequence",
			looped: false,
			cues: [
				{
					number: 1,
					name: "Opening",
					fade_millis: 1000,
					delay_millis: 0,
					trigger: { type: "manual" },
					changes: [],
				},
			],
		};
		mocks.playbacks.pool = [
			{
				number: 1,
				name: "Main",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
		];
		mocks.cueObjects = [{ id: "main", revision: 3, body: cueList }];

		const { container } = render(<CuelistWindow />);
		const ui = within(container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.click(ui.getByRole("button", { name: "Cuelist Settings" }));

		const settings = screen.getByRole("dialog", { name: "Cuelist Settings" });
		const sidebar = container.querySelector(".cue-properties")!;
		expect(sidebar).not.toContainElement(settings);
		expect(ui.getByRole("table")).toBeInTheDocument();
		expect(
			ui.queryByRole("heading", { name: "Cue Settings" }),
		).not.toBeInTheDocument();
		expect(
			within(settings)
				.getByRole("button", { name: "Save" })
				.closest(".ui-modal-titlebar"),
		).toBeInTheDocument();
		expect(
			within(settings).queryByRole("button", { name: "Cancel" }),
		).not.toBeInTheDocument();
		expect(
			within(settings)
				.getAllByRole("heading", { level: 3 })
				.map((heading) => heading.textContent),
		).toEqual(["Priority", "Restart behavior", "Timing"]);
		const mode = within(settings).getByRole("button", {
			name: /Mode\s*\(Sequence\)/,
		});
		fireEvent.click(mode);
		fireEvent.click(
			within(settings).getByRole("menuitemradio", { name: "Chaser" }),
		);
		expect(
			within(settings).getByRole("button", { name: /Mode\s*\(Chaser\)/ }),
		).toBeInTheDocument();
		expect(within(settings).getByLabelText("Speed multiplier")).toHaveAttribute(
			"inputmode",
			"decimal",
		);
		expect(
			within(settings).getByRole("slider", { name: "Chaser X-fade" }),
		).toHaveAttribute("max", "100");

		fireEvent.change(within(settings).getByLabelText("Numeric priority"), {
			target: { value: "11" },
		});
		fireEvent.click(
			within(settings).getByRole("button", { name: "Close Cuelist Settings" }),
		);
		const confirmation = screen.getByRole("dialog", {
			name: "Unsaved Cuelist Settings",
		});
		fireEvent.click(within(confirmation).getByRole("button", { name: "Stay" }));
		expect(settings).toBeInTheDocument();
		fireEvent.click(
			within(settings).getByRole("button", { name: "Close Cuelist Settings" }),
		);
		fireEvent.click(
			within(
				screen.getByRole("dialog", { name: "Unsaved Cuelist Settings" }),
			).getByRole("button", { name: "Discard changes" }),
		);
		expect(
			screen.queryByRole("dialog", { name: "Cuelist Settings" }),
		).not.toBeInTheDocument();
		expect(
			ui.queryByRole("heading", { name: "Cue Settings" }),
		).not.toBeInTheDocument();
		expect(ui.getByText("Selected Cue · 1")).toBeInTheDocument();
		expect(mocks.saveTopologyCueList).not.toHaveBeenCalled();
	});
});

describe("CuelistWindow Cue draft validation", () => {
	beforeEach(resetCuelistWindowMocks);

	it("does not let a late server refresh clobber an invalid Cue draft before validation", async () => {
		mocks.state.storeArmed = false;
		const cueList: CueList = {
			id: "main",
			name: "Main",
			priority: 10,
			mode: "sequence",
			looped: false,
			cues: [
				{
					id: "cue-1",
					number: 1,
					name: "Opening",
					fade_millis: 2_500,
					delay_millis: 0,
					trigger: { type: "manual" },
					changes: [],
				},
			],
		};
		mocks.playbacks.pool = [
			{
				number: 1,
				name: "Main",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
		];
		mocks.cueObjects = [{ id: "main", revision: 1, body: cueList }];
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		const fade = ui.getByLabelText("Fade");
		fireEvent.change(fade, { target: { value: "-1" } });

		mocks.cueObjects = [
			{
				id: "main",
				revision: 1,
				body: { ...cueList, cues: cueList.cues.map((cue) => ({ ...cue })) },
			},
		];
		view.rerender(<CuelistWindow />);
		expect(ui.getByLabelText("Fade")).toHaveValue("-1");

		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });
		expect(await ui.findByRole("alert")).toHaveTextContent(
			"Cue edit was not saved",
		);
		expect(mocks.saveTopologyCueList).not.toHaveBeenCalled();
	});

	it("saves an inline Cue through its captured topology identity without a broad refresh", async () => {
		const cueList = showEditableCueList();
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.change(ui.getByLabelText("Fade"), { target: { value: "3" } });

		mocks.cueObjects = [
			{
				id: "replacement-main",
				revision: 9,
				body: { ...cueList, name: "Concurrent" },
			},
		];
		view.rerender(<CuelistWindow />);
		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });

		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());
		expect(mocks.saveTopologyCueList).toHaveBeenCalledWith(
			"main",
			3,
			"legacy-main",
			expect.objectContaining({
				id: "main",
				name: "Main",
				cues: [expect.objectContaining({ id: "cue-1", fade_millis: 3_000 })],
			}),
		);
		expect(mocks.refresh).not.toHaveBeenCalled();
	});

	it("rebases queued inline edits onto the preceding authoritative outcome", async () => {
		showEditableCueList();
		let resolveFirst!: (outcome: ReturnType<typeof savedCueListOutcome>) => void;
		const first = new Promise<ReturnType<typeof savedCueListOutcome>>(
			(resolve) => {
				resolveFirst = resolve;
			},
		);
		mocks.saveTopologyCueList
			.mockReset()
			.mockImplementationOnce(() => first)
			.mockImplementationOnce(
				(
					_cueListId: string,
					expectedRevision: number,
					expectedObjectId: string,
					body: CueList,
				) =>
					Promise.resolve(
						savedCueListOutcome(
							expectedObjectId,
							expectedRevision + 1,
							body,
						),
					),
			);
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.change(ui.getByLabelText("Fade"), { target: { value: "3" } });
		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });
		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());

		fireEvent.change(ui.getByLabelText("Delay"), { target: { value: "2" } });
		fireEvent.keyDown(ui.getByLabelText("Delay"), { key: "Enter" });
		expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce();
		const firstBody = mocks.saveTopologyCueList.mock.calls[0][3] as CueList;
		act(() =>
			resolveFirst(savedCueListOutcome("legacy-main", 4, firstBody)),
		);

		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledTimes(2),
		);
		expect(mocks.saveTopologyCueList.mock.calls[1]).toEqual([
			"main",
			4,
			"legacy-main",
			expect.objectContaining({
				cues: [
					expect.objectContaining({
						fade_millis: 3_000,
						delay_millis: 2_000,
					}),
				],
			}),
		]);
	});

	it("cancels later queued edits after failure and lets the operator retry on repaired authority", async () => {
		const cueList = showEditableCueList();
		let resolveFirst!: (outcome: null) => void;
		const first = new Promise<null>((resolve) => {
			resolveFirst = resolve;
		});
		mocks.saveTopologyCueList
			.mockReset()
			.mockImplementationOnce(() => first)
			.mockImplementationOnce(
				(
					_cueListId: string,
					expectedRevision: number,
					expectedObjectId: string,
					body: CueList,
				) =>
					Promise.resolve(
						savedCueListOutcome(
							expectedObjectId,
							expectedRevision + 1,
							body,
						),
					),
			);
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.change(ui.getByLabelText("Fade"), { target: { value: "3" } });
		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });
		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());
		fireEvent.change(ui.getByLabelText("Delay"), { target: { value: "2" } });
		fireEvent.keyDown(ui.getByLabelText("Delay"), { key: "Enter" });

		mocks.cueObjects = [
			{
				id: "legacy-main",
				revision: 4,
				body: { ...cueList, name: "Concurrent repair" },
			},
		];
		view.rerender(<CuelistWindow />);
		act(() => resolveFirst(null));
		await waitFor(() =>
			expect(ui.getByRole("alert")).toHaveTextContent("revision conflict"),
		);
		expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce();

		fireEvent.keyDown(ui.getByLabelText("Delay"), { key: "Enter" });
		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledTimes(2),
		);
		expect(mocks.saveTopologyCueList.mock.calls[1]).toEqual([
			"main",
			4,
			"legacy-main",
			expect.objectContaining({
				name: "Concurrent repair",
				cues: [
					expect.objectContaining({
						fade_millis: 3_000,
						delay_millis: 2_000,
					}),
				],
			}),
		]);
	});

	it("isolates a replacement writer from a late same-object response", async () => {
		showEditableCueList();
		let resolveOld!: (outcome: null) => void;
		const oldResponse = new Promise<null>((resolve) => {
			resolveOld = resolve;
		});
		mocks.saveTopologyCueList.mockReset().mockImplementationOnce(() => oldResponse);
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.change(ui.getByLabelText("Fade"), { target: { value: "3" } });
		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });
		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());

		const replacementWriter = vi.fn(
			(
				_cueListId: string,
				expectedRevision: number,
				expectedObjectId: string,
				body: CueList,
			) =>
				Promise.resolve(
					savedCueListOutcome(
						expectedObjectId,
						expectedRevision + 1,
						body,
					),
				),
		);
		mocks.activeSaveTopologyCueList = replacementWriter;
		view.rerender(<CuelistWindow />);
		fireEvent.change(ui.getByLabelText("Delay"), { target: { value: "2" } });
		fireEvent.keyDown(ui.getByLabelText("Delay"), { key: "Enter" });
		await waitFor(() => expect(replacementWriter).toHaveBeenCalledOnce());

		act(() => resolveOld(null));
		await act(async () => Promise.resolve());
		expect(ui.queryByRole("alert")).not.toBeInTheDocument();
		expect(replacementWriter).toHaveBeenCalledOnce();
	});

	it("clears a stale repair marker after retry instead of rebasing a later dirty draft", async () => {
		showEditableCueList();
		mocks.saveTopologyCueList
			.mockReset()
			.mockResolvedValueOnce(null)
			.mockImplementation(
				(
					_cueListId: string,
					expectedRevision: number,
					expectedObjectId: string,
					body: CueList,
				) =>
					Promise.resolve(
						savedCueListOutcome(
							expectedObjectId,
							expectedRevision + 1,
							body,
						),
					),
			);
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.change(ui.getByLabelText("Fade"), { target: { value: "3" } });
		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });
		await ui.findByRole("alert");
		fireEvent.keyDown(ui.getByLabelText("Fade"), { key: "Enter" });
		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledTimes(2),
		);

		const retriedBody = mocks.saveTopologyCueList.mock.calls[1][3] as CueList;
		mocks.cueObjects = [
			{ id: "legacy-main", revision: 4, body: retriedBody },
		];
		view.rerender(<CuelistWindow />);
		fireEvent.change(ui.getByLabelText("Delay"), { target: { value: "2" } });
		mocks.cueObjects = [
			{
				id: "legacy-main",
				revision: 5,
				body: { ...retriedBody, name: "Later concurrent change" },
			},
		];
		view.rerender(<CuelistWindow />);
		fireEvent.keyDown(ui.getByLabelText("Delay"), { key: "Enter" });

		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledTimes(3),
		);
		expect(mocks.saveTopologyCueList.mock.calls[2]).toEqual([
			"main",
			4,
			"legacy-main",
			expect.objectContaining({
				name: "Main",
				cues: [expect.objectContaining({ delay_millis: 2_000 })],
			}),
		]);
	});
});

describe("CuelistWindow topology-backed Cuelist settings", () => {
	beforeEach(resetCuelistWindowMocks);

	it("saves settings against the modal-open storage identity and revision", async () => {
		const cueList = showEditableCueList();
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.click(ui.getByRole("button", { name: "Cuelist Settings" }));
		const settings = screen.getByRole("dialog", { name: "Cuelist Settings" });
		fireEvent.change(within(settings).getByLabelText("Numeric priority"), {
			target: { value: "11" },
		});

		mocks.cueObjects = [
			{
				id: "replacement-main",
				revision: 9,
				body: { ...cueList, name: "Concurrent" },
			},
		];
		view.rerender(<CuelistWindow />);
		fireEvent.click(within(settings).getByRole("button", { name: "Save" }));

		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());
		expect(mocks.saveTopologyCueList).toHaveBeenCalledWith(
			"main",
			3,
			"legacy-main",
			expect.objectContaining({ name: "Main", priority: 11 }),
		);
		expect(mocks.refresh).not.toHaveBeenCalled();
	});

	it("renumbers the complete Cuelist in one topology action", async () => {
		showEditableCueList();
		render(<CuelistWindow />);
		fireEvent.click(screen.getByText("Main").closest("button")!);
		fireEvent.click(screen.getByRole("button", { name: "Cuelist Settings" }));
		const settings = screen.getByRole("dialog", { name: "Cuelist Settings" });
		fireEvent.click(within(settings).getByRole("button", { name: "Renumber Cues" }));
		const renumber = screen.getByRole("dialog", { name: "Renumber Cues" });
		fireEvent.change(within(renumber).getByLabelText("Start Cue"), {
			target: { value: "10" },
		});
		fireEvent.click(within(renumber).getByRole("button", { name: "Renumber" }));

		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());
		expect(mocks.saveTopologyCueList).toHaveBeenCalledWith(
			"main",
			3,
			"legacy-main",
			expect.objectContaining({
				cues: [expect.objectContaining({ id: "cue-1", number: 10 })],
			}),
		);
		expect(mocks.refresh).not.toHaveBeenCalled();
	});
});

describe("CuelistWindow pool recording", () => {
	beforeEach(resetCuelistWindowMocks);

	it("renders empty numbered slots and records into the touched slot", async () => {
		const authority = createCommandLineTestAuthority({ text: "STORE" });
		render(authority.wrap(<CuelistWindow compact cueListTab="pool" />));
		await act(authority.settle);
		expect(screen.getAllByText("Tap to record Cuelist")).toHaveLength(1000);
		fireEvent.click(
			screen.getAllByText("Tap to record Cuelist")[0].closest("button")!,
		);
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
		expect(mocks.executeCommandLine).not.toHaveBeenCalled();
		expect(mocks.refresh).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(authority.writes).toEqual([
				{
					deskId: authority.deskId,
					text: "",
					expectedRevision: 1,
				},
			]),
		);
		expect(mocks.resetCommandLine).not.toHaveBeenCalled();
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_STORE_ARMED",
			value: false,
		});
	});

	it("selects an existing pool playback as the next Set assignment source", () => {
		mocks.state.storeArmed = false;
		mocks.state.cueListSetArmed = true;
		mocks.playbacks.pool = [
			{
				number: 7,
				name: "Main sequence",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
		];
		render(<CuelistWindow compact cueListTab="pool" />);
		fireEvent.click(screen.getByText("Main sequence").closest("button")!);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_TARGET",
			value: 7,
		});
	});

	it("shows the Set workflow in the header's secondary amber status line", () => {
		mocks.state.storeArmed = false;
		mocks.state.cueListSetArmed = true;
		const { container } = render(<CuelistWindow />);
		const status = container.querySelector(".cuelist-workflow-status")!;
		expect(status).toHaveTextContent(
			"Select a Cuelist, then touch the playback fader where it should be assigned.",
		);
		expect(status).toHaveClass("cuelist-workflow-status");
		expect(status.closest("small")).toBe(
			container.querySelector(".ui-window-info small"),
		);
		expect(container.querySelector(".pool-message")).toBeNull();
	});
});
