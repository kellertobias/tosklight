import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaneModel } from "../../types";
import { createVisualizationRow } from "../../windows/visualizationPaneModel";
import { VisualizationPaneSettings } from "./VisualizationPaneSettings";

const app = vi.hoisted(() => ({ dispatch: vi.fn() }));
vi.mock("../../state/AppContext", () => ({ useApp: () => app }));
vi.mock("../../features/patch/PatchState", () => ({
	usePatchedFixturesView: () => [],
}));

afterEach(() => {
	cleanup();
	app.dispatch.mockReset();
});

const pane = (
	visualizationRows: PaneModel["visualizationRows"],
): PaneModel => ({
	id: "visualization-pane",
	kind: "visualization",
	title: "Visualization",
	x: 1,
	y: 1,
	width: 8,
	height: 6,
	visualizationRows,
});

describe("Visualization pane settings", () => {
	it("adds and removes rows and adds side-by-side widgets through persisted pane actions", () => {
		const { rerender } = render(<VisualizationPaneSettings pane={pane([])} />);
		fireEvent.click(screen.getByRole("button", { name: "Add row" }));
		expect(app.dispatch).toHaveBeenCalledWith(
			expect.objectContaining({
				type: "SET_PANE_VISUALIZATION_ROWS",
				id: "visualization-pane",
				rows: [expect.objectContaining({ widgets: [expect.any(Object)] })],
			}),
		);

		app.dispatch.mockReset();
		const row = createVisualizationRow("row-1");
		rerender(<VisualizationPaneSettings pane={pane([row])} />);
		fireEvent.click(screen.getByRole("button", { name: "Add widget" }));
		expect(app.dispatch).toHaveBeenCalledWith(
			expect.objectContaining({
				rows: [
					expect.objectContaining({
						widgets: expect.arrayContaining([
							expect.objectContaining({ id: row.widgets[0].id }),
							expect.any(Object),
						]),
					}),
				],
			}),
		);

		app.dispatch.mockReset();
		fireEvent.click(screen.getByRole("button", { name: "Remove row" }));
		expect(app.dispatch).toHaveBeenCalledWith(
			expect.objectContaining({ rows: [] }),
		);
	});

	it("exposes source, processing, scale, and type-specific controls", () => {
		const row = createVisualizationRow("row-settings");
		render(<VisualizationPaneSettings pane={pane([row])} />);
		for (const label of [
			"Widget type",
			"Value source",
			"Universe",
			"Address",
			"Processing",
			"Factor",
			"Display scale",
			"Minimum",
			"Maximum",
			"Decimal places",
			"Unit suffix",
			"Low-value colour",
			"High-value colour",
		])
			expect(screen.getByLabelText(label)).toBeVisible();
	});
});
