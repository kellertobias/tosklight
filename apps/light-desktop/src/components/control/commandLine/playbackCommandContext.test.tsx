import { renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { usePlaybackCommandContext } from "./playbackCommandContext";

const authority = vi.hoisted(() => ({
	desk: { selected_surface_id: "main", command_page: 2 },
	surfaces: {
		main: { name: "Main" },
		"screen:second": { name: "Screen 1" },
		"pane:wide": { name: "Evening looks" },
	},
	screens: [{ surface_id: "screen:second", desired_open: false }],
}));
vi.mock("../../../features/playbackRuntime/PlaybackRuntimeView", () => ({
	usePlaybackDeskView: () => authority.desk,
}));
vi.mock("../../../features/screens/ScreensContext", () => ({
	useScreensOptional: () => ({
		surfaces: { playback_surfaces: authority.surfaces },
		screens: { screens: authority.screens },
	}),
}));

describe("Playback command context", () => {
	it("names Main, retained closed Screen and pane authority without selecting a fallback", () => {
		const { result, rerender } = renderHook(() =>
			usePlaybackCommandContext([
				{
					panes: [{ surface_id: "pane:wide", title: "Evening looks renamed" }],
				},
			]),
		);
		expect(result.current).toEqual({ name: "Main", page: 2 });
		authority.desk = { selected_surface_id: "screen:second", command_page: 7 };
		rerender();
		expect(result.current).toEqual({ name: "Screen 1", page: 7 });
		expect(authority.screens[0].desired_open).toBe(false);
		authority.desk.command_page = 8;
		rerender();
		expect(result.current).toEqual({ name: "Screen 1", page: 8 });
		authority.desk = { selected_surface_id: "pane:wide", command_page: 9 };
		rerender();
		expect(result.current).toEqual({ name: "Evening looks renamed", page: 9 });
	});
});
