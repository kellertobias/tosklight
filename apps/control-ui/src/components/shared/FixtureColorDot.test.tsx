import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { FixtureColorDot } from "./FixtureColorDot";

describe("FixtureColorDot", () => {
  it.each([
    ["dark", "rgb(0, 8, 24)"],
    ["bright", "rgb(255, 255, 255)"],
    ["absent", "transparent"],
    ["mixed", "linear-gradient(90deg, rgb(255, 0, 0) 50%, rgb(0, 0, 255) 50%)"],
  ])("keeps the authoritative %s fill behind the shared boundary", (_state, color) => {
    const { container } = render(<FixtureColorDot color={color} />);
    const dot = container.querySelector<HTMLElement>(".color-dot");

    expect(dot).toBeInTheDocument();
    expect(dot).toHaveAttribute("aria-hidden", "true");
    expect(dot?.style.background).toBe(color);
    expect(dot?.style.border).toBe("1px solid rgb(165, 175, 182)");
  });
});
