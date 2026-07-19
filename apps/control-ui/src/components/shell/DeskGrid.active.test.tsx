import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { DeskModel } from "../../types";
import { DeskGrid } from "./DeskGrid";

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
	it("keeps covered panes mounted but marks only the maximized pane active", () => {
		mocks.maximizedPaneId = null;
		mocks.paneProps.length = 0;
		const rendered = render(<DeskGrid desk={desk} />);
		expect(mocks.paneProps).toEqual([
			{ id: "stage", active: true, maximized: false },
			{ id: "groups", active: true, maximized: false },
		]);

		mocks.maximizedPaneId = "stage";
		mocks.paneProps.length = 0;
		rendered.rerender(<DeskGrid desk={desk} />);
		expect(mocks.paneProps).toEqual([
			{ id: "stage", active: true, maximized: true },
			{ id: "groups", active: false, maximized: false },
		]);
		expect(rendered.getByTestId("pane-groups")).toBeInTheDocument();
	});
});
