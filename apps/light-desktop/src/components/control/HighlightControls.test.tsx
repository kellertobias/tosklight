import {
	act,
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HighlightState } from "../../api/types";
import { DEFAULT_SELECTION_GRID_STATE } from "../../features/programmingInteraction/contracts";
import { HighlightControls, highlightStatusLabel } from "./HighlightControls";

const offState: HighlightState = {
	active: false,
	mode: "selection",
	output_enabled: false,
	capture_only: false,
	remembered: [],
	active_index: null,
	active_fixture: null,
	can_previous: false,
	can_next: false,
	owner_user_id: null,
};

const fixtures = [
	{ fixture_id: "fixture-a", number: 101, name: "Stage Left" },
	{ fixture_id: "fixture-b", number: 102, name: "Centre Spot" },
	{ fixture_id: "fixture-c", number: 103, name: "Stage Right" },
];

const server = {
	highlight: offState as HighlightState | null,
	highlightError: null as string | null,
	highlightAction: vi.fn().mockResolvedValue(true),
	dismissHighlightError: vi.fn(),
	selectedFixtures: [] as string[],
	patch: { fixtures: [] },
	session: { user: { id: "operator-a" } },
	shiftArmed: false,
	dispatch: vi.fn(),
	selection: {
		selected: [],
		expression: null,
		revision: 7,
		gestureOpen: false,
		grid: DEFAULT_SELECTION_GRID_STATE,
	},
	cycleGridMethod: vi.fn().mockResolvedValue({ action: "grid_method_cycled" }),
	setGridConfiguration: vi
		.fn()
		.mockResolvedValue({ action: "grid_configuration_set" }),
	reorderFromGrid: vi.fn().mockResolvedValue({ action: "grid_reordered" }),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/highlight/HighlightState", async (importOriginal) => ({
	...(await importOriginal<object>()),
	useHighlightSnapshot: () => server.highlight,
	useHighlightErrorMessage: () => server.highlightError,
	useHighlightActions: () => ({
		highlightAction: server.highlightAction,
		dismissHighlightError: server.dismissHighlightError,
	}),
}));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useSessionSnapshot: () => server.session,
}));
vi.mock(
	"../../features/programmingInteraction/ProgrammingInteractionView",
	() => ({
		useProgrammingSelectionView: () => server.selection,
		useProgrammingSelectionActions: () => ({
			cycleGridMethod: server.cycleGridMethod,
			setGridConfiguration: server.setGridConfiguration,
			reorderFromGrid: server.reorderFromGrid,
		}),
	}),
);
vi.mock("../../state/AppContext", () => ({
	useOptionalApp: () => ({
		state: { shiftArmed: server.shiftArmed },
		dispatch: server.dispatch,
	}),
}));

afterEach(() => {
	cleanup();
	server.highlight = { ...offState, remembered: [] };
	server.highlightError = null;
	server.highlightAction.mockReset().mockResolvedValue(true);
	server.dismissHighlightError.mockReset();
	server.shiftArmed = false;
	server.dispatch.mockReset();
	server.cycleGridMethod
		.mockReset()
		.mockResolvedValue({ action: "grid_method_cycled" });
	server.setGridConfiguration
		.mockReset()
		.mockResolvedValue({ action: "grid_configuration_set" });
	server.reorderFromGrid
		.mockReset()
		.mockResolvedValue({ action: "grid_reordered" });
	server.selection = {
		selected: [],
		expression: null,
		revision: 7,
		gestureOpen: false,
		grid: DEFAULT_SELECTION_GRID_STATE,
	};
	server.selectedFixtures = [];
});

