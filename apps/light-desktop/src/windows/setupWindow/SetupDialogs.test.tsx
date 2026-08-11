import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SetupWindowController } from "./controller";
import { SetupDialogs } from "./SetupDialogs";

vi.mock("../../components/setup/ScreensSetup", () => ({
	ProgrammerControlSurfaceSettings: () => <div>Encoder owner fields</div>,
}));

afterEach(cleanup);

describe("Desk Setup dialogs", () => {
	it("owns encoder placement in a closeable modal", () => {
		const setEncoderPlacementOpen = vi.fn();
		render(
			<SetupDialogs
				controller={
					{
						fixtureLibraryOpen: false,
						deskLockSettingsOpen: false,
						encoderPlacementOpen: true,
						setEncoderPlacementOpen,
					} as unknown as SetupWindowController
				}
			/>,
		);
		expect(
			screen.getByRole("dialog", { name: "Encoder placement" }),
		).toHaveTextContent("Encoder owner fields");
		fireEvent.click(
			screen.getByRole("button", { name: "Close encoder placement" }),
		);
		expect(setEncoderPlacementOpen).toHaveBeenCalledWith(false);
	});
});
