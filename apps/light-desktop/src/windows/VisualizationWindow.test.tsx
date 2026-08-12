import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { VisualizationWidgetView } from "./VisualizationWindow";
import { createVisualizationWidget } from "./visualizationPaneModel";

afterEach(cleanup);

describe("Visualization widgets", () => {
	it("renders text, graph, horizontal and vertical bars, and numeric text", () => {
		const base = createVisualizationWidget("widget");
		const { rerender } = render(
			<VisualizationWidgetView
				widget={{ ...base, title: "Message", type: "text" }}
				value={50}
				revision="1"
			/>,
		);
		expect(screen.getByText("50.0%")).toBeVisible();

		rerender(
			<VisualizationWidgetView
				widget={{ ...base, title: "History", type: "graph" }}
				value={25}
				revision="2"
			/>,
		);
		expect(screen.getByLabelText("History over time")).toBeVisible();
		expect(screen.getByText(/30s/)).toBeVisible();

		rerender(
			<VisualizationWidgetView
				widget={{
					...base,
					title: "Level",
					type: "bar",
					bar: { orientation: "horizontal" },
				}}
				value={75}
				revision="3"
			/>,
		);
		expect(screen.getByRole("meter", { name: "Level" })).toHaveAttribute(
			"value",
			"75",
		);
		expect(
			screen.getByRole("meter", { name: "Level" }).closest("div"),
		).toHaveClass("visualization-bar-horizontal");

		rerender(
			<VisualizationWidgetView
				widget={{
					...base,
					title: "Vertical",
					type: "bar",
					bar: { orientation: "vertical" },
				}}
				value={40}
				revision="4"
			/>,
		);
		expect(
			screen.getByRole("meter", { name: "Vertical" }).closest("div"),
		).toHaveClass("visualization-bar-vertical");

		rerender(
			<VisualizationWidgetView
				widget={{
					...base,
					title: "Number",
					type: "number",
					number: { ...base.number, decimalPlaces: 2, unit: " V" },
				}}
				value={12.345}
				revision="5"
			/>,
		);
		expect(screen.getByText("12.35 V")).toBeVisible();
	});

	it("shows an actionable unavailable state without inventing a zero", () => {
		render(
			<VisualizationWidgetView
				widget={createVisualizationWidget("missing")}
				value={null}
				revision="0"
			/>,
		);
		expect(screen.getByText("Value unavailable")).toBeVisible();
	});
});
