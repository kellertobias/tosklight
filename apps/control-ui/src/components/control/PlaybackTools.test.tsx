import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SpeedGroupId, SpeedGroupSoundState } from "../../api/types";
import { PlaybackTools } from "./PlaybackTools";

const dispatch = vi.fn();
const state = {
	playbackPage: 0,
	playbackPageNames: ["Main", "Effects"],
	playbackSetArmed: false,
	shiftArmed: false,
};
let playbackDesk: { active_page: number } | null = { active_page: 1 };
let runtimeStatus: "ready" | "loading" | "error" = "ready";
let topologyReady = true;
let pageObjects = [
	{
		kind: "playback_page" as const,
		id: "legacy-page-one",
		revision: 3,
		updated_at: "",
		body: { number: 1, name: "Main", slots: {} as Record<string, number> },
	},
];
const topologyActions = {
	createPage: vi.fn(async (): Promise<object | null> => ({})),
	renamePage: vi.fn(async (): Promise<object | null> => ({})),
	error: null as Error | null,
};
const runtimeActions = { setActivePage: vi.fn(async () => true) };
const commandProjection = {
	text: "FIXTURE",
	target: "FIXTURE" as const,
	pristine: true,
	pendingChoice: null,
	revision: 1,
};
const commandActions = {
	replace: vi.fn(async (text: string) => {
		commandProjection.text = text;
		commandProjection.pristine = text.trim().toUpperCase() === commandProjection.target;
		return true;
	}),
	reset: vi.fn().mockResolvedValue(true),
	flush: vi.fn().mockResolvedValue(true),
	execute: vi.fn().mockResolvedValue({ executed: true, report: "none" }),
	executeAfterPendingWrites: vi.fn(),
};
const commandStore = {
	getSnapshot: () => ({ commandLine: commandProjection }),
};
const server = {
	session: null as { session_id: string; desk: { id: string } } | null,
	configuration: {
		programmer_fade_millis: 3_000,
		sequence_master_fade_millis: 4_000,
		speed_groups_bpm: [120, 90, 60, 30, 15],
	},
	playbacks: {
		active_page: 9,
		pages: [{ number: 9, name: "Stale", slots: {} as Record<string, number> }],
	},
	setControlTiming: vi.fn(),
	speedGroup: vi.fn(),
	updateSpeedGroup: vi.fn(),
	observeSpeedGroup: vi.fn(),
	speedGroupAction: vi.fn(),
	commandLine: "FIXTURE",
	commandLinePristine: true,
	commandTargetMode: "FIXTURE" as const,
	setCommandLine: vi.fn(),
	executeCommandLine: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state,
		dispatch: (action: { type: string; value?: boolean }) => {
			if (action.type === "SET_PLAYBACK_SET_ARMED")
				state.playbackSetArmed = Boolean(action.value);
			if (action.type === "SET_SHIFT_ARMED")
				state.shiftArmed = Boolean(action.value);
			dispatch(action);
		},
	}),
}));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: () => playbackDesk,
	usePlaybackRuntimeActions: () => runtimeActions,
	usePlaybackRuntimeStatus: () => ({ status: runtimeStatus, error: null }),
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyProvider", () => ({
	usePlaybackTopologyActions: () => topologyActions,
}));
vi.mock("../../features/playbackTopology/PlaybackTopologyView", () => ({
	usePlaybackPagesView: () => ({
		ready: topologyReady,
		error: null,
		pages: topologyReady ? pageObjects : [],
	}),
}));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	async (importOriginal) => ({
		...(await importOriginal()),
		useProgrammingCommandLineView: () => commandProjection,
		useProgrammingCommandLineReady: () => true,
		useProgrammingCommandLineActions: () => commandActions,
		useProgrammingInteractionStore: () => commandStore,
	}),
);

