import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StageCommandControls } from "./StageCommandControls";

const dispatch = vi.fn();
const state = {
	stageMode: "navigate",
	stageView: "3d",
	stageZoom: 1,
	stagePanX: 10,
	stagePanY: 20,
	stageOrbitX: 30,
	stageOrbitY: 40,
	midiProfile: null,
};

vi.mock("../../state/AppContext", () => ({
	useApp: () => ({ state, dispatch }),
}));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
	useHardwareConnected: () => true,
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});

describe("StageCommandControls attached encoder declarations", () => {
	it("routes ordinary and held rotation to the displayed primary and secondary targets", () => {
		render(<StageCommandControls />);
		for (const [control, value] of [
			["encode/2", "up"],
			["encode/2", "right"],
			["encode/3", "down"],
			["encode/3", "left"],
		] as const)
			fireEvent(
				window,
				new CustomEvent("light:encoder-action", {
					detail: { control, value },
				}),
			);

		expect(dispatch.mock.calls.map(([action]) => action)).toEqual([
			{ type: "SET_STAGE_NAVIGATION", panX: 15 },
			{ type: "SET_STAGE_NAVIGATION", panY: 25 },
			{ type: "SET_STAGE_NAVIGATION", orbitX: 25 },
			{ type: "SET_STAGE_NAVIGATION", orbitY: 35 },
		]);
	});

	it("keeps click and undeclared encoder slots mutation-free", () => {
		render(<StageCommandControls />);
		for (const [control, value] of [
			["encode/2", "press"],
			["encode/4", "right"],
		] as const)
			fireEvent(
				window,
				new CustomEvent("light:encoder-action", {
					detail: { control, value },
				}),
			);
		expect(dispatch).not.toHaveBeenCalled();
	});
});
