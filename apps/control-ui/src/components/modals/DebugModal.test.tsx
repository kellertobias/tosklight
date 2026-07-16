import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DebugModal } from "./DebugModal";

const dispatch = vi.fn();
const simulateError = vi.fn();
const readServerLogs = vi.fn().mockResolvedValue([]);

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { debugOpen: true, midiProfile: false, touchScrollbars: false, showSectionNames: false }, dispatch }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => ({ bootstrap: { output_health: { frame_hz: 44, deadline_misses: 2, send_errors: 1 } }, readServerLogs, simulateError }) }));

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
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Show section names", "⚒Open Development", "Simulate Hardware", "Simulate Touch Scroll Bars", "Simulate DMX Error", "Clear Simulated Errors"]);
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

  it("opens Development and closes the status stack", () => {
    render(<DebugModal/>);
    fireEvent.click(screen.getByRole("button", { name: /Debug/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Open Development" }));
    expect(dispatch).toHaveBeenNthCalledWith(1, { type: "OPEN_BUILTIN", kind: "development" });
    expect(dispatch).toHaveBeenNthCalledWith(2, { type: "SET_MODAL", modal: "debugOpen", value: false });
    expect(dispatch).toHaveBeenNthCalledWith(3, { type: "SET_MODAL", modal: "setupOpen", value: false });
  });
});
