import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import "../../styles.css";
import { NumericPad, numericPadLayout } from "./NumericPad";

const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
	if (action.type === "SET_SHIFT_ARMED")
		state.shiftArmed = Boolean(action.value);
});
const server = {
	bootstrap: { active_programmers: [] as Array<Record<string, unknown>> },
	session: { session_id: "desk-session", user: { id: "operator" } },
	selectedFixtures: [] as string[],
	configuration: { programmer_fade_millis: 3_000 },
	commandLine: "FIXTURE",
	commandTargetMode: "FIXTURE",
	commandLinePristine: true,
	resetCommandLine: vi.fn(),
	clearProgrammer: vi.fn(),
	undoProgrammer: vi.fn(),
	preloadAction: vi.fn().mockResolvedValue(undefined),
	executeCommandLine: vi.fn().mockResolvedValue(true),
	setCommandLine: vi.fn(),
	setControlTiming: vi.fn(),
	playbacks: {
		selected_playback: 42,
		active: [
			{
				playback_number: 7,
				cue_list_id: "running",
				cue_index: 0,
				paused: false,
				master: 1,
				flash: false,
			},
		],
	},
};
const state = {
	storeArmed: false,
	cueListSetArmed: false,
	preload: "idle",
	builtIn: null as string | null,
	activeDeskId: "programming",
	desks: [
		{ id: "desk-one", name: "Desk One", panes: [] },
		{ id: "desk-two", name: "Desk Two", panes: [] },
		{ id: "desk-three", name: "Desk Three", panes: [] },
	],
	patchSetArmed: false,
	presetSetArmed: false,
	playbackSetArmed: false,
	shiftArmed: false,
};
const valuesActivity = {
	current: {
		authority: "loading" as "loading" | "normal" | "preload",
		ready: false,
		valueCount: 0,
		pendingValueCount: 0,
	},
};
const programmerValuesActions = {
	clear: vi.fn().mockResolvedValue(null),
};
const selectionActions = {
	replace: vi.fn().mockResolvedValue(null),
};
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
	reset: vi.fn(async () => {
		commandProjection.text = commandProjection.target;
		commandProjection.pristine = true;
		return true;
	}),
	flush: vi.fn().mockResolvedValue(true),
	execute: vi.fn().mockResolvedValue({ executed: true, report: "none" }),
	executeAfterPendingWrites: vi.fn(),
};
const commandStore = {
	getSnapshot: () => ({ commandLine: commandProjection }),
};
let playbackDesk: {
	active_page: number;
	selected_playback: number | null;
} | null = {
	active_page: 1,
	selected_playback: 42,
};
let runtimeStatus: "ready" | "loading" | "error" = "ready";

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state, dispatch }),
}));
vi.mock("../../features/programmerValues/useProgrammerValuesActivity", () => ({
	useProgrammerValuesActivity: () => valuesActivity.current,
}));
vi.mock("../../features/programmerValues/ProgrammerValuesView", () => ({
	useProgrammerValuesActions: () => programmerValuesActions,
}));
vi.mock("../../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: () => playbackDesk,
	usePlaybackRuntimeStatus: () => ({ status: runtimeStatus, error: null }),
}));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	async (importOriginal) => ({
		...(await importOriginal()),
		useProgrammingCommandLineView: () => commandProjection,
		useProgrammingCommandLineReady: () => true,
		useProgrammingCommandLineActions: () => commandActions,
		useProgrammingInteractionStore: () => commandStore,
		useProgrammingSelectionView: () => ({
			selected: server.selectedFixtures,
			expression: { type: "static" },
			revision: 1,
			gestureOpen: false,
		}),
		useProgrammingSelectionActions: () => selectionActions,
	}),
);

afterEach(() => {
	cleanup();
	server.bootstrap.active_programmers = [];
	server.selectedFixtures = [];
	commandProjection.text = "FIXTURE";
	commandProjection.pristine = true;
	state.shiftArmed = false;
	state.preload = "idle";
	state.activeDeskId = "programming";
	server.playbacks.selected_playback = 42;
	playbackDesk = { active_page: 1, selected_playback: 42 };
	runtimeStatus = "ready";
	valuesActivity.current = {
		authority: "loading",
		ready: false,
		valueCount: 0,
		pendingValueCount: 0,
	};
	(state as { builtIn: string | null }).builtIn = null;
	(state.desks[0] as { panes: Array<{ kind: string }> }).panes = [];
	vi.clearAllMocks();
});