describe("HighlightControls keypad actions", () => {
	it("renders only the four corrected keypad labels and routes ALL to restoration", async () => {
		render(<HighlightControls />);

		const controls = screen.getByRole("region", {
			name: "Highlight and selection stepping",
		});
		expect(
			[...controls.querySelectorAll("button")].map(
				(button) => button.textContent,
			),
		).toEqual(["HIGH", "PREV", "NEXT", "ALL"]);
		expect(
			[...controls.querySelectorAll("[data-keypad-key]")].map((button) =>
				button.getAttribute("data-keypad-key"),
			),
		).toEqual(["HIGH", "PREV", "NEXT", "ALL"]);
		expect(
			screen.getByRole("button", { name: "Turn Highlight on" }),
		).toHaveTextContent(/^HIGH$/);
		expect(screen.queryByText(/capture/i)).not.toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("button", { name: "Restore complete selection" }),
		);
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenCalledWith("all"),
		);
	});

	it("sends explicit On and Off actions and lights HIGH from active state alone", async () => {
		const { rerender } = render(<HighlightControls />);
		const high = () => screen.getByRole("button", { name: /Turn Highlight/ });
		expect(high()).toHaveClass("highlight-off");
		expect(high()).not.toHaveClass("highlight-armed");

		fireEvent.click(high());
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenCalledWith("on"),
		);

		server.highlightAction.mockClear();
		server.highlight = {
			...offState,
			active: true,
			output_enabled: false,
			capture_only: true,
		};
		rerender(<HighlightControls />);
		expect(high()).toHaveClass("highlight-armed");
		expect(high()).toHaveTextContent(/^HIGH$/);
		expect(high()).not.toHaveTextContent(/suppressed|empty|selection/i);
		fireEvent.click(high());

		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenCalledWith("off"),
		);
	});

	it("keeps PREV and NEXT available at the ends so authoritative stepping can wrap", async () => {
		server.highlight = {
			...offState,
			active: false,
			mode: "step",
			remembered: fixtures,
			active_index: 2,
			active_fixture: fixtures[2],
			can_previous: true,
			can_next: true,
			owner_user_id: "operator-a",
		};
		const { rerender } = render(<HighlightControls />);

		expect(
			screen.getByRole("button", { name: "Next selection item" }),
		).toBeEnabled();
		fireEvent.click(
			screen.getByRole("button", { name: "Next selection item" }),
		);
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenCalledWith("next"),
		);

		server.highlightAction.mockClear();
		server.highlight = {
			...server.highlight,
			active_index: 0,
			active_fixture: fixtures[0],
		};
		rerender(<HighlightControls />);
		expect(
			screen.getByRole("button", { name: "Previous selection item" }),
		).toBeEnabled();
		fireEvent.click(
			screen.getByRole("button", { name: "Previous selection item" }),
		);
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenCalledWith("previous"),
		);
	});
});

