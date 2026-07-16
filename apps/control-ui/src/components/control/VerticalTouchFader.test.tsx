import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TouchValueButton, VerticalTouchFader } from "./VerticalTouchFader";

vi.mock("../../api/ServerContext", () => ({ useServer: () => ({ bootstrap: null }) }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { midiProfile: null } }) }));

afterEach(cleanup);

describe("VerticalTouchFader", () => {
  it("never lets the mouse wheel control the fader", () => {
    const onChange = vi.fn();
    render(<VerticalTouchFader label="Intensity" value={50} onChange={onChange}/>);
    const fader = screen.getByRole("slider", { name: "Intensity" });
    fader.focus();
    const wheel = new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaY: -10 });
    fader.dispatchEvent(wheel);
    expect(wheel.defaultPrevented).toBe(true);
    expect(fader).not.toHaveFocus();
    expect(onChange).not.toHaveBeenCalled();
  });

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

  it("opens the compact set-value control with both a touch fader and number pad", () => {
    const onChange = vi.fn();
    render(<TouchValueButton label="Prog. Fade" value={3} maximum={20} display="3.0 s" onChange={onChange}/>);
    fireEvent.click(screen.getByRole("button", { name: /Prog\. Fade/ }));
    const dialog = screen.getByRole("dialog", { name: "Prog. Fade value" });
    expect(dialog.querySelector('input[type="range"]')).toBeInTheDocument();
    expect(screen.getByLabelText("Number input keypad")).toBeInTheDocument();
    const slider = screen.getByRole("slider", { name: "Prog. Fade" });
    fireEvent.pointerDown(slider);
    fireEvent.input(slider, { target: { value: "4.5" } });
    fireEvent.pointerUp(slider);
    expect(onChange).toHaveBeenCalledWith(4.5);
  });
});
