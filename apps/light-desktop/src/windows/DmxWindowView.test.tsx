import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  dmxOutputHealth,
  dmxPatchedFixtures,
  dmxSnapshot,
  dmxSnapshotWithoutOverrides,
} from "../../../ui-library/storybook/fixtures/dmx";
import { DmxWindowView } from "./DmxWindow";

beforeEach(() => {
  vi.stubGlobal("ResizeObserver", class {
    observe() {}
    disconnect() {}
  });
});
afterEach(cleanup);

function renderView(overrides: Partial<Parameters<typeof DmxWindowView>[0]> = {}) {
  const onSetDmxOverride = vi.fn();
  const onDotSizeChange = vi.fn();
  const rendered = render(<DmxWindowView
    dotSize="small"
    onDotSizeChange={onDotSizeChange}
    onSetDmxOverride={onSetDmxOverride}
    outputHealth={dmxOutputHealth}
    outputRoutes={[]}
    patchedFixtures={dmxPatchedFixtures}
    snapshot={dmxSnapshot}
    {...overrides}
  />);
  return { ...rendered, onDotSizeChange, onSetDmxOverride };
}

describe("DMX application view", () => {
  it("labels the two output modes Values and Sources", () => {
    renderView();

    expect(screen.getByRole("button", { name: "Values" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sources" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Values as dots" })).toBeNull();
  });

  it("renders all 512 selectable addresses for every visible universe and the output summary", () => {
    const { container } = renderView();
    expect(container.querySelectorAll(".dmx-universe")).toHaveLength(4);
    expect(container.querySelectorAll(".dmx-universe button")).toHaveLength(2_048);
    expect(screen.getByText("44.0 Hz")).toBeInTheDocument();
    expect(screen.getByText("170824")).toBeInTheDocument();
    expect(container.querySelector(".ui-data-table")).not.toBeInTheDocument();
    expect(container.querySelector(".ui-selection-tree")).not.toBeInTheDocument();
  });

  it("keeps a selected patched address stable and presents its complete inspector", () => {
    const { container, rerender, onDotSizeChange, onSetDmxOverride } = renderView({
      defaultSelection: { universe: 1, address: 13 },
    });
    const selected = screen.getByRole("button", { name: "Universe 1, address 13, value 224" });
    expect(selected).toHaveClass("selected", "high");
    expect(screen.getByText("Universe 1 · Channel 13")).toBeInTheDocument();
    expect(screen.getByText("DMX address 13 · 0x00D")).toBeInTheDocument();
    const dips = screen.getByLabelText("DIP switches for DMX address 13");
    expect(dips.children).toHaveLength(9);
    expect(dips.querySelectorAll(".on")).toHaveLength(3);
    const fixture = container.querySelector(".dmx-fixture-card") as HTMLElement;
    expect(fixture).toHaveTextContent("99");
    expect(fixture).toHaveTextContent("Stage Hazer");
    expect(fixture).toHaveTextContent("hazer");
    expect(fixture).toHaveTextContent("Fixture patch");
    expect(fixture).toHaveTextContent("1.13–14");
    expect(fixture).toHaveTextContent("1 of 2");
    expect(fixture).toHaveTextContent("intensity");
    expect(screen.getByRole("button", { name: /Raw value/u })).toBeInTheDocument();

    const changed = {
      ...dmxSnapshot,
      universes: dmxSnapshot.universes.map((frame) => frame.universe === 1
        ? { ...frame, slots: frame.slots.map((value, index) => index === 12 ? 91 : value) }
        : frame),
    };
    rerender(<DmxWindowView
      defaultSelection={{ universe: 1, address: 13 }}
      dotSize="small"
      onDotSizeChange={onDotSizeChange}
      onSetDmxOverride={onSetDmxOverride}
      outputHealth={dmxOutputHealth}
      outputRoutes={[]}
      patchedFixtures={dmxPatchedFixtures}
      snapshot={changed}
    />);
    expect(screen.getByRole("button", { name: "Universe 1, address 13, value 91" })).toHaveClass("selected");

    fireEvent.click(screen.getByRole("button", { name: "Release override" }));
    expect(onSetDmxOverride).toHaveBeenCalledWith(1, 13, null);
  }, 15_000);

  it("renders active and empty source states and releases the selected source", () => {
    const { rerender, onSetDmxOverride, onDotSizeChange } = renderView({ defaultView: "sources" });
    const source = screen.getByText("Universe 1 · Address 13").closest("article") as HTMLElement;
    fireEvent.click(within(source).getByRole("button", { name: "Release" }));
    expect(onSetDmxOverride).toHaveBeenCalledWith(1, 13, null);

    rerender(<DmxWindowView
      defaultView="sources"
      dotSize="small"
      onDotSizeChange={onDotSizeChange}
      onSetDmxOverride={onSetDmxOverride}
      outputHealth={dmxOutputHealth}
      outputRoutes={[]}
      patchedFixtures={dmxPatchedFixtures}
      snapshot={dmxSnapshotWithoutOverrides}
    />);
    expect(screen.getByText("No raw DMX overrides are active.")).toBeInTheDocument();
  });

  it("exposes Small and Large dot settings through the production settings surface", () => {
    const { onDotSizeChange } = renderView();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const settings = screen.getByRole("dialog", { name: "DMX Settings" });
    fireEvent.click(within(settings).getByRole("button", { name: "Large" }));
    expect(onDotSizeChange).toHaveBeenCalledWith("large");
  });
});
