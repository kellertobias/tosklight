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

	it("accepts camera positions in the same metres shown on the encoder", async () => {
		render(<StageVizCameraControls hardwareConnected={false} />);
		await screen.findByText("3.0m");
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 3 · Z Pos value" }),
		);
		expect(
			screen.getByRole("textbox", { name: "Enc 3 · Z Pos value" }),
		).toHaveTextContent("3");
		for (const key of ["1", "5", "ENTER"])
			fireEvent.click(screen.getByRole("button", { name: key }));
		expect(bridge.placeStagePaneCamera).toHaveBeenCalledWith({ z: 15 });
	});

	it("accepts camera angles in the same degrees shown on the encoder", async () => {
		render(<StageVizCameraControls hardwareConnected={false} />);
		await screen.findByText("3.0m");
		fireEvent.click(screen.getByRole("button", { name: "Direction" }));
		fireEvent.click(
			screen.getByRole("button", { name: "Set Enc 1 · Pan value" }),
		);
		expect(
			screen.getByRole("textbox", { name: "Enc 1 · Pan value" }),
		).toHaveTextContent("4");
		for (const key of ["4", "5", "ENTER"])
			fireEvent.click(screen.getByRole("button", { name: key }));
		expect(bridge.placeStagePaneCamera).toHaveBeenCalledWith({ pan: 45 });
	});

	it("routes attached encoder fine and coarse turns to the renderer camera", async () => {
		render(<StageVizCameraControls hardwareConnected />);
		await screen.findByRole("button", { name: "Encoder 1: X Pos, 1.0m" });

		// The encoder is only listened for once the camera has arrived, and a turn that lands
		// before then is simply not heard, so the turn is repeated until one is.
		await waitFor(() => {
			fireEvent(
				window,
				new CustomEvent("light:encoder-action", {
					detail: { control: "encode/1", value: "right" },
				}),
			);
			expect(bridge.placeStagePaneCamera).toHaveBeenCalledWith({ x: 2 });
		});
	});
});
