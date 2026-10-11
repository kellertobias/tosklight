import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { PaneSettingsModal } from "./PaneSettingsModal";
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		dispatch: vi.fn(),
		state: {
			paneSettingsId: "playbacks",
			activeDeskId: "desk",
			maximizedPaneId: null,
			desks: [
				{
					id: "desk",
					panes: [
						{
							id: "playbacks",
							kind: "playbacks",
							title: "Playbacks",
							x: 1,
							y: 1,
							width: 12,
							height: 18,
						},
					],
				},
			],
		},
	}),
}));
vi.mock("./cuePaneCuelistAuthority", () => ({
	useCuePaneCuelistPlaybacks: () => [],
}));
vi.mock("../../windows/fixtureSheetCuelistAuthority", () => ({
	useFixtureSheetCuelistAuthority: () => ({
		cueLists: [],
		selectedCueListId: "",
	}),
}));
vi.mock("./PlaybackPaneSettings", () => ({
	PlaybackPaneSettings: () => <p>Playback layout controls</p>,
}));
afterEach(cleanup);
it("names the unified pane settings tab Playbacks and opens its existing controls", () => {
	render(<PaneSettingsModal />);
	const tab = screen.getByRole("tab", { name: "Playbacks" });
	expect(screen.queryByRole("tab", { name: "Virtual Playbacks" })).toBeNull();
	fireEvent.click(tab);
	expect(screen.getByText("Playback layout controls")).toBeVisible();
});
