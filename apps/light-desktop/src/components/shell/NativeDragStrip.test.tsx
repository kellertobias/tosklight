import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { browserDesktopBridge } from "../../platform/desktop/browserDesktopBridge";
import { DesktopProvider } from "../../platform/desktop/DesktopContext";
import type { DesktopBridge } from "../../platform/desktop/types";
import { NativeDragStrip } from "./NativeDragStrip";

afterEach(cleanup);

function nativeShell(overrides: Partial<DesktopBridge>): DesktopBridge {
  return { ...browserDesktopBridge, available: true, ...overrides };
}

describe("native window controls", () => {
  it("provides close, fullscreen, and drag controls in web and native shells", () => {
    render(<NativeDragStrip />);
    expect(screen.getByRole("button", { name: "Close window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Enter fullscreen" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "Move window" })).toHaveAttribute("data-tauri-drag-region");
  });

  it("closes only its own window on a screen", () => {
    const closeCurrentWindow = vi.fn().mockResolvedValue(undefined);
    const exitApplication = vi.fn().mockResolvedValue(undefined);
    render(
      <DesktopProvider bridge={nativeShell({ closeCurrentWindow, exitApplication })}>
        <NativeDragStrip />
      </DesktopProvider>,
    );
    screen.getByRole("button", { name: "Close window" }).click();
    expect(closeCurrentWindow).toHaveBeenCalledOnce();
    expect(exitApplication).not.toHaveBeenCalled();
  });

  it("quits the desk from the main window", () => {
    const closeCurrentWindow = vi.fn().mockResolvedValue(undefined);
    const exitApplication = vi.fn().mockResolvedValue(undefined);
    render(
      <DesktopProvider bridge={nativeShell({ closeCurrentWindow, exitApplication })}>
        <NativeDragStrip closes="application" />
      </DesktopProvider>,
    );
    screen.getByRole("button", { name: "Quit ToskLight" }).click();
    expect(exitApplication).toHaveBeenCalledOnce();
    expect(closeCurrentWindow).not.toHaveBeenCalled();
  });
});
