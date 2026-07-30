import { fireEvent, render, screen } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { describe, expect, it, vi } from "vitest";
import { PaneSettingsModal } from "./PaneSettingsModal";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
}));

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: {
			activeDeskId: "desk",
			paneSettingsId: "layout-a",
			maximizedPaneId: null,
			desks: [
				{
					id: "desk",
					name: "Desk",
					panes: [
						{
							id: "layout-a",
							kind: "layout",
							title: "Front Layout",
							x: 0,
							y: 0,
							width: 8,
							height: 8,
							layoutGroupId: "deleted",
						},
					],
				},
			],
		},
		dispatch: mocks.dispatch,
	}),
}));
vi.mock("../../features/showObjects/ShowObjectsView", () => ({
	useShowObjectView: vi.fn(),
}));
vi.mock("../../features/showObjects/ShowObjectsState", () => ({
	usePortableGroups: () => [
		{ id: "1", body: { name: "Front", fixtures: [] } },
		{ id: "2", body: { name: "Back", fixtures: [] } },
	],
}));
vi.mock("./cuePaneCuelistAuthority", () => ({
	useCuePaneCuelistPlaybacks: () => [],
}));

describe("Layout Pane Settings", () => {
	it("retains an unavailable stable ID and changes only the configured pane", () => {
		render(<PaneSettingsModal />, { wrapper: ModalProvider });
		fireEvent.click(screen.getByRole("tab", { name: "Layout" }));
		const group = screen.getByRole("button", {
			name: "Unavailable · deleted",
		});
		fireEvent.click(group);
		expect(
			screen.getByRole("option", { name: "Unavailable · deleted" }),
		).toBeInTheDocument();
		fireEvent.click(screen.getByRole("option", { name: "2 · Back" }));
		expect(mocks.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_LAYOUT_GROUP",
			id: "layout-a",
			groupId: "2",
		});
	});
});
