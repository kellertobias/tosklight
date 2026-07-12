import { describe, expect, it } from "vitest";
import { appReducer, initialState } from "./appReducer";

describe("appReducer", () => {
  it("keeps programmer and playback as explicit switchable control modes", () => {
    const playback = appReducer(initialState, { type: "TOGGLE_CONTROL_MODE" });
    expect(playback.controlMode).toBe("playbacks");
    expect(appReducer(playback, { type: "TOGGLE_CONTROL_MODE" }).controlMode).toBe("programmer");
  });

  it("cycles preload from blind to output and supports release", () => {
    const blind = appReducer(initialState, { type: "ADVANCE_PRELOAD" });
    expect(blind.preload).toBe("blind");
    const output = appReducer(blind, { type: "ADVANCE_PRELOAD" });
    expect(output.preload).toBe("output");
    expect(appReducer(output, { type: "RELEASE_PRELOAD" }).preload).toBe("idle");
  });

  it("keeps resized panes inside the 24 by 18 grid", () => {
    const changed = appReducer(initialState, {
      type: "SET_PANE_RECT",
      id: initialState.desks[0].panes[0].id,
      rect: { x: 23, y: 17, width: 12, height: 12 },
    });
    expect(changed.desks[0].panes[0]).toMatchObject({ x: 23, y: 17, width: 2, height: 2 });
  });

  it("creates an empty new desk normally and clones when saving as new", () => {
    const empty = appReducer(initialState, { type: "NEW_DESK" });
    expect(empty.desks.at(-1)?.panes).toHaveLength(0);

    const saving = appReducer(initialState, { type: "START_SAVE_DESK" });
    const cloned = appReducer(saving, { type: "NEW_DESK" });
    expect(cloned.desks.at(-1)?.panes).toHaveLength(initialState.desks[0].panes.length);
    expect(cloned.desks.at(-1)?.panes[0].id).not.toBe(initialState.desks[0].panes[0].id);
  });

  it("copies the active desk into an existing save target and hydrates stored layouts", () => {
    const saving = appReducer(initialState, { type: "START_SAVE_DESK" });
    const saved = appReducer(saving, { type: "SAVE_DESK_TO", id: "playback" });
    expect(saved.activeDeskId).toBe("playback");
    expect(saved.desks.find((desk) => desk.id === "playback")?.panes).toHaveLength(initialState.desks[0].panes.length);

    const hydrated = appReducer(saved, {
      type: "HYDRATE_LAYOUT",
      desks: [{ id: "tour", name: "Tour", panes: [] }],
      activeDeskId: "tour",
    });
    expect(hydrated.desks).toEqual([{ id: "tour", name: "Tour", panes: [] }]);
    expect(hydrated.activeDeskId).toBe("tour");
  });

  it("does not override operator navigation when layout hydration finishes late", () => {
    const navigating = appReducer(initialState, { type: "OPEN_BUILTIN", kind: "groups" });
    const hydrated = appReducer(navigating, {
      type: "HYDRATE_LAYOUT",
      desks: [{ id: "tour", name: "Tour", panes: [] }],
      activeDeskId: "tour",
    });
    expect(hydrated.builtIn).toBe("groups");
    expect(hydrated.dockMode).toBe("builtins");
  });

  it("restores the last desk and built-in when switching dock sections", () => {
    const patch = appReducer(initialState, { type: "OPEN_DESK", id: "patch" });
    const groups = appReducer(patch, { type: "OPEN_BUILTIN", kind: "groups" });
    const desks = appReducer(groups, { type: "SET_DOCK_MODE", mode: "desks" });
    expect(desks.activeDeskId).toBe("patch");
    expect(desks.builtIn).toBeNull();
    const builtIns = appReducer(desks, { type: "SET_DOCK_MODE", mode: "builtins" });
    expect(builtIns.builtIn).toBe("groups");
  });

  it("configures playback rows and columns within desk limits", () => {
    const configured = appReducer(initialState, { type: "SET_PLAYBACK_LAYOUT", columns: 20, rows: 3 });
    expect(configured.playbackColumns).toBe(20);
    expect(configured.playbackRows).toBe(3);
    const clamped = appReducer(configured, { type: "SET_PLAYBACK_LAYOUT", columns: 99, rows: 8 });
    expect(clamped.playbackColumns).toBe(32);
    expect(clamped.playbackRows).toBe(3);
  });

  it("moves between playback executor pages and clamps at the ends", () => {
    expect(appReducer(initialState, { type: "SET_PLAYBACK_PAGE", page: 3 }).playbackPage).toBe(3);
    expect(appReducer(initialState, { type: "SET_PLAYBACK_PAGE", page: 999 }).playbackPage).toBe(126);
    expect(appReducer(initialState, { type: "SET_PLAYBACK_PAGE", page: -1 }).playbackPage).toBe(0);
  });

  it("keeps the preset family while navigating between built-ins", () => {
    const intensity = appReducer(initialState, { type: "SET_PRESET_FAMILY", family: "Intensity" });
    const groups = appReducer(intensity, { type: "OPEN_BUILTIN", kind: "groups" });
    const presets = appReducer(groups, { type: "OPEN_BUILTIN", kind: "presets" });
    expect(presets.presetFamily).toBe("Intensity");
  });

  it("arms and cancels the shared store workflow explicitly", () => {
    const armed = appReducer(initialState, { type: "SET_STORE_ARMED", value: true });
    expect(armed.storeArmed).toBe(true);
    expect(appReducer(armed, { type: "SET_STORE_ARMED", value: false }).storeArmed).toBe(false);
  });
});
