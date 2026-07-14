import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { VerticalTouchFader } from "./VerticalTouchFader";

vi.mock("../../api/ServerContext", () => ({ useServer: () => ({ bootstrap: null }) }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { midiProfile: null } }) }));

afterEach(cleanup);

describe("VerticalTouchFader", () => {
  it("renders up to three optional action buttons below the shared fader", () => {
    const action = vi.fn();
    const { container } = render(<VerticalTouchFader label="Playback 1" value={50} actions={[
      { id: "go", label: "GO", onClick: action },
      { id: "off", label: "OFF" },
      { id: "flash", label: "FLASH" },
      { id: "extra", label: "EXTRA" },
    ]}/>);
    expect(container.querySelectorAll(".vertical-touch-fader-actions .ui-button")).toHaveLength(3);
    expect(screen.queryByRole("button", { name: "EXTRA" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "GO" }));
    expect(action).toHaveBeenCalledOnce();
  });
});
