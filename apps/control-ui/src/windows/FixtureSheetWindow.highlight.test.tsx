import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { FixtureDefinition, HighlightState, PatchedFixture } from "../api/types";
import { FixtureSheetWindow } from "./FixtureSheetWindow";

const parameter = (attribute: string, value = 0) => ({
  attribute,
  components: [],
  default: value,
  virtual_dimmer: false,
  capabilities: [],
});

function multiHeadFixture(): PatchedFixture {
  return {
    fixture_id: "master",
    fixture_number: 100,
    name: "Pixel Bar",
    universe: 1,
    address: 1,
    definition: {
      schema_version: 1,
      id: "definition",
      revision: 1,
      manufacturer: "Test",
      device_type: "pixel fixture",
      name: "Pixel Bar",
      model: "Pixel Bar",
      mode: "2 cells",
      footprint: 4,
      heads: [
        { index: 8, name: "Base", shared: true, parameters: [parameter("tilt", 0.5)] },
        { index: 2, name: "Left", shared: false, parameters: [parameter("intensity")] },
        { index: 7, name: "Right", shared: false, parameters: [parameter("intensity")] },
      ],
      color_calibration: null,
      physical: {},
      hazardous: false,
      direct_control_protocols: [],
      signal_loss_policy: { type: "hold_last" },
      safe_values: {},
    } as FixtureDefinition,
    logical_heads: [
      { fixture_id: "right", head_index: 7 },
      { fixture_id: "left", head_index: 2 },
    ],
  };
}

const stepState = (active: boolean): HighlightState => ({
  active,
  mode: "step",
  output_enabled: active,
  capture_only: false,
  remembered: [
    { fixture_id: "left", number: 100, name: "Pixel Bar · Left" },
    { fixture_id: "right", number: 100, name: "Pixel Bar · Right" },
  ],
  active_index: 0,
  active_fixture: { fixture_id: "left", number: 100, name: "Pixel Bar · Left" },
  can_previous: true,
  can_next: true,
  owner_user_id: "operator-a",
});

const server = {
  bootstrap: { active_programmers: [] },
  session: { session_id: "session-a" },
  patch: { fixtures: [multiHeadFixture()] },
  groups: [],
  playbacks: { cue_lists: [] },
  selectedFixtures: ["left"],
  highlight: stepState(false),
  readVisualization: vi.fn().mockResolvedValue({ values: [] }),
  selectionGesture: vi.fn().mockResolvedValue(undefined),
};
const state = { preload: "idle", fixtureGroupsVisible: false };
const dispatch = vi.fn();

vi.mock("../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

beforeEach(() => {
  server.selectedFixtures = ["left"];
  server.highlight = stepState(false);
  server.readVisualization.mockClear().mockResolvedValue({ values: [] });
  server.selectionGesture.mockClear();
});

afterEach(() => cleanup());

describe("Fixture Sheet Highlight stepping visualization", () => {
  it("keeps the remembered heads subdued, marks the actual step prominently, and survives HIGH toggles", async () => {
    const { container, rerender } = render(<FixtureSheetWindow compact/>);
    await waitFor(() => expect(container.querySelector('[data-fixture-id="left"]')).toBeInTheDocument());

    const row = (fixtureId: string) => container.querySelector<HTMLElement>(`[data-fixture-id="${fixtureId}"]`);
    expect(row("left")).toHaveAttribute("data-step-selection", "active");
    expect(row("left")).toHaveClass("fixture-step-current", "fixture-step-base", "selected");
    expect(row("left")).toHaveTextContent("STEP");
    expect(row("right")).toHaveAttribute("data-step-selection", "base");
    expect(row("right")).toHaveClass("fixture-step-base");
    expect(row("right")).not.toHaveClass("selected");
    expect(row("right")).toHaveTextContent("BASE");
    expect(row("master")).toHaveAttribute("data-step-contained", "active");
    expect(row("master")).toHaveTextContent("STEP INSIDE");

    server.highlight = stepState(true);
    rerender(<FixtureSheetWindow compact/>);
    expect(row("left")).toHaveAttribute("data-step-selection", "active");
    expect(row("right")).toHaveAttribute("data-step-selection", "base");
    expect(row("master")).toHaveAttribute("data-step-contained", "active");
  });

  it("keeps contained state on a collapsed parent and removes every step marker after ALL", async () => {
    const { container, rerender } = render(<FixtureSheetWindow compact/>);
    await waitFor(() => expect(container.querySelector('[data-fixture-id="master"]')).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Collapse fixture 100.0 heads" }));
    const parent = container.querySelector<HTMLElement>('[data-fixture-id="master"]');
    expect(parent).toHaveAttribute("data-collapsed", "true");
    expect(parent).toHaveAttribute("data-step-contained", "active");
    expect(parent).toHaveTextContent("STEP INSIDE");
    expect(container.querySelector('[data-fixture-id="left"]')).not.toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="right"]')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Expand fixture 100.0 heads" }));
    server.highlight = { ...stepState(false), mode: "selection", active_index: null, active_fixture: null };
    server.selectedFixtures = ["left", "right"];
    rerender(<FixtureSheetWindow compact/>);

    const left = container.querySelector<HTMLElement>('[data-fixture-id="left"]');
    const right = container.querySelector<HTMLElement>('[data-fixture-id="right"]');
    expect(left).not.toHaveAttribute("data-step-selection");
    expect(right).not.toHaveAttribute("data-step-selection");
    expect(left).toHaveClass("selected");
    expect(right).toHaveClass("selected");
    expect(container.querySelector('[data-fixture-id="master"]')).not.toHaveAttribute("data-step-contained");
  });
});