afterEach(() => {
	cleanup();
	server.session = null;
	playbackDesk = { active_page: 1 };
	runtimeStatus = "ready";
	topologyReady = true;
	pageObjects = [
		{
			kind: "playback_page",
			id: "legacy-page-one",
			revision: 3,
			updated_at: "",
			body: { number: 1, name: "Main", slots: {} },
		},
	];
	state.playbackPage = 0;
	state.playbackSetArmed = false;
	state.shiftArmed = false;
	topologyActions.error = null;
	commandProjection.text = "FIXTURE";
	commandProjection.pristine = true;
	if (typeof localStorage.clear === "function") localStorage.clear();
	vi.clearAllMocks();
});

function soundState(group: SpeedGroupId): SpeedGroupSoundState {
	const bpm = server.configuration.speed_groups_bpm[group.charCodeAt(0) - 65];
	return {
		group,
		configuration: {
			enabled: false,
			analysis_mode: "tempo_bpm",
			frequency: { type: "preset", preset: "low" },
			input_gain_db: 0,
			confidence_threshold: 0.65,
			smoothing: 0.35,
			minimum_bpm: 40,
			maximum_bpm: 240,
			signal_hold_millis: 2_000,
			multiplier: 1,
		},
		snapshot: {
			manual_bpm: bpm,
			sound_bpm: null,
			effective_bpm: bpm,
			source: "manual",
			sound_status: { state: "disabled" },
			paused: false,
			phase_advancing: true,
			speed_master_scale: 1,
			sound_multiplier: 1,
			source_available: false,
			usable_signal: false,
			input_level: 0,
			selected_band_level: 0,
		},
	};
}

