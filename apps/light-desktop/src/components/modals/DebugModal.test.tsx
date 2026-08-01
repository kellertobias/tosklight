import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DebugModal, isMajorDeskEvent } from "./DebugModal";

const dispatch = vi.fn();
const simulateError = vi.fn();
const readServerLogs = vi.fn().mockResolvedValue([]);

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { debugOpen: true, midiProfile: false, touchScrollbars: false, showSectionNames: false }, dispatch }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => ({ bootstrap: { output_health: { frame_hz: 44, deadline_misses: 2, send_errors: 1 } }, readServerLogs, simulateError }) }));
vi.mock("../../features/shellStatus/ShellStatusActionsProvider", () => ({
  useShellStatusActions: () => ({ readServerLogs, simulateError }),
}));
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({
  useOutputHealth: () => ({ frame_hz: 44, deadline_misses: 2, send_errors: 1 }),
}));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("DebugModal", () => {
  it("shows diagnostics as Desk Status and keeps simulators in the Debug title menu", () => {
    render(<DebugModal/>);
    expect(screen.getByRole("dialog", { name: "Desk Status" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Desk Status" })).toBeInTheDocument();
    expect(screen.getByText("Current frame rate")).toBeInTheDocument();
    expect(screen.getByText("Scheduler deadline misses")).toBeInTheDocument();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Debug/ }));
    expect(screen.getByRole("menu", { name: "Debug" })).toBeInTheDocument();
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Show section names", "Simulate Hardware", "Simulate Touch Scroll Bars", "Simulate DMX Error", "Clear Simulated Errors"]);
    fireEvent.click(screen.getByRole("menuitem", { name: "Simulate Hardware" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "TOGGLE_MIDI_PROFILE" });
  });

  it("enables the section map and closes the status stack", () => {
    render(<DebugModal/>);
    fireEvent.click(screen.getByRole("button", { name: /Debug/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Show section names" }));
    expect(dispatch).toHaveBeenNthCalledWith(1, { type: "TOGGLE_SECTION_NAMES" });
    expect(dispatch).toHaveBeenNthCalledWith(2, { type: "SET_MODAL", modal: "debugOpen", value: false });
    expect(dispatch).toHaveBeenNthCalledWith(3, { type: "SET_MODAL", modal: "setupOpen", value: false });
  });

	it("shows only major desk events and starts its incremental cursor at zero", async () => {
		readServerLogs.mockResolvedValueOnce([
			{ revision: 1, kind: "command", payload: { text: "Fixture 1" } },
			{ revision: 2, kind: "session_started", payload: { desk: "main" } },
			{ revision: 3, kind: "playback_rejected", payload: { error: "busy" } },
		]);
		render(<DebugModal />);
		await waitFor(() => expect(readServerLogs).toHaveBeenCalledWith(0));
		expect(await screen.findByText(/session_started/)).toBeVisible();
		expect(screen.getByText(/playback_rejected/)).toBeVisible();
		expect(screen.queryByText(/Fixture 1/)).not.toBeInTheDocument();
		expect(screen.getByRole("heading", { name: "Major desk events" })).toBeVisible();
	});

	it("classifies connection changes and errors as major, but not command traffic", () => {
		expect(
			isMajorDeskEvent({
				revision: 1,
				kind: "hardware_connection_changed",
				payload: {},
			}),
		).toBe(true);
		expect(
			isMajorDeskEvent({ revision: 2, kind: "media_server_offline", payload: {} }),
		).toBe(true);
		expect(
			isMajorDeskEvent({ revision: 3, kind: "command", payload: {} }),
		).toBe(false);
	});
});
