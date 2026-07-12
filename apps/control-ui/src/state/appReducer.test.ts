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
});
