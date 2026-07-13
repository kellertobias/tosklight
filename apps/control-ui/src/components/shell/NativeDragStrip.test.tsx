import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { NativeDragStrip } from "./NativeDragStrip";

afterEach(cleanup);

describe("native window controls", () => {
  it("provides close, fullscreen, and drag controls in web and native shells", () => {
    render(<NativeDragStrip />);
    expect(screen.getByRole("button", { name: "Close window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Enter fullscreen" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByRole("button", { name: "Move window" })).toHaveAttribute("data-tauri-drag-region");
  });
});
