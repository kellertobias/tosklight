import { cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
  TouchValueButton,
  VerticalTouchFaderSurface,
} from "./VerticalTouchFader";

afterEach(cleanup);
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

describe("VerticalTouchFaderSurface", () => {
  it("keeps software range input absolute and clamps its visual level", () => {
    const change = vi.fn();
    const { container } = render(
      <VerticalTouchFaderSurface label="Intensity" value={125} maximum={100} hardware={false} onChange={change} />,
    );
    expect(container.querySelector(".vertical-touch-fader")).toHaveStyle({ "--fader-level": "1" });
    expect(screen.getByRole("slider", { name: "Intensity" })).not.toBeDisabled();
  });

  it("maps touch position vertically and gives both endpoints larger hit zones", () => {
    const change = vi.fn();
    render(
      <VerticalTouchFaderSurface
        label="Intensity"
        value={50}
        maximum={100}
        hardware={false}
        onChange={change}
      />,
    );
    const slider = screen.getByRole("slider", { name: "Intensity" });
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

    fireEvent.pointerDown(slider, { clientX: 10, clientY: 120, pointerId: 1 });
    fireEvent.pointerUp(slider, { clientX: 90, clientY: 120, pointerId: 1 });
    expect(change).toHaveBeenLastCalledWith(100);

    fireEvent.pointerDown(slider, { clientX: 10, clientY: 300, pointerId: 2 });
    fireEvent.pointerMove(slider, { clientX: 90, clientY: 300, pointerId: 2 });
    fireEvent.pointerUp(slider, { clientX: 90, clientY: 300, pointerId: 2 });
    expect(change).toHaveBeenLastCalledWith(50);

    fireEvent.pointerDown(slider, { clientX: 10, clientY: 480, pointerId: 3 });
    fireEvent.pointerUp(slider, { clientX: 90, clientY: 480, pointerId: 3 });
    expect(change).toHaveBeenLastCalledWith(0);
  });

  it("keeps Set value reachable while hardware disables the range gesture", () => {
    render(<VerticalTouchFaderSurface label="Enc 1 · Dimmer" value={50} hardware directInput />);
    expect(screen.getByRole("slider", { name: "Enc 1 · Dimmer" })).toBeDisabled();
    const setValue = screen.getByRole("button", { name: "Set value" });
    expect(setValue).toHaveAttribute("aria-haspopup", "dialog");
    expect(setValue).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(setValue);
    expect(setValue).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("dialog", { name: "Enc 1 · Dimmer value" })).toBeInTheDocument();
  });

  it("opens the direct value button as a modal with the fader before the keypad", () => {
    render(<TouchValueButton label="Grand Master" value={42} />);
    const trigger = screen.getByRole("button", { name: /Grand Master/ });
    expect(trigger).toHaveAttribute("aria-haspopup", "dialog");
    expect(trigger).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    const dialog = screen.getByRole("dialog", { name: "Grand Master value" });
    const body = dialog.querySelector(".modal-number-editor-content");
    expect(body?.children[0]).toContainElement(
      screen.getByRole("slider", { name: "Grand Master fader" }),
    );
    expect(body?.children[1]).toContainElement(
      screen.getByLabelText("Number input keypad"),
    );
    expect(dialog.closest(".ui-modal-stack-layer")).toHaveAttribute(
      "data-modal-top",
      "true",
    );
  });

  it("limits the action strip to three operator actions", () => {
    render(<VerticalTouchFaderSurface label="Playback 1" value={50} hardware={false} actions={[
      { id: "go", label: "GO" },
      { id: "off", label: "OFF" },
      { id: "flash", label: "FLASH" },
      { id: "extra", label: "EXTRA" },
    ]} />);
    expect(screen.getAllByRole("button")).toHaveLength(3);
    expect(screen.queryByRole("button", { name: "EXTRA" })).not.toBeInTheDocument();
  });
});
