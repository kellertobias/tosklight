import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PaneModel } from "../../types";
import { PlaybackPaneViewSettings } from "./PlaybackPaneViewSettings";

const model = vi.hoisted(() => ({
	update: vi.fn(),
	dispatch: vi.fn(),
	source: { type: "follow", surface_id: "main" } as unknown,
}));
vi.mock("../../features/screens/ScreensContext", () => ({
	useScreens: () => ({
		surfaces: {
			playback_surfaces: {
				main: { name: "Main" },
				"screen:second": { name: "Screen 1" },
				"pane:playbacks": { page_source: model.source },
			},
			resolved_pages: { "pane:playbacks": 4 },
		},
		screens: { screens: [{ name: "Screen 1", surface_id: "screen:second" }] },
		updatePlaybackSurface: model.update,
	}),
}));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ dispatch: model.dispatch }),
}));
const pane = {
	id: "playbacks",
	title: "Playbacks",
	surface_id: "pane:playbacks",
	playbackViewMode: "screen",
	playbackScreenSurfaceId: "main",
} as PaneModel;
afterEach(cleanup);
beforeEach(() => {
	model.update.mockReset().mockResolvedValue(undefined);
	model.dispatch.mockReset();
	model.source = { type: "follow", surface_id: "main" };
});
describe("Playback pane live screen and independent page", () => {
	it("switches the live screen reference without copying or changing either screen layout", async () => {
		render(<PlaybackPaneViewSettings pane={pane} />);
		fireEvent.click(screen.getByRole("button", { name: "Screen" }));
		fireEvent.click(screen.getByRole("option", { name: "Screen 1" }));
		await waitFor(() =>
			expect(model.dispatch).toHaveBeenCalledWith({
				type: "SET_PANE_PLAYBACK_VIEW",
				id: "playbacks",
				mode: "screen",
				screenSurfaceId: "screen:second",
			}),
		);
		expect(model.update).toHaveBeenCalledExactlyOnceWith("pane:playbacks", {
			page_source: { type: "follow", surface_id: "screen:second" },
		});
		expect(model.dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_TILE_LAYOUT",
			id: "playbacks",
			layout: undefined,
		});
	});
	it("retains a custom pane page when its displayed screen changes", async () => {
		model.source = { type: "own", page: 7 };
		render(<PlaybackPaneViewSettings pane={pane} />);
		fireEvent.click(screen.getByRole("button", { name: "Screen" }));
		fireEvent.click(screen.getByRole("option", { name: "Screen 1" }));
		await waitFor(() =>
			expect(model.update).toHaveBeenCalledExactlyOnceWith("pane:playbacks", {
				page_source: { type: "own", page: 7 },
			}),
		);
	});
	it("chooses a custom page using the currently resolved pane page", async () => {
		render(<PlaybackPaneViewSettings pane={pane} />);
		fireEvent.click(screen.getByRole("button", { name: "Page source" }));
		fireEvent.click(screen.getByRole("option", { name: "Custom page" }));
		await waitFor(() =>
			expect(model.update).toHaveBeenCalledExactlyOnceWith("pane:playbacks", {
				page_source: { type: "own", page: 4 },
			}),
		);
	});
	it("leaves saved pane references unchanged after authoritative rejection", async () => {
		model.update.mockRejectedValue(new Error("Page follow cycle"));
		render(<PlaybackPaneViewSettings pane={pane} />);
		fireEvent.click(screen.getByRole("button", { name: "Page source" }));
		fireEvent.click(screen.getByRole("option", { name: "Custom page" }));
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Page follow cycle",
		);
		expect(model.dispatch).not.toHaveBeenCalled();
	});
});

describe("Playback pane tile and embedded controls", () => {
	it.each([
		"screen",
		"free-region",
	] as const)("defaults legacy %s panes to tiles and opts in locally", (playbackViewMode) => {
		render(<PlaybackPaneViewSettings pane={{ ...pane, playbackViewMode }} />);
		const toggle = screen.getByRole("switch", {
			name: "Show playback controls",
		});
		expect(toggle).not.toBeChecked();
		fireEvent.click(toggle);
		expect(model.dispatch).toHaveBeenCalledExactlyOnceWith({
			type: "SET_PANE_PLAYBACK_CONTROLS",
			id: "playbacks",
			showControls: true,
		});
		expect(model.update).not.toHaveBeenCalled();
	});
	it("displays and disables an explicitly saved controls choice", () => {
		render(
			<PlaybackPaneViewSettings
				pane={{ ...pane, playbackShowControls: true }}
			/>,
		);
		const toggle = screen.getByRole("switch", {
			name: "Show playback controls",
		});
		expect(toggle).toBeChecked();
		fireEvent.click(toggle);
		expect(model.dispatch).toHaveBeenCalledExactlyOnceWith({
			type: "SET_PANE_PLAYBACK_CONTROLS",
			id: "playbacks",
			showControls: false,
		});
		expect(model.update).not.toHaveBeenCalled();
	});
});