describe("NumericPad Clear and SET routing", () => {
	it("ignores stale bootstrap values and clears selection before scoped normal values", () => {
		server.bootstrap.active_programmers = [
			{
				session_id: "desk-session",
				values: [
					{
						fixture_id: "fixture-1",
						attribute: "intensity",
						value: { kind: "normalized", value: 0.5 },
					},
				],
				group_values: {},
			},
		];
		const { rerender } = render(<NumericPad />);
		const clear = () => screen.getByRole("button", { name: "CLR" });
		expect(clear()).toHaveClass("clear-idle");

		server.selectedFixtures = ["fixture-1"];
		valuesActivity.current = {
			authority: "normal",
			ready: true,
			valueCount: 1,
			pendingValueCount: 0,
		};
		rerender(<NumericPad />);
		expect(clear()).toHaveClass("clear-active");
		fireEvent.click(clear());
		expect(selectionActions.replace).toHaveBeenCalledWith({
			resolvedFixtures: [],
		});
		expect(programmerValuesActions.clear).not.toHaveBeenCalled();

		server.selectedFixtures = [];
		rerender(<NumericPad />);
		expect(clear()).toHaveClass("clear-warning");
		fireEvent.click(clear());
		expect(programmerValuesActions.clear).toHaveBeenCalledWith(
			expect.any(String),
		);

		valuesActivity.current = {
			authority: "normal",
			ready: true,
			valueCount: 0,
			pendingValueCount: 0,
		};
		rerender(<NumericPad />);
		expect(clear()).toHaveClass("clear-idle");
	});

	it("routes Clear through the active Preload authority", () => {
		state.preload = "blind";
		valuesActivity.current = {
			authority: "preload",
			ready: true,
			valueCount: 1,
			pendingValueCount: 1,
		};
		render(<NumericPad />);

		const clear = screen.getByRole("button", { name: "CLR" });
		expect(clear).toHaveClass("clear-warning");
		fireEvent.click(clear);

		expect(server.preloadAction).toHaveBeenCalledWith("clear");
		expect(programmerValuesActions.clear).not.toHaveBeenCalled();
	});

	it("arms playback configuration when a Virtual Playback grid is the available target surface", () => {
		const grid = document.createElement("div");
		grid.className = "virtual-playback-grid";
		document.body.append(grid);
		render(<NumericPad />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PLAYBACK_SET_ARMED",
			value: true,
		});
		grid.remove();
	});

	it("routes software SET to a height-constrained Cue settings editor", () => {
		const fallback = document.createElement("section");
		fallback.className = "cue-settings-compact-fallback";
		document.body.append(fallback);
		const set = vi.fn();
		window.addEventListener("light:desk-action", set, { once: true });
		render(<NumericPad />);

		fireEvent.click(screen.getByRole("button", { name: "SET" }));

		expect(set).toHaveBeenCalledOnce();
		expect((set.mock.calls[0][0] as CustomEvent<string>).detail).toBe("set");
		expect(commandActions.replace).not.toHaveBeenCalled();
		fallback.remove();
	});

	it("arms the same selected Patch target from the software SET key", () => {
		state.builtIn = "patch";
		render(<NumericPad />);

		fireEvent.click(screen.getByRole("button", { name: "SET" }));

		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PATCH_ARMED",
			value: true,
		});
		expect(commandActions.replace).not.toHaveBeenCalled();
	});

	it("does not let a hidden Presets pane steal SET from another visible built-in", () => {
		state.activeDeskId = "desk-one";
		(state as { builtIn: string | null }).builtIn = "groups";
		(state.desks[0] as { panes: Array<{ kind: string }> }).panes = [
			{ kind: "presets" },
		];
		render(<NumericPad />);
		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		expect(dispatch).not.toHaveBeenCalledWith({
			type: "SET_PRESET_SET_ARMED",
			value: true,
		});
		expect(commandActions.replace).toHaveBeenCalledWith("SET");
	});

	it("keeps SET as a command token once Copy or Move entry has started", () => {
		state.activeDeskId = "desk-one";
		(state.desks[0] as { panes: Array<{ kind: string }> }).panes = [
			{ kind: "presets" },
		];
		commandProjection.text = "COPY";
		commandProjection.pristine = false;
		render(<NumericPad />);

		fireEvent.click(screen.getByRole("button", { name: "SET" }));
		expect(dispatch).not.toHaveBeenCalledWith({
			type: "SET_PRESET_SET_ARMED",
			value: true,
		});
		expect(commandActions.replace).toHaveBeenCalledWith("COPY SET ");
	});
});

