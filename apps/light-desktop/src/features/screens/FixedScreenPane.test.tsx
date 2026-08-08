import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FixedScreenPane as FixedScreenPaneConfiguration } from "../../api/types";
import { FixedScreenPane } from "./FixedScreenPane";

const mocks = vi.hoisted(() => ({
	readTextFile: vi.fn(),
}));

vi.mock("../files/FilesContext", () => ({
	useFiles: () => ({ readTextFile: mocks.readTextFile }),
}));
vi.mock("../../windows/FixtureSheetWindow", () => ({
	FixtureSheetWindow: (props: Record<string, unknown>) => (
		<div
			data-testid="fixture-sheet"
			data-view-only={String(props.viewOnly)}
			data-heads={String(props.fixtureSheetIncludedHeads)}
			data-compact-mode={String(props.fixtureSheetCompactMode)}
		/>
	),
}));
vi.mock("../../windows/StageWindow", () => ({
	StageWindow: (props: Record<string, unknown>) => (
		<div
			data-testid={`stage-${String(props.stageView)}`}
			data-view-only={String(props.viewOnly)}
			data-show-selection={String(props.showSelection)}
		/>
	),
}));
vi.mock("../../windows/CuelistWindow", () => ({
	CuelistWindow: (props: Record<string, unknown>) => (
		<div
			data-testid="fixed-cues"
			data-view-only={String(props.viewOnly)}
			data-cue-list-id={String(props.fixedCueListId)}
		/>
	),
}));

function fixtureSheet(): FixedScreenPaneConfiguration {
	return {
		type: "fixture_sheet",
		included_heads: "no_sub_heads",
		order: "active",
		active_only: true,
		compact_mode: "text_only",
		cue_list_id: null,
		columns: ["id", "name", "intensity"],
		show_type: true,
		show_group_shortcuts: true,
	};
}

describe("FixedScreenPane", () => {
	beforeEach(() => {
		mocks.readTextFile.mockReset();
	});
	afterEach(cleanup);

	it("renders every operator pane through an explicit view-only contract", () => {
		const view = render(<FixedScreenPane pane={fixtureSheet()} />);
		expect(screen.getByTestId("fixture-sheet")).toHaveAttribute(
			"data-view-only",
			"true",
		);
		expect(screen.getByTestId("fixture-sheet")).toHaveAttribute(
			"data-heads",
			"no-sub-heads",
		);
		expect(screen.getByTestId("fixture-sheet")).toHaveAttribute(
			"data-compact-mode",
			"text-only",
		);

		view.rerender(
			<FixedScreenPane
				pane={{
					type: "stage_2d",
					follow_preload: false,
					show_floor_grid: true,
				}}
			/>,
		);
		expect(screen.getByTestId("stage-2d")).toHaveAttribute(
			"data-view-only",
			"true",
		);
		expect(screen.getByTestId("stage-2d")).toHaveAttribute(
			"data-show-selection",
			"false",
		);

		view.rerender(
			<FixedScreenPane
				pane={{
					type: "stage_3d",
					follow_preload: true,
					show_floor_grid: false,
					show_beam_guides: true,
					render_quality: "full",
					environment_brightness: 0.4,
				}}
			/>,
		);
		expect(screen.getByTestId("stage-3d")).toHaveAttribute(
			"data-view-only",
			"true",
		);

		view.rerender(
			<FixedScreenPane pane={{ type: "cues", cue_list_id: "cue-list-id" }} />,
		);
		expect(screen.getByTestId("fixed-cues")).toHaveAttribute(
			"data-cue-list-id",
			"cue-list-id",
		);
		expect(screen.getByTestId("fixed-cues")).toHaveAttribute(
			"data-view-only",
			"true",
		);
	});

	it("shows selected text without editor, file, toolbar, or save controls", async () => {
		mocks.readTextFile.mockResolvedValue({
			root_id: "shows",
			path: "running.md",
			text: "# Running order",
			revision: "one",
			read_only: false,
		});

		render(
			<FixedScreenPane
				pane={{
					type: "text",
					root: "shows",
					path: "running.md",
					mode: "markdown",
				}}
			/>,
		);

		expect(
			await screen.findByRole("heading", { name: "Running order" }),
		).toBeVisible();
		expect(screen.queryByRole("button")).not.toBeInTheDocument();
		expect(screen.queryByLabelText("File text")).not.toBeInTheDocument();
		expect(screen.queryByText("Open File")).not.toBeInTheDocument();
		expect(screen.queryByText("Save")).not.toBeInTheDocument();
	});

	it("keeps a missing text identity unavailable instead of substituting content", async () => {
		mocks.readTextFile.mockRejectedValue(new Error("not found"));

		render(
			<FixedScreenPane
				pane={{
					type: "text",
					root: "shows",
					path: "missing.txt",
					mode: "plain",
				}}
			/>,
		);

		await waitFor(() =>
			expect(screen.getByRole("status")).toHaveTextContent("Text unavailable"),
		);
		expect(screen.getByRole("status")).toHaveTextContent("missing.txt");
		expect(screen.queryByLabelText("Plain Text")).not.toBeInTheDocument();
	});
});
