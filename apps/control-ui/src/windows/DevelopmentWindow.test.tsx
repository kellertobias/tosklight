import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DevelopmentWindow } from "./DevelopmentWindow";

vi.mock("../api/ServerContext", () => ({ useServer: () => ({ bootstrap: null }) }));
vi.mock("../state/AppContext", () => ({ useApp: () => ({ state: { midiProfile: null } }) }));

afterEach(cleanup);

describe("DevelopmentWindow", () => {
  it("shows the unified form-element catalog", () => {
    render(<DevelopmentWindow compact developmentView="forms"/>);
    expect(screen.getByRole("heading", { name: "Side labels" })).toBeInTheDocument();
    for (const count of [2, 3, 4, 5, 6]) expect(screen.getByRole("radiogroup", { name: `${count} values` })).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Toggle" })).toBeInTheDocument();
    expect(screen.getByRole("slider", { name: "Horizontal fader" })).toBeInTheDocument();
  });

  it("shows vertical faders with zero through three actions", () => {
    const { container } = render(<DevelopmentWindow compact developmentView="faders"/>);
    expect(container.querySelectorAll(".vertical-touch-fader")).toHaveLength(4);
    expect(screen.getByRole("button", { name: "FLASH" })).toBeInTheDocument();
  });
});
