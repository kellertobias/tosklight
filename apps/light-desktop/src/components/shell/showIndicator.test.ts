import { describe, expect, it } from "vitest";
import { getShowIndicator } from "./showIndicator";

describe("getShowIndicator", () => {
  it("shows a healthy saved state when connected", () => {
    expect(getShowIndicator("connected")).toEqual(expect.objectContaining({
      className: "show-status-connected",
      label: "Show active",
    }));
  });

  it("explains that connected show changes are saved automatically", () => {
    expect(getShowIndicator("connected").detail).toContain("saved automatically");
  });

  it("shows a disconnected server", () => {
    expect(getShowIndicator("offline")).toEqual(expect.objectContaining({
      className: "show-status-disconnected",
      label: "Server disconnected",
    }));
  });
});
