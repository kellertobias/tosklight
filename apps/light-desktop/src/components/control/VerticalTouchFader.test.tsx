import { cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import { ModalProvider } from "@tosklight/ui/modals";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TouchValueButton } from "@tosklight/ui/faders";
import { VerticalTouchFader } from "./VerticalTouchFader";

const render = (ui: Parameters<typeof rtlRender>[0]) => rtlRender(ui, { wrapper: ModalProvider });

let hardwareConnected = false;
vi.mock("../../features/deskSnapshot/DeskSnapshotState", () => ({ useHardwareConnected: () => hardwareConnected }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { midiProfile: null } }) }));

afterEach(() => {
  cleanup();
  hardwareConnected = false;
});

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
    expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass("action-count-3");
    expect(screen.queryByRole("button", { name: "EXTRA" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "GO" }));
    expect(action).toHaveBeenCalledOnce();
  });

  it("preserves displays whose leading number is not the raw fader position", () => {
    render(<VerticalTouchFader label="Speed Group A" value={100} display="120 BPM · MANUAL"/>);
    expect(screen.getByText("120 BPM · MANUAL")).toBeInTheDocument();
  });

  it("keeps direct value entry reachable while hardware disables the range gesture", () => {
    hardwareConnected = true;
    render(<VerticalTouchFader label="Enc 1 · Dimmer" value={50} directInput/>);
    expect(screen.getByRole("slider", { name: "Enc 1 · Dimmer" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Set value" }));
    expect(screen.getByRole("dialog", { name: "Enc 1 · Dimmer value" })).toBeInTheDocument();
  });

  it("opens the compact set-value control with both a touch fader and number pad", () => {
    const onChange = vi.fn();
    render(<TouchValueButton label="Prog. Fade" value={3} maximum={20} display="3.0 s" onChange={onChange}/>);
    fireEvent.click(screen.getByRole("button", { name: /Prog\. Fade/ }));
    const dialog = screen.getByRole("dialog", { name: "Prog. Fade value" });
    expect(dialog.querySelector('input[type="range"]')).toBeInTheDocument();
    expect(screen.getByLabelText("Number input keypad")).toBeInTheDocument();
    const slider = screen.getByRole("slider", { name: "Prog. Fade fader" });
    vi.spyOn(slider, "getBoundingClientRect").mockReturnValue({
      bottom: 500,
      height: 400,
      left: 0,
      right: 100,
      top: 100,
      width: 100,
      x: 0,
      y: 100,
      toJSON: () => undefined,
    });
    fireEvent.pointerDown(slider, { clientY: 300, pointerId: 1 });
    fireEvent.pointerUp(slider, { clientY: 300, pointerId: 1 });
    expect(onChange).toHaveBeenCalledWith(10);
  });
});
