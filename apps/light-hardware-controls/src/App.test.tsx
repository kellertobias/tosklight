// @vitest-environment jsdom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import "./styles.css";
import { App as HardwareControlsApp } from "./App";
import type { OscBridge } from "./transport/oscBridge";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => undefined),
}));

const storage = {
  getItem: vi.fn().mockReturnValue(null),
  setItem: vi.fn(),
  removeItem: vi.fn(),
  clear: vi.fn(),
  key: vi.fn().mockReturnValue(null),
  length: 0,
};

beforeEach(() => {
  vi.stubGlobal("localStorage", storage);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

describe("hardware controls Programmer layout", () => {
  it("sends turn, held-turn, and click vocabulary on encoder and NAV paths", async () => {
    const send = vi.fn().mockResolvedValue(undefined);
    const bridge: OscBridge = {
      connect: vi.fn().mockResolvedValue(undefined),
      send,
      listenFeedback: vi.fn().mockResolvedValue(() => undefined),
    };
    render(<HardwareControlsApp bridge={bridge} />);

    fireEvent.click(screen.getByRole("button", { name: "Encoder 2 up" }));
    fireEvent.click(screen.getByRole("button", { name: "Encoder 2 hold" }));
    fireEvent.click(screen.getByRole("button", { name: "Encoder 2 right" }));
    fireEvent.click(screen.getByRole("button", { name: "Encoder 2 click" }));
    fireEvent.click(screen.getByRole("button", { name: "Navigation down" }));
    fireEvent.click(screen.getByRole("button", { name: "Navigation hold" }));
    fireEvent.click(screen.getByRole("button", { name: "Navigation left" }));
    fireEvent.click(screen.getByRole("button", { name: "Navigation click" }));

    expect(send.mock.calls).toEqual([
      ["encode/2", ["up", expect.any(String)]],
      ["encode/2", ["right", expect.any(String)]],
      ["encode/2", ["press", expect.any(String)]],
      ["nav", ["down", expect.any(String)]],
      ["nav", ["left", expect.any(String)]],
      ["nav", ["press", expect.any(String)]],
    ]);
  });

  it("reuses one request id across a Programmer key press and release", () => {
    HTMLElement.prototype.setPointerCapture = vi.fn();
    const send = vi.fn().mockResolvedValue(undefined);
    const bridge: OscBridge = {
      connect: vi.fn().mockResolvedValue(undefined),
      send,
      listenFeedback: vi.fn().mockResolvedValue(() => undefined),
    };
    const { container } = render(<HardwareControlsApp bridge={bridge} />);
    const key = container.querySelector(
      '[data-keypad-key="1"]',
    ) as HTMLButtonElement;

    fireEvent.pointerDown(key, { pointerId: 1 });
    fireEvent.pointerUp(key, { pointerId: 1 });

    const programmerCalls = send.mock.calls.filter(
      ([path]) => path === "programmer/digit-1",
    );
    expect(programmerCalls).toHaveLength(2);
    expect(programmerCalls[0]?.[1]).toEqual([true, expect.any(String)]);
    expect(programmerCalls[1]?.[1]).toEqual([
      false,
      programmerCalls[0]?.[1][1],
    ]);
  });

  it("uses the revised command area and keeps equal adjacent fade faders", () => {
    const { container } = render(<HardwareControlsApp />);
    const commandGrid = container.querySelector(
      ".hardware-keypad-command-section",
    );
    expect(commandGrid).not.toBeNull();

    const record = within(commandGrid as HTMLElement).getByRole("button", {
      name: "RECORD",
    });
    const preload = within(commandGrid as HTMLElement).getByRole("button", {
      name: "PRELOAD GO",
    });
    expect(record.getAttribute("data-keypad-key")).toBe("RECORD");
    expect(preload.getAttribute("data-keypad-key")).toBe("PRELOAD GO");
    expect(record.style.gridColumn).toBe("1 / span 2");
    expect(record.style.gridRow).toBe("1 / span 1");
    expect(preload.style.gridColumn).toBe("3 / span 2");
    expect(preload.style.gridRow).toBe("1 / span 1");
    for (const key of [
      "PROGRAMMER / PLAYBACK",
      "PLAYBACK",
      "OFF",
      "DIFF",
      "PAGE_UP",
      "PAGE_DOWN",
    ])
      expect(
        commandGrid?.querySelector(`[data-keypad-key="${key}"]`),
      ).not.toBeNull();
    expect(container.querySelector(".hardware-programmer-actions")).toBeNull();

    const fadeArea = container.querySelector(".fade-times");
    const programmerFade = screen.getByText("Prog Fade").closest("label");
    const cueFade = screen.getByText("Cue Fade").closest("label");
    expect(fadeArea).not.toBeNull();
    expect(Array.from(fadeArea?.children ?? [])).toEqual([
      programmerFade,
      cueFade,
    ]);
    expect(programmerFade?.classList.contains("time-fader")).toBe(true);
    expect(cueFade?.classList.contains("time-fader")).toBe(true);
    expect(programmerFade?.className).toBe(cueFade?.className);
  });

  it("exposes only the four regular Highlight keys and no dedicated status display", () => {
    const { container } = render(<HardwareControlsApp />);

    expect(
      ["HIGH", "PREV", "NEXT", "ALL"].map(
        (key) =>
          container.querySelector(`[data-keypad-key="${key}"]`)?.textContent,
      ),
    ).toEqual(["HIGH", "PREV", "NEXT", "ALL"]);
    expect(
      container.querySelector(
        ".hardware-highlight-feedback,.highlight-hardware,[aria-label='Highlight status']",
      ),
    ).toBeNull();
    expect(container.textContent).not.toMatch(
      /selection summary|output suppressed/i,
    );
  });
});
