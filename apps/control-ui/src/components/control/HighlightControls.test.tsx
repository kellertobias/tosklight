import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { HighlightState } from "../../api/types";
import { HighlightControls, highlightStatusLabel } from "./HighlightControls";

const offState: HighlightState = {
  active: false,
  mode: "off",
  output_enabled: false,
  capture_only: false,
  remembered: [],
  active_index: null,
  active_fixture: null,
  can_previous: false,
  can_next: false,
  owner_user_id: null,
};

const fixtures = [
  { fixture_id: "fixture-a", number: 101, name: "Stage Left" },
  { fixture_id: "fixture-b", number: 102, name: "Centre Spot" },
  { fixture_id: "fixture-c", number: 103, name: "Stage Right" },
];

const server = {
  highlight: offState as HighlightState | null,
  highlightError: null as string | null,
  highlightAction: vi.fn().mockResolvedValue(true),
  dismissHighlightError: vi.fn(),
  selectedFixtures: [] as string[],
  patch: { fixtures: [] },
  session: { user: { id: "operator-a" } },
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => {
  cleanup();
  server.highlight = { ...offState, remembered: [] };
  server.highlightError = null;
  server.highlightAction.mockReset().mockResolvedValue(true);
  server.dismissHighlightError.mockReset();
  server.selectedFixtures = [];
});

