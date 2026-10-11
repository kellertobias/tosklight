import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ScreensProvider } from "../../features/screens/ScreensContext";
import type { ScreensContextValue } from "../../features/screens/types";
import type { PaneModel } from "../../types";
import { PlaybackPaneSettings } from "./PlaybackPaneSettings";
const dispatch = vi.fn();
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ dispatch }) }));
afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});
const pane = {
	id: "one",
	kind: "playbacks",
	title: "Playbacks",
	x: 0,
	y: 0,
	width: 8,
	height: 6,
} as PaneModel;
function setup() {
	const updatePlaybackSurface = vi.fn().mockResolvedValue(undefined);
	const layout = {
		rows: [
			{
				first_position: 201,
				count: 10,
				presentation: "buttons" as const,
				button_count: 1,
			},
		],
	};
	const registry = {
		main: {
			name: "Main",
			layout: { rows: [] },
			page_source: { type: "own" as const, page: 1 },
		},
		second: {
			name: "Second",
			layout,
			page_source: { type: "pinned" as const, page: 2 },
		},
	};
	const source = {
		surfaces: {
			playback_surfaces: registry,
			selected_surface_id: "main",
			resolved_pages: { main: 1, second: 2 },
			command_page: 1,
		},
		screens: {
			screens: [{ id: "two", name: "Second Screen", surface_id: "second" }],
			surfaces: null,
		},
		updatePlaybackSurface,
	} as unknown as ScreensContextValue;
	render(
		<ScreensProvider source={source}>
			<PlaybackPaneSettings pane={pane} />
		</ScreensProvider>,
	);
	return { updatePlaybackSurface, registry, layout };
}
describe("central pane playback configuration", () => {
	it("reviews Take from Screen without writing, and cancel leaves registry and assignments untouched", () => {
		const { updatePlaybackSurface, layout } = setup();
		fireEvent.click(
			screen.getByRole("button", { name: "Choose a screen layout" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Second Screen" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Review copied layout" }),
		);
		expect(
			screen.getByRole("dialog", { name: "Configure Playbacks" }),
		).toBeInTheDocument();
		expect(screen.getByLabelText("First Playback Number")).toHaveValue("201");
		fireEvent.change(screen.getByLabelText("First Playback Number"), {
			target: { value: "401" },
		});
		expect(updatePlaybackSurface).not.toHaveBeenCalled();
		fireEvent.click(
			screen.getByRole("button", { name: "Close playback configuration" }),
		);
		expect(updatePlaybackSurface).not.toHaveBeenCalled();
		expect(layout.rows[0].first_position).toBe(201);
	});
	it("saves only reviewed layout to the pane surface and preserves the source page context", async () => {
		const { updatePlaybackSurface, registry, layout } = setup();
		fireEvent.click(
			screen.getByRole("button", { name: "Choose a screen layout" }),
		);
		fireEvent.click(screen.getByRole("option", { name: "Second Screen" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Review copied layout" }),
		);
		fireEvent.click(screen.getByRole("button", { name: "Apply" }));
		await waitFor(() =>
			expect(updatePlaybackSurface).toHaveBeenCalledWith("pane:one", {
				name: "Playbacks",
				layout,
				page_source: { type: "follow", surface_id: "main" },
			}),
		);
		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_PANE_PLAYBACK_SURFACE",
			id: "one",
			surfaceId: "pane:one",
		});
		expect(layout.rows[0].first_position).toBe(201);
	});
});
