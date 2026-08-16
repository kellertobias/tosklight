import {
	act,
	cleanup,
	fireEvent,
	render as rtlRender,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackSnapshot,
} from "../api/types";
import { PaneSettingsModal } from "../components/modals/PaneSettingsModal";
import { createCommandLineTestAuthority } from "../features/programmingInteraction/testing/commandLineTestAuthority";
import { CuelistWindow } from "./CuelistWindow";

const render = (ui: Parameters<typeof rtlRender>[0]) =>
	rtlRender(ui, { wrapper: ModalProvider });

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
	showObjectsStore: {
		getSnapshot: () => ({ authorityGeneration: 1 }),
		subscribe: () => () => undefined,
	},
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
		saveCueList: mocks.activeSaveTopologyCueList ?? mocks.saveTopologyCueList,
	}),
}));
vi.mock(
	"../features/playbackRuntime/PlaybackRuntimeView",
	async (importOriginal) => {
		const actual = await importOriginal<
			typeof import("../features/playbackRuntime/PlaybackRuntimeView")
		>();
		return {
			...actual,
			useCueListRuntime: (
				cueListId: string | null | undefined,
				playbackNumber?: number | null,
			) =>
				mocks.playbacks.active.find(
					(entry) =>
						entry.cue_list_id === cueListId &&
						(playbackNumber == null || entry.playback_number === playbackNumber),
				),
		};
	},
);
vi.mock("../features/cueRecording/CueRecordingProvider", () => ({
	useCueRecording: () => ({ record: mocks.recordCue }),
}));
vi.mock("../features/showObjects/ShowObjectsState", () => ({
	usePortableGroups: () => [],
	useShowObjectCollectionsReady: () => true,
	useShowObjectsStore: () => mocks.showObjectsStore,
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
	mocks.saveTopologyCueList
		.mockReset()
		.mockImplementation(
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
				number: "1",
				name: "Opening",
				information: "House opens",
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
	mocks.cueObjects = [{ id: "legacy-main", revision: 3, body: cueList }];
	return cueList;
}

function appendDirectKeyboardValue(next: string) {
	for (const key of next) fireEvent.keyDown(window, { key });
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

	it("opens exact Cue property modals from table cells without a sidebar", () => {
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
						id: "cue-opening",
						number: "1",
						name: "Opening",
						fade_millis: 1000,
						delay_millis: 0,
						trigger: { type: "manual" },
						actions: [
							{ type: "jump", cue_id: "cue-opening", count: 2 },
						],
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
		).toEqual([
			"Preview",
			"No.",
			"Name",
			"Info",
			"Jump",
			"Jump Count",
			"Trigger",
			"Trigger Time",
			"In Delay",
			"In Fade",
			"Out Delay",
			"Out Fade",
		]);
		expect(document.querySelector(".cue-properties")).not.toBeInTheDocument();
		for (const name of [
			"Cue Name",
			"Cue Information",
			"Jump",
			"Jump Count",
			"Trigger",
			"Trigger Time",
			"In Delay",
			"In Fade",
			"Out Delay",
			"Out Fade",
		]) {
			fireEvent.click(screen.getAllByRole("button", { name })[0]);
			const dialog = screen.getByRole("dialog", { name });
			expect(dialog).toHaveClass(
				name === "Cue Name" || name === "Cue Information"
					? "keyboard-modal"
					: name === "Jump" || name === "Trigger"
						? "ui-grouped-selection-modal"
						: "direct-value-modal",
			);
			fireEvent.click(
				within(dialog).getByRole("button", {
					name:
						name === "Cue Name" || name === "Cue Information"
							? "Close input"
							: `Close ${name}`,
				}),
			);
		}
	});

	it("shows a Link destination by stable Cue identity in its Trigger modal", () => {
		const destination = {
			id: "cue-blackout",
			number: "12",
			name: "Blackout",
			fade_millis: 0,
			delay_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		};
		const cueList = editableCueList();
		cueList.cues[0].trigger = {
			type: "link",
			cue_id: destination.id,
			delay_millis: 250,
		};
		cueList.cues.push(destination);
		showEditableCueList(cueList);
		const view = render(<CuelistWindow />);
		const ui = within(view.container);
		fireEvent.click(ui.getByText("Main").closest("button")!);

		expect(ui.getByText("LINK → Cue 12 · Blackout")).toBeInTheDocument();
		fireEvent.click(
			ui.getByText("LINK → Cue 12 · Blackout").closest("button")!,
		);
		const modal = screen.getByRole("dialog", { name: "Trigger" });
		expect(
			within(modal).getByRole("button", {
				name: /LINK → Cue 12 · Blackout/,
			}),
		).toBeInTheDocument();
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
						number: "1",
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

	it("uses compact rows without the Preview column or a sidebar", () => {
		mocks.state.storeArmed = false;
		mocks.playbacks.cue_lists = [editableCueList()];
		const { container } = render(
			<CuelistWindow
				compact
				cueListTab="cues"
				cueListCompactRows
				thumbnails={{ 0: "data:image/png;base64,preview" }}
			/>,
		);

		expect(
			within(container)
				.getAllByRole("columnheader")
				.map((cell) => cell.textContent),
		).toEqual([
			"No.",
			"Name",
			"Info",
			"Jump",
			"Jump Count",
			"Trigger",
			"Trigger Time",
			"In Delay",
			"In Fade",
			"Out Delay",
			"Out Fade",
		]);
		expect(container.querySelector(".cue-table")).toHaveClass(
			"compact-cue-rows",
		);
		expect(container.querySelector(".cue-table img")).not.toBeInTheDocument();
		expect(container.querySelector(".cue-properties")).not.toBeInTheDocument();
	});

	it("opens and closes a Cue preview modal through the thumbnail click path", () => {
		showEditableCueList();
		const { container } = render(
			<CuelistWindow
				compact
				cueListTab="cues"
				thumbnails={{ 0: "data:image/png;base64,preview" }}
			/>,
		);
		const preview = within(container).getByRole("button", {
			name: "Open Cue 1 preview",
		});
		fireEvent.pointerDown(preview, { pointerType: "touch" });
		fireEvent.pointerUp(preview, { pointerType: "touch" });
		fireEvent.click(preview);

		const modal = screen.getByRole("dialog", {
			name: "Cue 1 preview image",
		});
		expect(within(modal).getByRole("img", { name: "Cue 1 preview" })).toHaveAttribute(
			"src",
			"data:image/png;base64,preview",
		);
		fireEvent.click(
			within(modal).getByRole("button", { name: "Close Cue preview" }),
		);
		expect(
			screen.queryByRole("dialog", { name: "Cue 1 preview image" }),
		).toBeNull();
	});

	it("shows configured current or next Cue information at the bottom", () => {
		const cueList = editableCueList();
		cueList.cues.push({
			...cueList.cues[0],
			id: "cue-2",
			number: "2",
			name: "Blackout",
			information: "Clear the stage",
		});
		showEditableCueList(cueList);
		mocks.playbacks.active = [
			{
				playback_number: 1,
				cue_list_id: "main",
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
				effective_next_cue_number: "2",
			},
		];
		const view = render(
			<CuelistWindow
				compact
				cueListTab="cues"
				cueInformationBlock="current"
			/>,
		);
		expect(
			within(view.container).getByRole("region", {
				name: "Current Cue Information",
			}),
		).toHaveTextContent("House opens");

		view.rerender(
			<CuelistWindow
				compact
				cueListTab="cues"
				cueInformationBlock="next"
			/>,
		);
		expect(
			within(view.container).getByRole("region", {
				name: "Next Cue Information",
			}),
		).toHaveTextContent("Clear the stage");
	});
});

describe("CuelistWindow fixed and selected playback sources", () => {
	beforeEach(resetCuelistWindowMocks);

	it("uses the stable fixed Cuelist identity without fallback and keeps Cue rows passive", () => {
		mocks.state.storeArmed = false;
		mocks.cueObjects = [
			{
				id: "object-main",
				revision: 1,
				body: {
					...editableCueList(),
					id: "main",
					cues: [
						{
							number: 1,
							name: "Main opening",
							fade_millis: 0,
							delay_millis: 0,
							trigger: { type: "manual" },
							changes: [],
						},
					],
				},
			},
		];
		const view = render(
			<CuelistWindow
				compact
				viewOnly
				cueListTab="cues"
				cueListSource="fixed"
				fixedCueListId="main"
			/>,
		);
		const row = within(view.container).getByText("Main opening").closest("tr");
		expect(row).toHaveAttribute("aria-disabled", "true");
		expect(row).not.toHaveAttribute("tabindex");
		if (!row) throw new Error("Expected the fixed Cuelist row");
		fireEvent.click(row);
		expect(row).not.toHaveClass("selected");

		view.rerender(
			<CuelistWindow
				compact
				viewOnly
				cueListTab="cues"
				cueListSource="fixed"
				fixedCueListId="missing"
			/>,
		);
		expect(
			within(view.container).getByText("Fixed Cuelist is unavailable"),
		).toBeInTheDocument();
		expect(
			within(view.container).queryByText("Main opening"),
		).not.toBeInTheDocument();
	});

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
						number: "1",
						name: "Main opening",
						fade_millis: 0,
						delay_millis: 0,
						trigger: { type: "manual" },
						changes: [],
					},
					{
						number: "2",
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
						number: "1",
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

	it("offers Cuelist selection and compact rows without a sidebar switch", () => {
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
		expect(
			screen.queryByRole("switch", { name: "Cue sidebar" }),
		).not.toBeInTheDocument();

		fireEvent.click(screen.getByRole("switch", { name: "Compact Cue rows" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_CUELIST_COMPACT_ROWS",
			id: "cues-1",
			value: true,
		});
		fireEvent.click(screen.getByRole("button", { name: "Off" }));
		fireEvent.click(screen.getByRole("option", { name: "Next Cue" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_CUE_INFORMATION_BLOCK",
			id: "cues-1",
			value: "next",
		});
	});

	it("closes untouched Cuelist Settings without a discard prompt", () => {
		showEditableCueList();
		const { container } = render(<CuelistWindow />);
		const ui = within(container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.click(ui.getByRole("button", { name: "Cuelist Settings" }));

		const settings = screen.getByRole("dialog", { name: "Cuelist Settings" });
		fireEvent.click(
			within(settings).getByRole("button", {
				name: "Close Cuelist Settings",
			}),
		);

		expect(
			screen.queryByRole("dialog", { name: "Unsaved Cuelist Settings" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("dialog", { name: "Cuelist Settings" }),
		).not.toBeInTheDocument();
	});

	it("closes restored normalized Cuelist Settings without a discard prompt", () => {
		showEditableCueList();
		const { container } = render(<CuelistWindow />);
		const ui = within(container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.click(ui.getByRole("button", { name: "Cuelist Settings" }));

		const settings = screen.getByRole("dialog", { name: "Cuelist Settings" });
		fireEvent.click(
			within(settings).getByRole("button", { name: /Mode\s*\(Sequence\)/ }),
		);
		fireEvent.click(
			within(settings).getByRole("menuitemradio", { name: "Chaser" }),
		);
		fireEvent.click(
			within(settings).getByRole("button", { name: /Mode\s*\(Chaser\)/ }),
		);
		fireEvent.click(
			within(settings).getByRole("menuitemradio", { name: "Sequence" }),
		);
		fireEvent.change(within(settings).getByLabelText("Numeric priority"), {
			target: { value: "11" },
		});
		fireEvent.change(within(settings).getByLabelText("Numeric priority"), {
			target: { value: "10" },
		});
		fireEvent.click(
			within(settings).getByRole("button", {
				name: "Close Cuelist Settings",
			}),
		);

		expect(
			screen.queryByRole("dialog", { name: "Unsaved Cuelist Settings" }),
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole("dialog", { name: "Cuelist Settings" }),
		).not.toBeInTheDocument();
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
					number: "1",
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
		expect(container.querySelector(".cue-properties")).not.toBeInTheDocument();
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
		fireEvent.pointerDown(confirmation.closest("[data-modal-id]")!);
		expect(
			screen.queryByRole("dialog", { name: "Unsaved Cuelist Settings" }),
		).not.toBeInTheDocument();
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
		expect(ui.getByRole("table")).toBeInTheDocument();
		expect(container.querySelector(".cue-properties")).not.toBeInTheDocument();
		expect(mocks.saveTopologyCueList).not.toHaveBeenCalled();
	});
});

describe("CuelistWindow Cue property transactions", () => {
	beforeEach(resetCuelistWindowMocks);

	it("discards Cancel and saves one modal draft through captured topology authority", async () => {
		const cueList = showEditableCueList();
		const { container } = render(<CuelistWindow />);
		const ui = within(container);
		fireEvent.click(ui.getByText("Main").closest("button")!);

		fireEvent.click(ui.getByRole("button", { name: "In Fade" }));
		let modal = screen.getByRole("dialog", { name: "In Fade" });
		fireEvent.click(
			within(modal).getByRole("button", { name: "Close In Fade" }),
		);
		expect(mocks.saveTopologyCueList).not.toHaveBeenCalled();

		fireEvent.click(ui.getByRole("button", { name: "In Fade" }));
		modal = screen.getByRole("dialog", { name: "In Fade" });
		fireEvent.click(within(modal).getByRole("button", { name: "3" }));
		fireEvent.click(within(modal).getByRole("button", { name: "ENTER" }));

		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce(),
		);
		expect(mocks.saveTopologyCueList).toHaveBeenCalledWith(
			cueList.id,
			3,
			"legacy-main",
			expect.objectContaining({
				cues: [expect.objectContaining({ id: "cue-1", fade_millis: 3_000 })],
			}),
		);
	});

	it("keeps the property modal open when the authoritative write fails", async () => {
		showEditableCueList();
		mocks.saveTopologyCueList.mockReset().mockResolvedValue(null);
		const { container } = render(<CuelistWindow />);
		const ui = within(container);
		fireEvent.click(ui.getByText("Main").closest("button")!);
		fireEvent.click(ui.getByRole("button", { name: "In Fade" }));
		const modal = screen.getByRole("dialog", { name: "In Fade" });
		fireEvent.click(within(modal).getByRole("button", { name: "3" }));
		fireEvent.click(within(modal).getByRole("button", { name: "ENTER" }));

		expect(await within(modal).findByRole("alert")).toHaveTextContent(
			"Cue edit was not saved",
		);
		expect(modal).toBeInTheDocument();
	});

	it("saves Cue name and information through the topology authority", async () => {
		const cueList = showEditableCueList();
		const { container } = render(<CuelistWindow />);
		const ui = within(container);
		fireEvent.click(ui.getByText("Main").closest("button")!);

		fireEvent.click(ui.getByRole("button", { name: "Cue Name" }));
		appendDirectKeyboardValue(" Reprise");
		fireEvent.keyDown(window, { key: "Enter" });
		await waitFor(() => expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce());
		expect(mocks.saveTopologyCueList).toHaveBeenLastCalledWith(
			cueList.id,
			3,
			"legacy-main",
			expect.objectContaining({
				cues: [expect.objectContaining({ name: "Opening Reprise" })],
			}),
		);

		fireEvent.click(ui.getByRole("button", { name: "Cue Information" }));
		appendDirectKeyboardValue(" · Stand by follow spot");
		fireEvent.click(screen.getByRole("button", { name: "Done" }));
		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledTimes(2),
		);
		expect(mocks.saveTopologyCueList).toHaveBeenLastCalledWith(
			cueList.id,
			4,
			"legacy-main",
			expect.objectContaining({
				cues: [
					expect.objectContaining({
						information: "House opens · Stand by follow spot",
					}),
				],
			}),
		);
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

		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce(),
		);
		expect(mocks.saveTopologyCueList).toHaveBeenCalledWith(
			"main",
			3,
			"legacy-main",
			expect.objectContaining({ name: "Main", priority: 11 }),
		);
		expect(mocks.refresh).not.toHaveBeenCalled();
	});

	it("renumbers the complete Cuelist in one topology action", async () => {
		const cueList = editableCueList();
		cueList.cues.push({
			...cueList.cues[0],
			id: "cue-2",
			number: "2.5",
			name: "Look",
			fade_millis: 4_000,
		});
		showEditableCueList(cueList);
		render(<CuelistWindow />);
		fireEvent.click(screen.getByText("Main").closest("button")!);
		fireEvent.click(screen.getByRole("button", { name: "Cuelist Settings" }));
		const settings = screen.getByRole("dialog", { name: "Cuelist Settings" });
		fireEvent.click(
			within(settings).getByRole("button", { name: "Renumber Cues" }),
		);
		let renumber = screen.getByRole("dialog", { name: "Renumber Cues" });
		expect(
			within(renumber).getByText(/the first Cue receives this number/i),
		).toBeVisible();
		const primaryAction = within(renumber).getByRole("button", {
			name: "Renumber",
		});
		expect(primaryAction.closest(".ui-title-chrome-terminals")).not.toBeNull();
		expect(renumber.querySelector(".modal-actions")).toBeNull();

		fireEvent.click(
			within(renumber).getByRole("button", { name: "Cancel renumbering" }),
		);
		expect(
			screen.queryByRole("dialog", { name: "Renumber Cues" }),
		).not.toBeInTheDocument();
		expect(mocks.saveTopologyCueList).not.toHaveBeenCalled();

		fireEvent.click(
			within(settings).getByRole("button", { name: "Renumber Cues" }),
		);
		renumber = screen.getByRole("dialog", { name: "Renumber Cues" });
		fireEvent.change(within(renumber).getByLabelText("First new Cue number"), {
			target: { value: "10" },
		});
		fireEvent.click(within(renumber).getByRole("button", { name: "Renumber" }));

		await waitFor(() =>
			expect(mocks.saveTopologyCueList).toHaveBeenCalledOnce(),
		);
		expect(mocks.saveTopologyCueList.mock.calls[0].slice(0, 3)).toEqual([
			"main",
			3,
			"legacy-main",
		]);
		const saved = mocks.saveTopologyCueList.mock.calls[0][3] as CueList;
		expect(saved.cues).toEqual([
			{ ...cueList.cues[0], number: "10" },
			{ ...cueList.cues[1], number: "11" },
		]);
		expect(mocks.refresh).not.toHaveBeenCalled();
	});
});

describe("CuelistWindow pool recording", () => {
	beforeEach(resetCuelistWindowMocks);

	it("renders only explicitly configured Cuelist presentation icons", async () => {
		mocks.state.storeArmed = false;
		mocks.playbacks.pool = [
			{
				number: 1,
				name: "No icon",
				target: { type: "cue_list", cue_list_id: "main" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
			},
			{
				number: 2,
				name: "Explicit icon",
				target: { type: "cue_list", cue_list_id: "encore" },
				buttons: ["go", "go_minus", "flash"],
				fader: "master",
				go_activates: true,
				auto_off: true,
				xfade_millis: 0,
				presentation_icon: "★",
			},
		];
		const authority = createCommandLineTestAuthority();
		const { container } = render(
			authority.wrap(<CuelistWindow compact cueListTab="pool" />),
		);
		await act(authority.settle);
		const cards = container.querySelectorAll(".cuelist-card");
		expect(cards[0].querySelector(".pool-card-icon")).toBeNull();
		expect(cards[1].querySelector(".pool-card-icon")).toHaveTextContent("★");
	});

	it("renders empty numbered slots and records into the touched slot", async () => {
		const authority = createCommandLineTestAuthority({ text: "STORE" });
		render(authority.wrap(<CuelistWindow compact cueListTab="pool" />));
		await act(authority.settle);
		expect(screen.getAllByText("Tap to record Cuelist")).toHaveLength(1000);
		const cards = document.querySelectorAll(".cuelist-card");
		expect(cards).toHaveLength(1000);
		expect(cards[0]).toHaveAttribute("data-pool-slot-id", "1");
		expect(cards[0]).toHaveClass("store-target");
		expect(cards[999]).toHaveAttribute("data-pool-slot-id", "1000");
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
		const { container } = render(<CuelistWindow compact cueListTab="pool" />);
		const cards =
			container.querySelectorAll<HTMLButtonElement>(".cuelist-card");
		const source = screen.getByText("Main sequence").closest("button")!;
		expect(source).toHaveClass("set-target");
		expect(source).toHaveTextContent("Set");
		expect(cards[1]).not.toHaveClass("set-target");
		fireEvent.click(source);
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_CUELIST_SET_TARGET",
			value: 7,
		});
	});

	it("right-click opens that exact Cuelist's settings without starting assignment", () => {
		mocks.state.storeArmed = false;
		mocks.state.cueListSetArmed = true;
		showEditableCueList();
		mocks.state.cueListSetArmed = true;
		render(<CuelistWindow compact cueListTab="pool" />);
		const source = screen.getByText("Main").closest("button")!;
		const contextMenu = new MouseEvent("contextmenu", {
			bubbles: true,
			cancelable: true,
		});
		fireEvent(source, contextMenu);

		expect(contextMenu.defaultPrevented).toBe(true);
		expect(
			screen.getByRole("dialog", { name: "Cuelist Settings" }),
		).toHaveTextContent("Main");
		expect(mocks.dispatch).not.toHaveBeenCalledWith(
			expect.objectContaining({ type: "SET_CUELIST_SET_TARGET" }),
		);
	});

	it("right-click on an empty Cuelist reports settings guidance without assignment", () => {
		mocks.state.storeArmed = false;
		mocks.state.cueListSetArmed = true;
		const { container } = render(<CuelistWindow compact cueListTab="pool" />);
		const empty = container.querySelector<HTMLButtonElement>(".cuelist-card")!;
		fireEvent.contextMenu(empty);

		expect(container.querySelector(".pool-message")).toHaveTextContent(
			"Cuelist 1 is empty · record it before opening settings.",
		);
		expect(mocks.dispatch).not.toHaveBeenCalledWith(
			expect.objectContaining({ type: "SET_CUELIST_SET_TARGET" }),
		);
	});

	it("keeps the touch hold shortcut for opening exact Cuelist settings", () => {
		vi.useFakeTimers();
		mocks.state.storeArmed = false;
		showEditableCueList();
		const { container } = render(<CuelistWindow compact cueListTab="pool" />);
		const source = screen.getByText("Main").closest("button");
		expect(source).not.toBeNull();
		if (!source) throw new Error("Expected Main Cuelist pool card");
		fireEvent.pointerDown(source, { pointerType: "touch" });
		act(() => vi.advanceTimersByTime(650));
		fireEvent.pointerUp(source, { pointerType: "touch" });

		expect(
			screen.getByRole("dialog", { name: "Cuelist Settings" }),
		).toHaveTextContent("Main");
		expect(container.querySelector(".cue-table")).not.toBeInTheDocument();
		expect(mocks.dispatch).not.toHaveBeenCalledWith(
			expect.objectContaining({ type: "SET_CUELIST_SET_TARGET" }),
		);
		vi.useRealTimers();
	});

	it("opens the presentation image in a modal and restores image-button focus", async () => {
		mocks.state.storeArmed = false;
		showEditableCueList();
		mocks.playbacks.pool[0].presentation_image =
			"data:image/png;base64,cHJldmlldw==";
		const { container } = render(<CuelistWindow compact cueListTab="pool" />);
		const preview = screen.getByRole("button", {
			name: "Open Main preview",
		});
		preview.focus();
		fireEvent.click(preview);

		let dialog = screen.getByRole("dialog", { name: "Main preview image" });
		expect(
			within(dialog).getByRole("img", { name: "Main preview" }),
		).toHaveAttribute("src", "data:image/png;base64,cHJldmlldw==");
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Close Cuelist preview" }),
		);
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Main preview image" }),
			).not.toBeInTheDocument(),
		);

		fireEvent.click(preview);
		dialog = screen.getByRole("dialog", { name: "Main preview image" });
		await waitFor(() =>
			expect(
				within(dialog).getByRole("button", {
					name: "Close Cuelist preview",
				}),
			).toHaveFocus(),
		);
		fireEvent.keyDown(document, { key: "Escape" });
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Main preview image" }),
			).not.toBeInTheDocument(),
		);
		await waitFor(() => expect(preview).toHaveFocus());

		fireEvent.click(preview);
		dialog = screen.getByRole("dialog", { name: "Main preview image" });
		fireEvent.pointerDown(dialog.closest("[data-modal-id]")!);
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Main preview image" }),
			).not.toBeInTheDocument(),
		);
		await waitFor(() => expect(preview).toHaveFocus());
		expect(container.querySelector(".cue-table")).not.toBeInTheDocument();
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
