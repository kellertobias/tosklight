import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaneModel } from "../../types";
import { RunningPaneSettings } from "./PaneSettingsModal";

const dispatch = vi.fn();
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ dispatch, state: {} }),
}));

afterEach(() => {
	cleanup();
	dispatch.mockReset();
});

describe("Running pane settings", () => {
	it("shows every persisted filter and updates only the selected pane", () => {
		const pane = {
			id: "running-pane",
			kind: "running",
			title: "Running",
			x: 1,
			y: 1,
			width: 8,
			height: 6,
			runningFilter: "dynamic",
		} satisfies PaneModel;
		render(<RunningPaneSettings pane={pane} />);

		const filters = screen.getByRole("radiogroup", { name: "Running kind" });
		expect(filters).toBeVisible();
		expect(screen.getByRole("radio", { name: "Dynamics" })).toHaveAttribute(
			"aria-checked",
			"true",
		);
		fireEvent.click(screen.getByRole("radio", { name: "Macros" }));
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_RUNNING_FILTER",
			id: "running-pane",
			filter: "macro",
		});
	});
});
