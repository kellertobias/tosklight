import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShowObjectsStore } from "../showObjects/store";
import { useShowObjects } from "./useShowData";
import type { ServerState } from "./useServerState";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

describe("useShowObjects", () => {
	it("leaves Group and Preset hydration to active view sessions", async () => {
		const objects = vi.fn().mockResolvedValue([]);
		const state = {
			client: { objects },
			showObjectsStore: new ShowObjectsStore(),
			showObjectsRequest: { current: 0 },
			setCueObjects: vi.fn(),
			setDeskLayout: vi.fn(),
			setDeskLayoutScope: vi.fn(),
			setOutputRoutes: vi.fn(),
			setPatchLayers: vi.fn(),
			setStageLayout: vi.fn(),
			setUnresolvedMvrFixtures: vi.fn(),
		} as unknown as ServerState;
		const { result } = renderHook(() => useShowObjects(state));

		await act(() => result.current(SHOW_ID, "user-1"));

		const kinds = objects.mock.calls.map((call) => call[1]);
		expect(kinds).toEqual([
			"cue_list",
			"route",
			"user_layout",
			"stage_layout",
			"patch_layer",
			"unresolved_mvr_fixture",
		]);
		expect(kinds).not.toContain("group");
		expect(kinds).not.toContain("preset");
		expect(state.showObjectsStore.getSnapshot().showId).toBe(SHOW_ID);
	});
});