describe("HighlightControls shortcuts and ownership", () => {
	it("binds Alt+H, Alt+A, and Alt+Left/Right while removing Alt+C", async () => {
		server.highlight = {
			...offState,
			remembered: fixtures,
			can_previous: true,
			can_next: true,
			owner_user_id: "operator-a",
		};
		render(<HighlightControls />);

		fireEvent.keyDown(window, { key: "a", altKey: true });
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenLastCalledWith("all"),
		);
		server.highlightAction.mockClear();

		fireEvent.keyDown(window, { key: "c", altKey: true });
		expect(server.highlightAction).not.toHaveBeenCalled();

		fireEvent.keyDown(window, { key: "ArrowRight", altKey: true });
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenLastCalledWith("next"),
		);
		server.highlightAction.mockClear();

		fireEvent.keyDown(window, { key: "ArrowLeft", altKey: true });
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenLastCalledWith("previous"),
		);
		server.highlightAction.mockClear();

		fireEvent.keyDown(window, { key: "h", altKey: true });
		await waitFor(() =>
			expect(server.highlightAction).toHaveBeenLastCalledWith("toggle"),
		);
		server.highlightAction.mockClear();
		fireEvent.keyDown(window, {
			key: "ArrowRight",
			altKey: true,
			repeat: true,
		});
		expect(server.highlightAction).not.toHaveBeenCalled();
	});

	it("keeps ownership errors outside the lit HIGH key", () => {
		server.highlight = {
			...offState,
			active: true,
			mode: "step",
			output_enabled: false,
			capture_only: true,
			remembered: fixtures,
			active_index: 0,
			active_fixture: fixtures[0],
			can_previous: true,
			can_next: true,
			owner_user_id: "operator-b",
			owner_user_name: "Focus operator",
			message: "Blind mode",
		};
		server.highlightError = "Highlight is controlled by Focus operator.";
		const { container } = render(<HighlightControls />);

		const high = screen.getByRole("button", { name: "Turn Highlight off" });
		expect(high).toHaveClass("highlight-armed");
		expect(high).toHaveTextContent(/^HIGH$/);
		const alert = screen.getByRole("alert");
		expect(alert).toHaveAttribute("data-highlight-error-alert");
		expect(alert).toHaveTextContent(
			"Highlight is controlled by Focus operator",
		);
		expect(alert.parentElement).toBe(document.body);
		expect(container.contains(alert)).toBe(false);
		expect(screen.queryByLabelText("Highlight status")).not.toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Dismiss Highlight error" }),
		);
		expect(server.dismissHighlightError).toHaveBeenCalledOnce();
	});

	it("blocks every live desk action while another operator owns Highlight", () => {
		server.highlight = {
			...offState,
			remembered: fixtures,
			can_previous: true,
			can_next: true,
			owner_user_id: "operator-b",
			owner_user_name: "Focus operator",
		};
		render(<HighlightControls />);

		expect(
			screen.getByRole("button", {
				name: "Highlight is controlled by Focus operator",
			}),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Previous selection item" }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Next selection item" }),
		).toBeDisabled();
		expect(
			screen.getByRole("button", { name: "Restore complete selection" }),
		).toBeDisabled();
	});
});