describe("NumericPad layout and Shift routing", () => {
	it("uses six-row grids with aligned Highlight actions, a 2x2 Fade control, and a single-row Enter key", () => {
		const { container } = render(<NumericPad />);
		expect(
			container.querySelector(".numeric-pad-command-section"),
		).toBeInTheDocument();
		expect(
			container.querySelector(".numeric-pad-number-section"),
		).toBeInTheDocument();
		expect(container.querySelector(".numeric-pad-fade")).toHaveStyle({
			gridColumn: "1 / span 2",
			gridRow: "1 / span 2",
		});
		expect(container.querySelector(".numeric-pad-fade")).toHaveAttribute(
			"data-grid-column-span",
			"2",
		);
		expect(container.querySelector(".numeric-pad-fade")).toHaveAttribute(
			"data-grid-row-span",
			"2",
		);
		for (const { key, section, column, row, rowSpan = 1 } of numericPadLayout) {
			const expectedColumn = section === "commands" ? column : column - 3;
			const expectedRow = row + 1;
			expect(container.querySelector(`[data-keypad-key="${key}"]`)).toHaveStyle(
				{
					gridColumn: `${expectedColumn}`,
					gridRow: `${expectedRow} / span ${rowSpan}`,
				},
			);
		}
		const highlight = screen.getByRole("region", {
			name: "Highlight and selection stepping",
		});
		expect(highlight.parentElement).toHaveClass("numeric-pad-number-section");
		expect(
			[...highlight.querySelectorAll("button")].map(
				(button) => button.textContent,
			),
		).toEqual(["HIGH", "PREV", "NEXT", "ALL"]);
		expect(highlight.querySelector(".highlight-toggle")).toHaveTextContent(
			/^HIGH$/,
		);
		expect(highlight.querySelector(".highlight-previous")).toHaveTextContent(
			/^PREV$/,
		);
		expect(highlight.querySelector(".highlight-next")).toHaveTextContent(
			/^NEXT$/,
		);
		expect(highlight.querySelector(".highlight-all")).toHaveTextContent(
			/^ALL$/,
		);
		expect(screen.getByRole("button", { name: "SET" })).toHaveAttribute(
			"data-keypad-key",
			"SET",
		);
		expect(screen.getByRole("button", { name: "CUE" })).toHaveAttribute(
			"data-keypad-key",
			"CUE",
		);
		expect(screen.getByRole("button", { name: "UND" })).toBeInTheDocument();
		expect(screen.getByRole("button", { name: "TRU" })).toHaveStyle({
			gridColumn: "4",
			gridRow: "5 / span 1",
		});
		expect(screen.getByRole("button", { name: "ENT" })).toHaveStyle({
			gridColumn: "4",
			gridRow: "6 / span 1",
		});
	});

	it("renders inactive HIGH with the same neutral off treatment as idle CLR", () => {
		render(<NumericPad />);

		const highStyle = getComputedStyle(
			screen.getByRole("button", { name: "Turn Highlight on" }),
		);
		const clearStyle = getComputedStyle(
			screen.getByRole("button", { name: "CLR" }),
		);

		expect(highStyle.backgroundColor).toBe(clearStyle.backgroundColor);
		expect(highStyle.backgroundColor).toBe("rgb(23, 28, 34)");
		expect(highStyle.borderTopColor).toBe(clearStyle.borderTopColor);
		expect(highStyle.borderTopWidth).toBe(clearStyle.borderTopWidth);
		expect(highStyle.borderTopStyle).toBe(clearStyle.borderTopStyle);
	});
});

describe("NumericPad Shift routing", () => {
	it("routes Shift shortcuts to built-ins, the scoped selected playback, and stored desks", () => {
		server.playbacks.selected_playback = 99;
		render(<NumericPad />);
		const shifted = (key: string) => {
			fireEvent.click(screen.getByRole("button", { name: "SHIFT" }));
			fireEvent.click(screen.getByRole("button", { name: key }));
		};

		shifted(".");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "help",
		});
		shifted("0");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "fixtures",
		});
		shifted("1");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "groups",
		});
		shifted("2");
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PRESET_FAMILY",
			family: "Mixed",
		});
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "presets",
		});
		shifted("3");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		shifted("4");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN_CUELIST",
			number: 42,
		});
		expect(dispatch).not.toHaveBeenCalledWith({
			type: "OPEN_BUILTIN_CUELIST",
			number: 99,
		});
		shifted("5");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "dynamics",
		});
		shifted("6");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "channels",
		});
		shifted("TIME");
		expect(commandActions.replace).toHaveBeenCalledWith("SPD GRP");
		commandProjection.text = "SPD GRP 1 AT";
		commandProjection.pristine = false;
		shifted("TIME");
		expect(commandActions.replace).toHaveBeenCalledWith(
			"SPD GRP 1 AT SPD GRP",
		);
		shifted("7");
		shifted("8");
		shifted("9");
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_DESK",
			id: "desk-one",
		});
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_DESK",
			id: "desk-two",
		});
		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_DESK",
			id: "desk-three",
		});
		shifted("CLR");
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "systemControlsOpen",
			value: true,
		});
		shifted("DEL");
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_MODAL",
			modal: "systemControlsOpen",
			value: true,
		});
	});

	it("opens Cuelists without selecting stale bootstrap Playback state while desk authority loads", () => {
		server.playbacks.selected_playback = 99;
		playbackDesk = { active_page: 1, selected_playback: 42 };
		runtimeStatus = "loading";
		render(<NumericPad />);

		fireEvent.click(screen.getByRole("button", { name: "SHIFT" }));
		fireEvent.click(screen.getByRole("button", { name: "4" }));

		expect(dispatch).toHaveBeenCalledWith({
			type: "OPEN_BUILTIN",
			kind: "cuelists",
		});
		expect(dispatch).not.toHaveBeenCalledWith(
			expect.objectContaining({ type: "OPEN_BUILTIN_CUELIST" }),
		);
	});
});
