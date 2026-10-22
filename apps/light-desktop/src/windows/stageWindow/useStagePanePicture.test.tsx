// @vitest-environment jsdom

import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NativeStagePane } from "./useNativeStagePane";
import { stageViewMode, useStagePanePicture } from "./useStagePanePicture";

const bridge = vi.hoisted(() => ({ setStagePanePicture: vi.fn() }));
vi.mock("../../platform/desktop", () => ({ useDesktopBridge: () => bridge }));
vi.mock("../../state/AppContext", () => ({
	useApp: () => ({
		state: {
			desks: [],
			activeDeskId: "main",
			stageVizAtmosphere: 1,
			stageEnvironmentBrightness: 0.3,
			stageVizQuality: "ultra",
			stageVizExposure: 1,
			stageVizLaserBrightness: 1,
			stageVizShowLabels: true,
			stageShowFloorGrid: true,
			stageShowBeamGuides: true,
			stageVizBackground: "#080a10",
		},
	}),
}));

describe("Control Stage picture admission", () => {
	afterEach(() => {
		cleanup();
		vi.clearAllMocks();
	});

	it.each(["3d", "3d-viz"] as const)("maps saved %s to wireframe", (view) => {
		expect(stageViewMode(view, "top")).toBe("lines_3d");
	});

	it.each([
		["top", "top_down"],
		["front", "front_to_back"],
		["back", "back_to_front"],
		["left", "left_to_right"],
		["right", "right_to_left"],
	] as const)("retains the %s orthographic plan", (side, expected) => {
		expect(stageViewMode("2d", side)).toBe(expected);
	});

	it.each([
		"full_3d",
		"simple_3d",
		"lines_3d",
		"unknown",
	])("rejects heavy quality and rendering for %s even with saved Ultra settings", (mode) => {
		renderHook(() =>
			useStagePanePicture(
				{ id: "stage", active: true } as NativeStagePane,
				mode,
				true,
			),
		);
		expect(bridge.setStagePanePicture).toHaveBeenCalledWith(
			"stage",
			expect.objectContaining({
				mode: "lines_3d",
				quality: "draft",
				atmosphere: 0,
				followPreload: true,
			}),
		);
	});

	it.each([
		"top_down",
		"front_to_back",
		"back_to_front",
		"left_to_right",
		"right_to_left",
	])("sends the %s plan without promoting it to full rendering", (mode) => {
		renderHook(() =>
			useStagePanePicture(
				{ id: "stage", active: true } as NativeStagePane,
				mode,
			),
		);
		expect(bridge.setStagePanePicture).toHaveBeenCalledWith(
			"stage",
			expect.objectContaining({ mode, quality: "draft" }),
		);
	});

	it("sends no settings to an inactive native pane", () => {
		renderHook(() =>
			useStagePanePicture({ id: "stage", active: false } as NativeStagePane),
		);
		expect(bridge.setStagePanePicture).not.toHaveBeenCalled();
	});
});
