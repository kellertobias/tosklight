import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
      icon_asset: "data:image/png;base64,fixture-icon",
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
const state = {
  preload: "idle",
  fixtureGroupsVisible: false,
  fixtureSheetOrder: "fixture-id" as const,
  fixtureSheetActiveOnly: false,
  fixtureSheetCueListId: "",
  fixtureSheetColumns: ["id", "icon", "name", "dimmer", "color", "position", "beam", "focus"] as ("id" | "icon" | "name" | "patch" | "dimmer" | "color" | "position" | "beam" | "focus")[],
  fixtureSheetShowType: true,
  fixtureSheetIncludedHeads: "all" as "all" | "no-sub-heads" | "no-master-heads",
};
const dispatch = vi.fn();

vi.mock("../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../features/server/useShowObjectsState", () => ({ useGroups: () => server.groups }));
vi.mock("../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

beforeEach(() => {
  server.patch.fixtures = [multiHeadFixture()];
  server.selectedFixtures = ["left"];
  server.highlight = stepState(false);
  server.readVisualization.mockClear().mockResolvedValue({ values: [] });
  server.selectionGesture.mockClear();
  state.fixtureSheetColumns = ["id", "icon", "name", "dimmer", "color", "position", "beam", "focus"];
  state.fixtureSheetShowType = true;
  state.fixtureSheetIncludedHeads = "all";
  dispatch.mockClear();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("Fixture Sheet Highlight stepping visualization", () => {
  it("does not poll visualization while its pane is covered", async () => {
    vi.useFakeTimers();
    render(<FixtureSheetWindow active={false} compact/>);

    await act(() => vi.advanceTimersByTimeAsync(1_000));

    expect(server.readVisualization).not.toHaveBeenCalled();
  });

  it("keeps every fixture row available in a compact scrollable pane", async () => {
    server.patch.fixtures = Array.from({ length: 13 }, (_, index) => {
      const fixture = multiHeadFixture();
      return {
        ...fixture,
        fixture_id: `fixture-${index + 1}`,
        fixture_number: index + 1,
        name: `Fixture ${index + 1}`,
        logical_heads: [],
        definition: { ...fixture.definition, heads: [fixture.definition.heads[0]] },
      };
    });
    const { container } = render(<FixtureSheetWindow compact/>);
    await waitFor(() => expect(container.querySelector('[data-fixture-id="fixture-13"]')).toBeInTheDocument());
    expect(container.querySelectorAll(".ui-data-table-row:not(.header)")).toHaveLength(13);
  });

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

  it("uses settings instead of row buttons to show masters, subheads, or both", async () => {
    const { container, rerender } = render(<FixtureSheetWindow/>);
    await waitFor(() => expect(container.querySelector('[data-fixture-id="master"]')).toBeInTheDocument());

    expect(screen.queryByRole("button", { name: /fixture 100\.0 heads/i })).not.toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="left"]')).toHaveClass("fixture-head-row");
    expect(container.querySelector('[data-fixture-id="left"] .fixture-sheet-id')).toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="left"] .fixture-name')).toBeInTheDocument();

    state.fixtureSheetIncludedHeads = "no-sub-heads";
    rerender(<FixtureSheetWindow/>);
    expect(container.querySelector('[data-fixture-id="master"]')).toHaveTextContent(/^100/);
    expect(container.querySelector('[data-fixture-id="master"]')).not.toHaveTextContent("100.0");
    expect(container.querySelector('[data-fixture-id="left"]')).not.toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="right"]')).not.toBeInTheDocument();

    state.fixtureSheetIncludedHeads = "no-master-heads";
    rerender(<FixtureSheetWindow/>);
    expect(container.querySelector('[data-fixture-id="master"]')).not.toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="left"]')).toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="right"]')).toBeInTheDocument();
    expect(container.querySelector('[data-fixture-id="left"]')).not.toHaveClass("fixture-head-indented-row");
    expect(container.querySelector('[data-fixture-id="right"]')).not.toHaveClass("fixture-head-indented-row");

    state.fixtureSheetIncludedHeads = "all";
    server.highlight = { ...stepState(false), mode: "selection", active_index: null, active_fixture: null };
    server.selectedFixtures = ["left", "right"];
    rerender(<FixtureSheetWindow/>);

    const left = container.querySelector<HTMLElement>('[data-fixture-id="left"]');
    const right = container.querySelector<HTMLElement>('[data-fixture-id="right"]');
    expect(left).not.toHaveAttribute("data-step-selection");
    expect(right).not.toHaveAttribute("data-step-selection");
    expect(left).toHaveClass("selected");
    expect(right).toHaveClass("selected");
    expect(container.querySelector('[data-fixture-id="master"]')).not.toHaveAttribute("data-step-contained");
  });

  it("uses a compact View tab, exposes column controls, and hides optional name details", async () => {
    state.fixtureSheetColumns = ["id", "name", "dimmer"];
    state.fixtureSheetShowType = false;
    render(<FixtureSheetWindow/>);

    expect(screen.getByText("Name", { selector: ".ui-data-table-row.header span" })).toBeInTheDocument();
    expect(screen.queryByText("Beam", { selector: ".ui-data-table-row.header span" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Test · 2 cells/)).not.toBeInTheDocument();
    expect(screen.queryByText(/U1\.1/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const settings = screen.getByRole("dialog", { name: "Fixture Sheet" });
    expect(settings).toBeVisible();
    expect(screen.getByRole("tab", { name: "View" })).toBeVisible();
    expect(screen.queryByRole("tab", { name: "Ordering" })).not.toBeInTheDocument();
    expect(screen.queryByRole("tab", { name: "Filters" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Ordering" })).toBeVisible();
    expect(screen.getByRole("heading", { name: "Filters" })).toBeVisible();
    const includedHeads = screen.getByRole("button", { name: /^All$/ });
    fireEvent.click(includedHeads);
    fireEvent.click(screen.getByRole("option", { name: "No master heads" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_FIXTURE_SHEET_OPTIONS", includedHeads: "no-master-heads" });

    fireEvent.click(screen.getByRole("tab", { name: "Columns" }));
    expect(screen.getByRole("switch", { name: "Fixture ID" })).toBeChecked();
    expect(screen.getByRole("switch", { name: "Icon" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "Patch address" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "Beam" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "Show fixture type" })).not.toBeChecked();
    fireEvent.click(screen.getByRole("switch", { name: "Patch address" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_FIXTURE_SHEET_OPTIONS", columns: ["id", "name", "dimmer", "patch"] });
  });

  it("keeps fixture type aligned with the name and renders icon and patch as dedicated columns", async () => {
    state.fixtureSheetColumns = ["id", "icon", "name", "patch"];
    render(<FixtureSheetWindow/>);

    const master = await screen.findByRole("row", { name: /100\.0.*Pixel Bar · Master.*Test · 2 cells.*U1\.1/ });
    const name = master.querySelector<HTMLElement>(".fixture-name");
    expect(name).toHaveTextContent("Pixel Bar · MasterTest · 2 cells");
    expect(name).not.toHaveTextContent("U1.1");
    expect(name?.querySelector(".fixture-type")).toBeInTheDocument();
    expect(master.querySelector('.fixture-sheet-icon img[src="data:image/png;base64,fixture-icon"]')).toBeInTheDocument();
    expect(master.querySelector(".fixture-sheet-patch")).toHaveTextContent("U1.1");

    const headers = screen.getAllByRole("columnheader").map((header) => header.textContent);
    expect(headers).toEqual(["ID", "Icon", "Name / type", "Patch"]);
  });
});
