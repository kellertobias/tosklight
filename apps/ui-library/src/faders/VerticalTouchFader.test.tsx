import { cleanup, fireEvent, render as rtlRender, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalProvider } from "../modals/ModalStack";
import { VerticalTouchFaderSurface } from "./VerticalTouchFader";

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

  it("keeps Set value reachable while hardware disables the range gesture", () => {
    render(<VerticalTouchFaderSurface label="Enc 1 · Dimmer" value={50} hardware directInput />);
    expect(screen.getByRole("slider", { name: "Enc 1 · Dimmer" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Set value" }));
    expect(screen.getByRole("dialog", { name: "Enc 1 · Dimmer value" })).toBeInTheDocument();
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
