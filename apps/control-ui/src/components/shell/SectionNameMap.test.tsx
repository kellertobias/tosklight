import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SectionNameMap } from "./SectionNameMap";

const dispatch = vi.fn();
let controlMode: "programmer" | "playbacks" = "programmer";
vi.mock("../../state/AppContext", () => ({
  useApp: () => ({ state: { showSectionNames: true, controlMode }, dispatch }),
}));

afterEach(() => { cleanup(); vi.clearAllMocks(); controlMode = "programmer"; });

describe("SectionNameMap", () => {
  it("names the programmer command regions and keeps Desk Status available", () => {
    render(<SectionNameMap/>);
    expect(screen.getByLabelText("Dock section")).toHaveTextContent("Dock");
    expect(screen.getByLabelText("View section")).toHaveTextContent("View");
    expect(screen.getByLabelText("Command section")).toHaveTextContent("Command SectionCommand LineProgrammerNum Block");
    fireEvent.click(screen.getByRole("button", { name: "Desk Status" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_MODAL", modal: "debugOpen", value: true });
  });

  it("uses the playback names in playback mode", () => {
    controlMode = "playbacks";
    render(<SectionNameMap/>);
    expect(screen.getByLabelText("Command section")).toHaveTextContent("Command SectionCommand LinePlaybackPlayback Speed Group Section");
  });
});