describe("HighlightControls shifted selection grid actions", () => {
	it("uses physical Shift+Alt+A as a press gesture and never invokes ordinary ALL", async () => {
		render(<HighlightControls />);

		fireEvent.keyDown(window, { key: "a", altKey: true, shiftKey: true });
		expect(server.cycleGridMethod).not.toHaveBeenCalled();
		fireEvent.keyUp(window, { key: "a", altKey: true, shiftKey: true });

		await waitFor(() => expect(server.cycleGridMethod).toHaveBeenCalledOnce());
		expect(server.highlightAction).not.toHaveBeenCalled();
	});

	it("opens Grid Settings at the 650 ms hold boundary without cycling or invoking ALL", () => {
		vi.useFakeTimers();
		try {
			render(<HighlightControls />);
			fireEvent.keyDown(window, { key: "a", altKey: true, shiftKey: true });
			act(() => vi.advanceTimersByTime(650));

			expect(
				screen.getByRole("dialog", { name: "Grid Settings" }),
			).toBeInTheDocument();
			fireEvent.keyUp(window, { key: "a", altKey: true, shiftKey: true });
			expect(server.cycleGridMethod).not.toHaveBeenCalled();
			expect(server.highlightAction).not.toHaveBeenCalled();
		} finally {
			vi.useRealTimers();
		}
	});

	it("routes latched Shift+PREV/NEXT to columns/rows and consumes Shift", async () => {
		server.shiftArmed = true;
		render(<HighlightControls />);

		fireEvent.click(
			screen.getByRole("button", { name: "Previous selection item" }),
		);
		await waitFor(() =>
			expect(server.reorderFromGrid).toHaveBeenCalledWith("columns"),
		);
		await waitFor(() =>
			expect(
				screen.getByRole("region", {
					name: "Highlight and selection stepping",
				}),
			).toHaveAttribute("aria-busy", "false"),
		);
		expect(server.dispatch).toHaveBeenCalledWith({
			type: "SET_SHIFT_ARMED",
			value: false,
		});
		expect(server.highlightAction).not.toHaveBeenCalled();

		server.reorderFromGrid.mockClear();
		fireEvent.keyDown(window, {
			key: "ArrowRight",
			altKey: true,
			shiftKey: true,
		});
		await waitFor(() =>
			expect(server.reorderFromGrid).toHaveBeenCalledWith("rows"),
		);
		expect(server.highlightAction).not.toHaveBeenCalled();
	});

	it("opens authoritative Grid Settings from hardware, shows all methods and conditional XYZ origins, and saves", async () => {
		server.selection = {
			...server.selection,
			grid: {
				configuration: {
					method: "stage2d",
					axisOrigin: { x: 1, y: 2, z: 3 },
				},
				rowsFirst: "bottom_right",
				columnsFirst: "top_right",
			},
		};
		render(<HighlightControls />);

		act(() => window.dispatchEvent(new Event("light:selection-grid-settings")));
		const dialog = screen.getByRole("dialog", { name: "Grid Settings" });
		const method = screen.getByRole("button", { name: "Grid method" });
		fireEvent.click(method);
		expect(
			screen.getAllByRole("option").map((option) => option.textContent),
		).toEqual([
			"2D Stage",
			"Top to Bottom",
			"Bottom to Top",
			"Front to Back",
			"Back to Front",
			"Left to Right",
			"Right to Left",
			"Horizontal axis (X)",
			"Vertical axis (Z)",
			"Room-depth axis (Y)",
		]);
		expect(dialog).toHaveTextContent("Rows-first next");
		expect(dialog).toHaveTextContent("Bottom right");
		expect(dialog).toHaveTextContent("Columns-first next");
		expect(dialog).toHaveTextContent("Top right");
		expect(screen.queryByLabelText("X")).not.toBeInTheDocument();

		fireEvent.click(
			screen.getByRole("option", { name: "Horizontal axis (X)" }),
		);
		expect(screen.getByLabelText("X")).toHaveValue("1");
		expect(screen.getByLabelText("Y")).toHaveValue("2");
		expect(screen.getByLabelText("Z")).toHaveValue("3");
		fireEvent.change(screen.getByLabelText("X"), { target: { value: "4.5" } });
		fireEvent.click(screen.getByRole("button", { name: "Save Grid Settings" }));

		await waitFor(() =>
			expect(server.setGridConfiguration).toHaveBeenCalledWith({
				method: "horizontal_axis_x",
				axisOrigin: { x: 4.5, y: 2, z: 3 },
			}),
		);
		await waitFor(() =>
			expect(
				screen.queryByRole("dialog", { name: "Grid Settings" }),
			).not.toBeInTheDocument(),
		);
	});

	it("keeps Grid Settings open with an actionable error when the CAS writer rejects the save", async () => {
		server.setGridConfiguration.mockResolvedValue(null);
		render(<HighlightControls />);
		act(() => window.dispatchEvent(new Event("light:selection-grid-settings")));
		fireEvent.click(screen.getByRole("button", { name: "Save Grid Settings" }));

		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Grid Settings could not be saved. Refresh and try again.",
		);
		expect(
			screen.getByRole("dialog", { name: "Grid Settings" }),
		).toBeInTheDocument();
	});
});

describe("highlightStatusLabel", () => {
	it("uses a zero-based server index only for display conversion", () => {
		expect(
			highlightStatusLabel({
				...offState,
				mode: "step",
				remembered: fixtures,
				active_index: 0,
				active_fixture: fixtures[0],
				can_next: true,
			}),
		).toBe("STEP 1/3 · Fixture 101 · Stage Left");
	});

	it("reports complete selection independently of HIGH state", () => {
		expect(highlightStatusLabel({ ...offState, remembered: fixtures })).toBe(
			"ALL · 3 selected",
		);
	});
});
