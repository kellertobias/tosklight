import { act, cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import { createRef, type ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import {
  HardwareEncoderDisplayView,
  type HardwareEncoderDisplayHandle,
} from "./HardwareEncoderDisplay";

afterEach(cleanup);
const render = (ui: ReactElement) => rtlRender(ui, { wrapper: ModalProvider });

describe("HardwareEncoderDisplayView", () => {
  it("renders unassigned, single-target, and dual-target states", () => {
    const { rerender } = render(<HardwareEncoderDisplayView slot={1} />);
    expect(screen.getByLabelText("Encoder 1 unassigned")).toBeInTheDocument();
    rerender(<HardwareEncoderDisplayView slot={1} target={{ label: "Dimmer", value: "52%" }} />);
    expect(screen.getByLabelText("Encoder 1: Dimmer, 52%")).toHaveClass("single-target");
    rerender(<HardwareEncoderDisplayView slot={1} target={{ label: "Pan", value: "20°" }} secondary={{ label: "Tilt", value: "30°" }} />);
    expect(screen.getByLabelText("Encoder 1: Pan, 20°")).toHaveClass("dual-target");
  });

  it("provides a generic imperative activation path for application hardware adapters", () => {
    const ref = createRef<HardwareEncoderDisplayHandle>();
    const edit = vi.fn();
    render(<HardwareEncoderDisplayView ref={ref} slot={2} target={{ label: "Zoom", value: "42%" }} editValue={42} onEdit={edit} />);
    act(() => ref.current?.activate());
    expect(screen.getByRole("dialog", { name: "Encoder 2 value" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "5" }));
    fireEvent.click(screen.getByRole("button", { name: "ENTER" }));
    expect(edit).toHaveBeenCalledWith(5);
  });

  it("offers Release only with ownership and a callback, then closes without editing", () => {
    const ref = createRef<HardwareEncoderDisplayHandle>();
    const edit = vi.fn();
    const release = vi.fn();
    const rendered = render(<HardwareEncoderDisplayView ref={ref} slot={1} target={{ label: "Dimmer", value: "52%" }} editValue={52} canRelease onEdit={edit} />);
    act(() => ref.current?.activate());
    expect(screen.queryByRole("button", { name: "Release Dimmer" })).not.toBeInTheDocument();

    rendered.rerender(<HardwareEncoderDisplayView ref={ref} slot={1} target={{ label: "Dimmer", value: "52%" }} editValue={52} canRelease onEdit={edit} onRelease={release} />);
    fireEvent.click(screen.getByRole("button", { name: "Release Dimmer" }));
    expect(release).toHaveBeenCalledOnce();
    expect(edit).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
