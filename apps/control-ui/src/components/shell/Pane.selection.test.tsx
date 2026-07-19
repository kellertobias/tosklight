import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PaneModel } from "../../types";
import { Pane } from "./Pane";

const selectionView = vi.hoisted(() =>
	vi.fn((enabled: boolean) =>
		enabled ? { selected: ["fixture-a", "fixture-b"] } : null,
	),
);

vi.mock("../../features/programmingInteraction/ProgrammingInteractionView", () => ({
	useProgrammingSelectionView: selectionView,
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: {
			stageMode: "select",
			stageView: "2d",
			presetFamily: "Intensity",
		},
		dispatch: vi.fn(),
	}),
}));
vi.mock("../../windows/WindowRegistry", () => ({
	windowRegistry: {
		stage: () => <div>Stage body</div>,
		fixtures: () => <div>Fixture body</div>,
		groups: () => <div>Group body</div>,
	},
}));
vi.mock("../common", () => ({ Button: () => null }));
vi.mock("../shared/SourceLegend", () => ({ SourceLegend: () => null }));
vi.mock("../window-kit", () => ({
	WindowHeader: ({ info }: { info?: { primary: React.ReactNode } }) => (
		<header>{info?.primary}</header>
	),
}));
vi.mock("./PaneChromeContext", () => ({
	PaneChromeProvider: ({ children }: React.PropsWithChildren) => children,
}));

function pane(kind: PaneModel["kind"]): PaneModel {
	return {
		id: kind,
		kind,
		title: kind,
		x: 1,
		y: 1,
		width: 12,
		height: 18,
	};
}

afterEach(() => {
	cleanup();
	selectionView.mockClear();
});

describe("Pane selection scope", () => {
	it("observes selection only for an active Stage or Fixture pane", () => {
		const rendered = render(
			<Pane pane={pane("stage")} active maximized={false} editing={false} />,
		);
		expect(selectionView).toHaveBeenLastCalledWith(true);
		expect(screen.getByText("2 selected")).toBeInTheDocument();

		rendered.rerender(
			<Pane
				pane={pane("fixtures")}
				active={false}
				maximized={false}
				editing={false}
			/>,
		);
		expect(selectionView).toHaveBeenLastCalledWith(false);
		expect(screen.getByText("0 selected")).toBeInTheDocument();

		rendered.rerender(
			<Pane pane={pane("groups")} active maximized={false} editing={false} />,
		);
		expect(selectionView).toHaveBeenLastCalledWith(false);
	});
});
