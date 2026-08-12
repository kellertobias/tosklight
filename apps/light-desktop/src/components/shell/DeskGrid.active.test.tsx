import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { DeskModel } from "../../types";
import { DeskGrid, stageGridBackdropRects } from "./DeskGrid";

const mocks = vi.hoisted(() => ({
	maximizedPaneId: null as string | null,
	paneProps: [] as Array<{ id: string; active: boolean; maximized: boolean }>,
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: {
			maximizedPaneId: mocks.maximizedPaneId,
			paneSettingsId: null,
		},
		dispatch: vi.fn(),
	}),
}));
vi.mock("./Pane", () => ({
	Pane: (props: {
		pane: { id: string };
		active: boolean;
		maximized: boolean;
	}) => {
		mocks.paneProps.push({
			id: props.pane.id,
			active: props.active,
			maximized: props.maximized,
		});
		return <div data-testid={`pane-${props.pane.id}`} />;
	},
}));
vi.mock("../modals/WindowPicker", () => ({ WindowPicker: () => null }));
vi.mock("../modals/PaneSettingsModal", () => ({
	PaneSettingsModal: () => null,
}));

const desk: DeskModel = {
	id: "desk",
	name: "Desk",
	panes: [
		{
			id: "stage",
			kind: "stage",
			title: "Stage",
			x: 1,
			y: 1,
			width: 12,
			height: 18,
		},
		{
			id: "groups",
			kind: "groups",
			title: "Groups",
			x: 13,
			y: 1,
			width: 12,
			height: 18,
		},
	],
};

describe("DeskGrid view activity", () => {
	it("keeps the desktop grid only outside Stage pane rectangles", () => {
		expect(stageGridBackdropRects(desk)).toEqual([
			{ x: 13, y: 1, width: 12, height: 18 },
		]);
		expect(
			stageGridBackdropRects({
				...desk,
				panes: [{ ...desk.panes[0], x: 7, y: 5, width: 12, height: 8 }],
			}),
		).toEqual([
			{ x: 1, y: 1, width: 24, height: 4 },
			{ x: 1, y: 5, width: 6, height: 8 },
			{ x: 19, y: 5, width: 6, height: 8 },
			{ x: 1, y: 13, width: 24, height: 6 },
		]);
	});

	it("keeps covered panes mounted but marks only the maximized pane active", () => {
		mocks.maximizedPaneId = null;
		mocks.paneProps.length = 0;
		const rendered = render(<DeskGrid desk={desk} />);
		expect(
			rendered.container.querySelector('[data-ui-component="grid-desktop"]'),
		).toBeInTheDocument();
		expect(mocks.paneProps).toEqual([
			{ id: "stage", active: true, maximized: false },
			{ id: "groups", active: true, maximized: false },
		]);
		const backdrop = rendered.container.querySelector(
			".stage-grid-backdrop",
		) as HTMLElement;
		expect(backdrop).toHaveStyle({
			gridColumn: "13 / span 12",
			gridRow: "1 / span 18",
			"--stage-grid-backdrop-columns": "12",
			"--stage-grid-backdrop-rows": "18",
		});

		mocks.maximizedPaneId = "stage";
		mocks.paneProps.length = 0;
		rendered.rerender(<DeskGrid desk={desk} />);
		expect(mocks.paneProps).toEqual([
			{ id: "stage", active: true, maximized: true },
			{ id: "groups", active: false, maximized: false },
		]);
		expect(
			rendered.container.querySelectorAll(".stage-grid-backdrop"),
		).toHaveLength(0);
		expect(rendered.getByTestId("pane-groups")).toBeInTheDocument();
	});
});