describe("HighlightControls", () => {
  it("renders the four keypad labels and routes the existing actions to the server", async () => {
    server.selectedFixtures = ["fixture-a", "fixture-b"];
    render(<HighlightControls/>);

    expect(screen.getByRole("region", { name: "Highlight and step through" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Turn Highlight on" })).toHaveTextContent("HIGH");
    expect(screen.getByRole("button", { name: "Previous highlighted fixture" })).toHaveTextContent("PREV");
    expect(screen.getByRole("button", { name: "Next highlighted fixture" })).toHaveTextContent("NEXT");
    expect(screen.getByRole("button", { name: "Capture current selection for Highlight" })).toHaveTextContent("ALL");

    fireEvent.click(screen.getByRole("button", { name: "Capture current selection for Highlight" }));
    await waitFor(() => expect(server.highlightAction).toHaveBeenCalledWith("capture"));
  });

  it("sends explicit On and Off actions so a deliberate rapid toggle is not repeat-guarded", async () => {
    const { rerender } = render(<HighlightControls/>);

    fireEvent.click(screen.getByRole("button", { name: "Turn Highlight on" }));
    await waitFor(() => expect(server.highlightAction).toHaveBeenCalledWith("on"));

    server.highlightAction.mockClear();
    server.highlight = {
      ...offState,
      active: true,
      mode: "selection",
      output_enabled: true,
      remembered: fixtures,
      can_next: true,
      owner_user_id: "operator-a",
    };
    rerender(<HighlightControls/>);
    fireEvent.click(screen.getByRole("button", { name: "Turn Highlight off" }));

    await waitFor(() => expect(server.highlightAction).toHaveBeenCalledWith("off"));
  });

  it("shows all remembered fixtures before stepping and the current fixture while stepping", () => {
    server.highlight = {
      ...offState,
      active: true,
      mode: "selection",
      output_enabled: true,
      remembered: fixtures,
      can_next: true,
      owner_user_id: "operator-a",
    };
    const { rerender } = render(<HighlightControls/>);
    expect(screen.getByRole("region", { name: "Highlight and step through" })).toHaveAttribute("title", expect.stringContaining("Fixture 102 · Centre Spot"));
    expect(screen.getByRole("button", { name: "Turn Highlight off" })).toHaveTextContent("All 3");

    server.highlight = {
      ...server.highlight,
      mode: "step",
      active_index: 1,
      active_fixture: fixtures[1],
      can_previous: true,
      can_next: true,
    };
    rerender(<HighlightControls/>);
    expect(screen.getByRole("region", { name: "Highlight and step through" })).toHaveAttribute("title", expect.stringContaining("2/3 · Fixture 102 · Centre Spot"));
    expect(screen.getByRole("button", { name: "Turn Highlight off" })).toHaveTextContent("2/3 · Fixture 102 · Centre Spot");
  });

  it("stops at the ends and never advances its displayed index locally", async () => {
    server.highlight = {
      ...offState,
      active: true,
      mode: "step",
      output_enabled: true,
      remembered: fixtures,
      active_index: 2,
      active_fixture: fixtures[2],
      can_previous: true,
      can_next: false,
      owner_user_id: "operator-a",
    };
    render(<HighlightControls/>);

    expect(screen.getByRole("button", { name: "Next highlighted fixture" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Previous highlighted fixture" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Previous highlighted fixture" }));
    await waitFor(() => expect(server.highlightAction).toHaveBeenCalledWith("previous"));
    expect(screen.getByRole("region", { name: "Highlight and step through" })).toHaveAttribute("title", expect.stringContaining("3/3 · Fixture 103 · Stage Right"));
  });

  it("provides keyboard actions without sending repeated or unavailable steps", async () => {
    server.highlight = {
      ...offState,
      active: true,
      mode: "step",
      output_enabled: true,
      remembered: fixtures,
      active_index: 0,
      active_fixture: fixtures[0],
      can_previous: false,
      can_next: true,
      owner_user_id: "operator-a",
    };
    render(<HighlightControls/>);

    fireEvent.keyDown(window, { key: "ArrowRight", altKey: true });
    await waitFor(() => expect(server.highlightAction).toHaveBeenCalledWith("next"));
    server.highlightAction.mockClear();
    fireEvent.keyDown(window, { key: "ArrowLeft", altKey: true });
    fireEvent.keyDown(window, { key: "ArrowRight", altKey: true, repeat: true });
    expect(server.highlightAction).not.toHaveBeenCalled();
  });

  it("makes capture-only safety and ownership conflicts explicit", () => {
    server.highlight = {
      ...offState,
      active: true,
      mode: "step",
      output_enabled: false,
      capture_only: true,
      remembered: fixtures,
      active_index: 0,
      active_fixture: fixtures[0],
      can_next: true,
      owner_user_id: "operator-b",
      owner_user_name: "Focus operator",
      message: "Blind mode",
    };
    server.highlightError = "Highlight is controlled by Focus operator.";
    render(<HighlightControls/>);

    expect(screen.getByRole("region", { name: "Highlight and step through" })).toHaveClass("capture-only");
    expect(screen.getByRole("region", { name: "Highlight and step through" })).toHaveAttribute("title", expect.stringContaining("Capture only; no live highlight output"));
    expect(screen.getByText("CAPTURE ONLY")).toBeVisible();
    expect(screen.getByRole("button", { name: "Turn Highlight off" })).toBeEnabled();
    expect(screen.getByRole("alert")).toHaveTextContent("Highlight is controlled by Focus operator");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss Highlight error" }));
    expect(server.dismissHighlightError).toHaveBeenCalledOnce();
  });

  it("allows capture but blocks live-output actions while another operator owns Highlight", () => {
    server.selectedFixtures = ["fixture-a"];
    server.highlight = {
      ...offState,
      remembered: fixtures,
      can_next: true,
      owner_user_id: "operator-b",
      owner_user_name: "Focus operator",
    };
    render(<HighlightControls/>);

    expect(screen.getByRole("button", { name: "Capture current selection for Highlight" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Highlight is controlled by Focus operator" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next highlighted fixture" })).toBeDisabled();
  });
});

describe("highlightStatusLabel", () => {
  it("uses a zero-based server index only for display conversion", () => {
    expect(highlightStatusLabel({
      ...offState,
      active: true,
      mode: "step",
      output_enabled: true,
      remembered: fixtures,
      active_index: 0,
      active_fixture: fixtures[0],
      can_next: true,
    })).toBe("1/3 · Fixture 101 · Stage Left");
  });
});
