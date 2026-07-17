import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import "../../../../hardware-controls/src/styles.css";
import { App as HardwareControlsApp } from "../../../../hardware-controls/src/App";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => undefined) }));

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
  it("uses the 2x2 command area for RECORD and PRELOAD GO and keeps equal adjacent fade faders", () => {
    const { container } = render(<HardwareControlsApp/>);
    const commandGrid = container.querySelector(".hardware-keypad-command-section");
    expect(commandGrid).toBeInTheDocument();

    const record = within(commandGrid as HTMLElement).getByRole("button", { name: "RECORD" });
    const preload = within(commandGrid as HTMLElement).getByRole("button", { name: "PRELOAD GO" });
    expect(record).toHaveAttribute("data-keypad-key", "RECORD");
    expect(preload).toHaveAttribute("data-keypad-key", "PRELOAD GO");
    expect(record).toHaveStyle({ gridColumn: "1", gridRow: "1 / span 2" });
    expect(preload).toHaveStyle({ gridColumn: "2", gridRow: "1 / span 2" });
    expect(container.querySelector(".hardware-programmer-actions")).not.toBeInTheDocument();

    const fadeArea = container.querySelector(".fade-times");
    const programmerFade = screen.getByText("Prog Fade").closest("label");
    const cueFade = screen.getByText("Cue Fade").closest("label");
    expect(fadeArea).toBeInTheDocument();
    expect([...fadeArea!.children]).toEqual([programmerFade, cueFade]);
    expect(programmerFade).toHaveClass("time-fader");
    expect(cueFade).toHaveClass("time-fader");
    expect(programmerFade!.className).toBe(cueFade!.className);
    expect(getComputedStyle(fadeArea!).gridTemplateColumns).toBe("1fr 1fr");
  });

  it("exposes only the four regular Highlight keys and no dedicated status display", () => {
    const { container } = render(<HardwareControlsApp/>);

    expect(["HIGH", "PREV", "NEXT", "ALL"].map((key) =>
      container.querySelector(`[data-keypad-key="${key}"]`)?.textContent,
    )).toEqual(["HIGH", "PREV", "NEXT", "ALL"]);
    expect(container.querySelector(".hardware-highlight-feedback,.highlight-hardware,[aria-label='Highlight status']")).not.toBeInTheDocument();
    expect(container.textContent).not.toMatch(/selection summary|output suppressed/i);
  });
});