describe("PlaybackTools", () => {
	it("orders page controls, fade masters, and speed groups with icon-only chevrons", () => {
		const { container } = render(<PlaybackTools />);
		const tools = container.querySelector<HTMLElement>(".playback-tools");
		if (!tools) throw new Error("Playback tools were not rendered");
		expect([...tools.children].map((child) => child.className)).toEqual([
			"playback-command-keys",
			"playback-page-controls",
			"programmer-fade-fader full",
			"cue-fade-master",
			"speed-group-stack",
		]);
		const commandKeys = container.querySelector<HTMLElement>(
			".playback-command-keys",
		);
		if (!commandKeys)
			throw new Error("Playback command keys were not rendered");
		expect(
			within(commandKeys)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual(["SET", "CPY", "MOV", "DEL", "SHIFT"]);
		const previous = screen.getByRole("button", {
			name: "Previous playback page",
		});
		const next = screen.getByRole("button", { name: "Next playback page" });
		expect(previous.textContent).toBe("");
		expect(next.textContent).toBe("");
		expect(previous.querySelector("svg path")).toBeInTheDocument();
		expect(next.querySelector("svg path")).toBeInTheDocument();
		const current = screen.getByRole("button", {
			name: "Select playback page. Page 1 Main",
		});
		expect(within(current).getByText("Page")).toBeInTheDocument();
		expect(within(current).getByText("1")).toBeInTheDocument();
		expect(within(current).getByText("Main")).toBeInTheDocument();
		const speedGroups =
			container.querySelector<HTMLElement>(".speed-group-stack");
		if (!speedGroups) throw new Error("Speed Groups were not rendered");
		const speedGroupA = within(speedGroups).getByRole("button", {
			name: "Speed group A, 120 BPM",
		});
		expect([...speedGroupA.children].map((child) => child.className)).toEqual([
			"speed-group-label",
			"speed-group-value",
			"speed-group-unit",
		]);
	});

	it("routes playback command keys through the shared command line behavior", () => {
		const { rerender } = render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "CPY" }));
		expect(commandActions.replace).toHaveBeenCalledWith("COPY");

		fireEvent.click(screen.getByRole("button", { name: "SHIFT" }));
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: true,
		});
		rerender(<PlaybackTools />);
		expect(screen.getByRole("button", { name: "SHIFT" })).toHaveClass("active");

		fireEvent.click(screen.getByRole("button", { name: "DEL" }));
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "systemControlsOpen",
			value: true,
		});
		expect(commandActions.replace).toHaveBeenCalledTimes(1);
	});

	it("creates and selects the next page when the last page has an assignment", async () => {
		pageObjects[0].body.slots = { "1": 12 };
		render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));
		await waitFor(() =>
			expect(topologyActions.createPage).toHaveBeenCalledWith(2),
		);
		expect(runtimeActions.setActivePage).toHaveBeenCalledWith(2);
		expect(dispatch).not.toHaveBeenCalledWith({
			type: "SET_PLAYBACK_PAGE",
			page: 1,
		});
	});

	it("shows a failed next-Page creation and does not change desk authority", async () => {
		pageObjects[0].body.slots = { "1": 12 };
		topologyActions.createPage.mockResolvedValueOnce(null);
		render(<PlaybackTools />);

		fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));

		const dialog = await screen.findByRole("dialog", { name: "Playback pages" });
		expect(await within(dialog).findByRole("alert")).toHaveTextContent(
			"Playback Page 2 could not be created.",
		);
		expect(runtimeActions.setActivePage).not.toHaveBeenCalled();
	});

	it("does not let Previous materialize a missing Page through desk compatibility", () => {
		playbackDesk = { active_page: 3 };
		pageObjects = [
			pageObjects[0],
			{
				...pageObjects[0],
				id: "page-three",
				body: { number: 3, name: "Third", slots: {} },
			},
		];
		render(<PlaybackTools />);

		const previous = screen.getByRole("button", {
			name: "Previous playback page",
		});
		expect(previous).toBeDisabled();
		fireEvent.click(previous);
		expect(runtimeActions.setActivePage).not.toHaveBeenCalled();
	});

	it("keeps Next disabled on an empty last page but lets the page menu add one", async () => {
		render(<PlaybackTools />);
		expect(
			screen.getByRole("button", { name: "Next playback page" }),
		).toBeDisabled();
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const dialog = screen.getByRole("dialog", { name: "Playback pages" });
		const addPage = within(dialog).getByRole("button", {
			name: "Add new page",
		});
		expect(addPage.parentElement).toHaveClass("ui-modal-title-actions");
		fireEvent.click(addPage);
		await waitFor(() =>
			expect(topologyActions.createPage).toHaveBeenCalledWith(2),
		);
		expect(runtimeActions.setActivePage).toHaveBeenCalledWith(2);
	});

	it("opens page rename with SET then Page and persists the trimmed name", async () => {
		const view = render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Rename playback page 1",
		});
		pageObjects = [
			{
				...pageObjects[0],
				revision: 4,
				body: { ...pageObjects[0].body, name: "Concurrent" },
			},
		];
		view.rerender(<PlaybackTools />);
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PLAYBACK_SET_ARMED",
			value: false,
		});
		fireEvent.change(
			within(dialog).getByRole("textbox", { name: "Playback page name" }),
			{ target: { value: "  Act One  " } },
		);
		fireEvent.click(
			within(dialog).getByRole("button", { name: "Rename Page" }),
		);
		await waitFor(() =>
			expect(topologyActions.renamePage).toHaveBeenCalledWith(1, "Act One", {
				expectedPageRevision: 4,
				expectedPageObjectId: "legacy-page-one",
			}),
		);
	});

	it("keeps a failed rename open with operation-owned feedback", async () => {
		topologyActions.error = new Error("Page revision changed");
		topologyActions.renamePage.mockResolvedValueOnce(null);
		const view = render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Rename playback page 1",
		});

		fireEvent.change(
			within(dialog).getByRole("textbox", { name: "Playback page name" }),
			{ target: { value: "Act One" } },
		);
		fireEvent.click(within(dialog).getByRole("button", { name: "Rename Page" }));

		await waitFor(() =>
			expect(within(dialog).getByRole("alert")).toHaveTextContent(
				"Playback Page 1 could not be renamed.",
			),
		);
		expect(dialog).toBeInTheDocument();

		pageObjects = [{ ...pageObjects[0], revision: 4 }];
		view.rerender(<PlaybackTools />);
		fireEvent.click(within(dialog).getByRole("button", { name: "Rename Page" }));
		await waitFor(() => expect(topologyActions.renamePage).toHaveBeenCalledTimes(2));
		expect(topologyActions.renamePage).toHaveBeenLastCalledWith(1, "Act One", {
			expectedPageRevision: 4,
			expectedPageObjectId: "legacy-page-one",
		});
	});

	it("does not close the page menu while Add is between create and select", async () => {
		let resolveCreate: (value: object | null) => void = () => undefined;
		topologyActions.createPage.mockReturnValueOnce(
			new Promise((resolve) => {
				resolveCreate = resolve;
			}),
		);
		render(<PlaybackTools />);
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const dialog = screen.getByRole("dialog", { name: "Playback pages" });
		fireEvent.click(within(dialog).getByRole("button", { name: "Add new page" }));
		await waitFor(() => expect(topologyActions.createPage).toHaveBeenCalledOnce());

		const close = within(dialog).getByRole("button", {
			name: "Close Playback pages",
		});
		expect(close).toBeDisabled();
		fireEvent.keyDown(window, { key: "Escape" });
		fireEvent.pointerDown(dialog.parentElement!);
		expect(dialog).toBeInTheDocument();

		await act(async () => resolveCreate({}));
		await waitFor(() => expect(runtimeActions.setActivePage).toHaveBeenCalledWith(2));
		await waitFor(() => expect(dialog).not.toBeInTheDocument());
	});

	it("serializes rapid rename submissions", async () => {
		let resolveRename: (value: object) => void = () => undefined;
		topologyActions.renamePage.mockReturnValueOnce(
			new Promise((resolve) => {
				resolveRename = resolve;
			}),
		);
		render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const dialog = screen.getByRole("dialog", {
			name: "Rename playback page 1",
		});
		fireEvent.change(
			within(dialog).getByRole("textbox", { name: "Playback page name" }),
			{ target: { value: "Act One" } },
		);
		const rename = within(dialog).getByRole("button", { name: "Rename Page" });

		fireEvent.click(rename);
		fireEvent.click(rename);

		expect(topologyActions.renamePage).toHaveBeenCalledOnce();
		expect(within(dialog).getByRole("button", { name: "Renaming…" })).toBeDisabled();
		resolveRename({});
		await waitFor(() => expect(dialog).not.toBeInTheDocument());
	});

	it("closes an open rename dialog when topology writer authority changes", async () => {
		const view = render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Rename playback page 1" }),
		).toBeInTheDocument();
		const openedRenamePage = topologyActions.renamePage;
		topologyActions.renamePage = vi.fn(async () => ({}));

		view.rerender(<PlaybackTools />);

		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Rename playback page 1" }),
			).not.toBeInTheDocument(),
		);
		topologyActions.renamePage = openedRenamePage;
	});

	it("closes an open rename dialog while Page authority repairs", async () => {
		const view = render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Rename playback page 1" }),
		).toBeInTheDocument();

		topologyReady = false;
		view.rerender(<PlaybackTools />);

		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Rename playback page 1" }),
			).not.toBeInTheDocument(),
		);
	});

	it("does not continue old Page creation after authority replacement", async () => {
		pageObjects[0].body.slots = { "1": 12 };
		let resolveCreate: (value: object | null) => void = () => undefined;
		const originalCreate = topologyActions.createPage;
		topologyActions.createPage = vi.fn(
			() =>
				new Promise<object | null>((resolve) => {
					resolveCreate = resolve;
				}),
		);
		const view = render(<PlaybackTools />);
		fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));
		await waitFor(() => expect(topologyActions.createPage).toHaveBeenCalledOnce());

		topologyActions.createPage = vi.fn(async (): Promise<object | null> => ({}));
		view.rerender(<PlaybackTools />);
		await act(async () => resolveCreate({}));

		expect(runtimeActions.setActivePage).not.toHaveBeenCalled();
		topologyActions.createPage = originalCreate;
	});

	it("ignores a failed old-scope selection completion", async () => {
		playbackDesk = { active_page: 2 };
		pageObjects = [
			pageObjects[0],
			{
				...pageObjects[0],
				id: "page-two",
				body: { number: 2, name: "Second", slots: {} },
			},
		];
		let resolveSelection: (value: boolean) => void = () => undefined;
		const originalSelection = runtimeActions.setActivePage;
		runtimeActions.setActivePage = vi.fn(
			() =>
				new Promise<boolean>((resolve) => {
					resolveSelection = resolve;
				}),
		);
		const view = render(<PlaybackTools />);
		fireEvent.click(
			screen.getByRole("button", { name: "Previous playback page" }),
		);
		await waitFor(() => expect(runtimeActions.setActivePage).toHaveBeenCalledOnce());

		runtimeActions.setActivePage = vi.fn(async () => true);
		view.rerender(<PlaybackTools />);
		await act(async () => resolveSelection(false));

		expect(screen.queryByRole("dialog", { name: "Playback pages" })).toBeNull();
		runtimeActions.setActivePage = originalSelection;
	});

	it("keeps runtime selection feedback separate from an older topology error", async () => {
		topologyActions.error = new Error("Older Page conflict");
		runtimeActions.setActivePage.mockResolvedValueOnce(false);
		render(<PlaybackTools />);
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const pages = screen.getByRole("dialog", { name: "Playback pages" });
		fireEvent.click(
			within(pages).getByRole("button", { name: /Main/ }),
		);

		expect(await within(pages).findByRole("alert")).toHaveTextContent(
			"Playback Page 1 could not be selected.",
		);
		expect(pages).not.toHaveTextContent("Older Page conflict");
	});

	it("opens the full-text keyboard from the rename button in the page menu", () => {
		render(<PlaybackTools />);
		fireEvent.click(
			screen.getByRole("button", { name: "Select playback page. Page 1 Main" }),
		);
		const pages = screen.getByRole("dialog", { name: "Playback pages" });
		fireEvent.click(
			within(pages).getByRole("button", { name: "Rename playback page 1" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Playback page name" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText("Full text keyboard")).toBeInTheDocument();
		expect(runtimeActions.setActivePage).not.toHaveBeenCalled();
	});

	it("does not fall back to stale bootstrap Pages while scoped authority loads", () => {
		playbackDesk = null;
		topologyReady = false;

		render(<PlaybackTools />);

		expect(
			screen.getByRole("button", { name: "Playback page loading" }),
		).toBeDisabled();
		expect(screen.getByText("Loading…")).toBeInTheDocument();
		expect(screen.queryByText("Stale")).not.toBeInTheDocument();
		expect(
			screen.getByRole("button", { name: "Next playback page" }),
		).toBeDisabled();
	});

	it("does not act on a retained desk while runtime authority repairs", () => {
		runtimeStatus = "loading";
		playbackDesk = { active_page: 1 };

		render(<PlaybackTools />);

		expect(
			screen.getByRole("button", { name: "Playback page loading" }),
		).toBeDisabled();
		fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));
		expect(runtimeActions.setActivePage).not.toHaveBeenCalled();
	});

	it("opens the selected Speed Group Sound-to-Light configuration instead of treating the UI button as a Learn tap", async () => {
		server.session = { session_id: "session-a", desk: { id: "desk-a" } };
		server.speedGroup.mockImplementation(async (group: SpeedGroupId) =>
			soundState(group),
		);
		render(<PlaybackTools />);
		await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(5));
		fireEvent.click(
			screen.getByRole("button", { name: "Speed group A, 120 BPM" }),
		);
		expect(
			await screen.findByRole("dialog", {
				name: "Speed Group A Sound to Light",
			}),
		).toBeInTheDocument();
		expect(
			screen.getByText("Audio input on this desk/browser"),
		).toBeInTheDocument();
		expect(server.setControlTiming).not.toHaveBeenCalled();
		expect(server.speedGroupAction).not.toHaveBeenCalled();
	});
});
