import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StageCommandControls } from "./StageCommandControls";

const dispatch = vi.fn();
const hardware = vi.hoisted(() => ({ connected: true }));
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
	useHardwareConnected: () => hardware.connected,
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
	hardware.connected = true;
});

describe("StageCommandControls desk encoders", () => {
	it("routes ordinary and press-turn movement at fine and coarse steps", () => {
		render(<StageCommandControls />);
		for (const [control, value] of [
			["encode/2", "up"],
			["encode/2", "right"],
			["encode/4", "down"],
			["encode/5", "left"],
		] as const)
			fireEvent(
				window,
				new CustomEvent("light:encoder-action", {
					detail: { control, value },
				}),
			);

		expect(dispatch.mock.calls.map(([action]) => action)).toEqual([
			{ type: "SET_STAGE_NAVIGATION", panX: 11 },
			{ type: "SET_STAGE_NAVIGATION", panX: 15 },
			{ type: "SET_STAGE_NAVIGATION", orbitX: 29 },
			{ type: "SET_STAGE_NAVIGATION", orbitY: 35 },
		]);
	});

	it("uses the shared accessible touch encoder when no hardware is attached", () => {
		hardware.connected = false;
		render(<StageCommandControls />);

		const xPan = screen.getByRole("group", { name: "Enc 2 · X Pan" });
		expect(xPan).toHaveTextContent("10");
		fireEvent.keyDown(xPan, { key: "ArrowUp" });

		expect(dispatch).toHaveBeenCalledWith({
			type: "SET_STAGE_NAVIGATION",
			panX: 11,
		});
	});

	it("keeps click and undeclared encoder slots mutation-free", () => {
		render(<StageCommandControls />);
		for (const [control, value] of [
			["encode/2", "press"],
			["encode/6", "right"],
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
