import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StageVizCameraControls } from "./StageVizCameraControls";

const bridge = vi.hoisted(() => ({
	stagePaneCamera: vi.fn(async () => [1, 2, 3, 4, 5, 6] as const),
	placeStagePaneCamera: vi.fn(async () => undefined),
}));

vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => bridge,
}));

afterEach(() => {
	cleanup();
	vi.clearAllMocks();
});

describe("StageVizCameraControls desk encoders", () => {
	it("uses the standard accessible touch encoder surface", async () => {
		render(<StageVizCameraControls hardwareConnected={false} />);
		const xPosition = await screen.findByRole("group", {
			name: "Enc 1 · X Pos",
		});

		expect(xPosition).toHaveTextContent("1.0m");
		fireEvent.keyDown(xPosition, { key: "ArrowUp" });

		expect(bridge.placeStagePaneCamera).toHaveBeenCalledWith({ x: 1.1 });
	});

	it("routes attached encoder fine and coarse turns to the renderer camera", async () => {
		render(<StageVizCameraControls hardwareConnected />);
		await screen.findByRole("button", { name: "Encoder 1: X Pos, 1.0m" });

		fireEvent(
			window,
			new CustomEvent("light:encoder-action", {
				detail: { control: "encode/1", value: "right" },
			}),
		);

		await waitFor(() =>
			expect(bridge.placeStagePaneCamera).toHaveBeenCalledWith({ x: 2 }),
		);
	});
});
