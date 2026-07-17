import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { FixturePatchSetup } from "./FixturePatchSetup";
import { blankFixtureProfile } from "./fixtureProfileModel";

const state = { patchSetArmed: false };
const dispatch = vi.fn();
const server = {
  patch: { fixtures: [] as PatchedFixture[] },
  patchLayers: [] as Array<{ body: { id: string; name: string; order: number } }>,
  fixtureProfiles: [],
  fixtureLibrary: [],
  unresolvedMvrFixtures: [],
  setSelection: vi.fn(),
  updatePatchedFixture: vi.fn().mockResolvedValue(true),
  patchFixture: vi.fn(),
  savePatchLayer: vi.fn(),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

function splitFixture(): PatchedFixture {
  const profile = blankFixtureProfile();
  profile.id = "profile-split";
  profile.revision = 1;
  profile.manufacturer = "Acme";
  profile.name = "Split Wash";
  profile.short_name = "Split";
  profile.modes[0].id = "mode-split";
  profile.modes[0].splits = [{ number: 1, footprint: 4 }, { number: 3, footprint: 12 }];
  return {
    fixture_id: "fixture-split",
    fixture_number: 17,
    name: "Split Wash 17",
    definition: {
      schema_version: 2,
      id: profile.id,
      revision: 1,
      manufacturer: profile.manufacturer,
      device_type: "wash",
      name: profile.name,
      model: profile.short_name,
      mode: "Default",
      footprint: 4,
      heads: [],
      color_calibration: null,
      physical: {},
      model_asset: null,
      icon_asset: null,
      hazardous: false,
      direct_control_protocols: [],
      signal_loss_policy: { type: "hold_last" },
      safe_values: {},
      profile_id: profile.id,
      mode_id: profile.modes[0].id,
      profile_snapshot: profile,
    },
    universe: 1,
    address: 101,
    split_patches: [
      { split: 1, universe: 1, address: 101 },
      { split: 3, universe: 2, address: 201 },
    ],
    layer_id: "default",
    direct_control: null,
    location: { x: 0, y: 0, z: 0 },
    rotation: { x: 0, y: 0, z: 0 },
    logical_heads: [],
    multipatch: [],
    move_in_black_enabled: true,
    move_in_black_delay_millis: 0,
    highlight_overrides: {},
  };
}

beforeEach(() => {
  state.patchSetArmed = false;
  server.patch.fixtures = [splitFixture()];
  vi.clearAllMocks();
  server.updatePatchedFixture.mockResolvedValue(true);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("selected split SET editing", () => {
  it("uses the selected split for an armed touch, keyboard, or attached-hardware SET action", async () => {
    const { rerender } = render(<FixturePatchSetup/>);
    fireEvent.click(screen.getByRole("button", { name: "Split 3 patch 2.201" }));
    expect(server.setSelection).toHaveBeenCalledWith(["fixture-split"]);

    state.patchSetArmed = true;
    rerender(<FixturePatchSetup/>);

    expect(await screen.findByRole("heading", { name: "Set fixture split 3 address" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Split 3 address"), { target: { value: "4.401" } });
    fireEvent.click(screen.getByRole("button", { name: /^Set$/ }));

    await waitFor(() => expect(server.updatePatchedFixture).toHaveBeenCalledWith("fixture-split", {
      split_patches: [
        { split: 1, universe: 1, address: 101 },
        { split: 3, universe: 4, address: 401 },
      ],
      universe: 1,
      address: 101,
    }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PATCH_ARMED", value: false });
  });
});

describe("schema-v2 patch conflict actions", () => {
  function fixturesWithConflict() {
    const current = splitFixture();
    current.multipatch = [{
      id: "current-mp", name: "Current duplicate", universe: 6, address: 101,
      split_patches: [{ split: 1, universe: 6, address: 101 }, { split: 3, universe: 7, address: 201 }],
      location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 },
    }];
    const blocked = splitFixture();
    blocked.fixture_id = "fixture-blocked";
    blocked.fixture_number = 18;
    blocked.name = "Blocked Wash 18";
    blocked.universe = 4;
    blocked.address = 401;
    blocked.split_patches = [{ split: 1, universe: 4, address: 401 }, { split: 3, universe: 5, address: 201 }];
    blocked.multipatch = [{
      id: "blocked-mp", name: "Blocked duplicate", universe: 8, address: 301,
      split_patches: [{ split: 1, universe: 8, address: 301 }, { split: 3, universe: 9, address: 401 }],
      location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 },
    }];
    return { current, blocked };
  }

  async function requestConflictingSplitPatch() {
    fireEvent.click(screen.getByRole("button", { name: "Split 3 patch 2.201" }));
    fireEvent.change(await screen.findByLabelText("Split 3 address"), { target: { value: "4.401" } });
    fireEvent.click(screen.getByRole("button", { name: /^Set$/ }));
    expect(await screen.findByRole("heading", { name: "Patch conflict" })).toBeInTheDocument();
  }

  it("unpatches every split and multi-patch range on the current fixture", async () => {
    const { current, blocked } = fixturesWithConflict();
    server.patch.fixtures = [current, blocked];
    state.patchSetArmed = true;
    render(<FixturePatchSetup/>);
    await requestConflictingSplitPatch();

    fireEvent.click(screen.getByRole("button", { name: "Unpatch current fixture" }));
    await waitFor(() => expect(server.updatePatchedFixture).toHaveBeenCalledWith("fixture-split", {
      universe: null,
      address: null,
      split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: null, address: null }],
      multipatch: [{
        id: "current-mp", name: "Current duplicate", universe: null, address: null,
        split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: null, address: null }],
        location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 },
      }],
    }));
  });

  it("unpatches every physical range of all conflicts before applying the requested split", async () => {
    const { current, blocked } = fixturesWithConflict();
    server.patch.fixtures = [current, blocked];
    state.patchSetArmed = true;
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<FixturePatchSetup/>);
    await requestConflictingSplitPatch();

    fireEvent.click(screen.getByRole("button", { name: "Unpatch conflicts and apply" }));
    await waitFor(() => expect(server.updatePatchedFixture).toHaveBeenCalledWith("fixture-blocked", {
      universe: null,
      address: null,
      split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: null, address: null }],
      multipatch: [{
        id: "blocked-mp", name: "Blocked duplicate", universe: null, address: null,
        split_patches: [{ split: 1, universe: null, address: null }, { split: 3, universe: null, address: null }],
        location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 },
      }],
    }));
    await waitFor(() => expect(server.updatePatchedFixture).toHaveBeenCalledWith("fixture-split", {
      split_patches: [{ split: 1, universe: 1, address: 101 }, { split: 3, universe: 4, address: 401 }],
      universe: 1,
      address: 101,
    }));
    expect(confirm).toHaveBeenCalledOnce();
  });
});
