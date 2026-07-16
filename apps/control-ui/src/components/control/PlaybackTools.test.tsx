import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PlaybackTools } from "./PlaybackTools";

const dispatch = vi.fn();
const state = {
  playbackPage: 0,
  playbackPageNames: ["Main", "Effects"],
};
const server = {
  configuration: {
    programmer_fade_millis: 3_000,
    sequence_master_fade_millis: 4_000,
    speed_groups_bpm: [120, 90, 60, 30, 15],
  },
  playbacks: { active_page: 1, pages: [{ number: 1, name: "Main" }] },
  setControlTiming: vi.fn(),
  setPlaybackPage: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("PlaybackTools", () => {
  it("orders page controls, fade masters, and speed groups with icon-only chevrons", () => {
    const { container } = render(<PlaybackTools/>);
    const tools = container.querySelector(".playback-tools")!;
    expect([...tools.children].map((child) => child.className)).toEqual([
      "playback-page-controls",
      "programmer-fade-fader full",
      "cue-fade-master",
      "speed-group-stack",
    ]);
    const previous = screen.getByRole("button", { name: "Previous playback page" });
    const next = screen.getByRole("button", { name: "Next playback page" });
    expect(previous.textContent).toBe("");
    expect(next.textContent).toBe("");
    expect(previous.querySelector("svg path")).toBeInTheDocument();
    expect(next.querySelector("svg path")).toBeInTheDocument();
    const current = screen.getByRole("button", { name: "Select playback page. Page 1 Main" });
    expect(within(current).getByText("Page")).toBeInTheDocument();
    expect(within(current).getByText("1")).toBeInTheDocument();
    expect(within(current).getByText("Main")).toBeInTheDocument();
    const speedGroupA = within(container.querySelector(".speed-group-stack")!).getByRole("button", { name: "Speed group A, 120 BPM" });
    expect([...speedGroupA.children].map((child) => child.className)).toEqual([
      "speed-group-label",
      "speed-group-value",
      "speed-group-unit",
    ]);
  });
});
