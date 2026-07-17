import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HardwareControlSummary } from "./HardwareControlSummary";

const server = {
  configuration: {
    programmer_fade_millis: 3_000,
    sequence_master_fade_millis: 4_000,
    speed_groups_bpm: [120, 90, 60, 30, 15],
  },
  playbacks: { active_page: 1, pages: [] },
  highlightError: "Highlight is controlled by another operator.",
  dismissHighlightError: vi.fn(),
  setControlTiming: vi.fn(),
  setPlaybackPage: vi.fn(),
};
const state = { playbackPage: 0 };
const dispatch = vi.fn();

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

afterEach(() => {
  cleanup();
  server.highlightError = "Highlight is controlled by another operator.";
  server.dismissHighlightError.mockReset();
});

describe("hardware-connected Highlight error feedback", () => {
  it("uses the same body-level dismissible alert without inserting a Highlight status panel", () => {
    const { container } = render(<HardwareControlSummary/>);
    const alert = screen.getByRole("alert");

    expect(alert).toHaveAttribute("data-highlight-error-alert");
    expect(alert.parentElement).toBe(document.body);
    expect(container.contains(alert)).toBe(false);
    expect(container.querySelector('[aria-label="Highlight status"]')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss Highlight error" }));
    expect(server.dismissHighlightError).toHaveBeenCalledOnce();
  });
});
