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
    const isolated = { ...initialState, desks: initialState.desks.map((desk, index) => index ? desk : { ...desk, panes: desk.panes.slice(0, 1) }) };
    const changed = appReducer(isolated, {
      type: "SET_PANE_RECT",
      id: isolated.desks[0].panes[0].id,
      rect: { x: 23, y: 17, width: 12, height: 12 },
    });
    expect(changed.desks[0].panes[0]).toMatchObject({ x: 23, y: 17, width: 2, height: 2 });
  });

  it("rejects pane moves and resizes that overlap another pane", () => {
    const pane = initialState.desks[0].panes[0];
    const blocker = initialState.desks[0].panes[1];
    const changed = appReducer(initialState, { type: "SET_PANE_RECT", id: pane.id, rect: { x: blocker.x, y: blocker.y, width: blocker.width, height: blocker.height } });
    expect(changed.desks[0].panes[0]).toEqual(pane);
  });

  it("creates an empty new desk normally and clones when saving as new", () => {
    const empty = appReducer(initialState, { type: "NEW_DESK" });
    expect(empty.desks.at(-1)?.panes).toHaveLength(0);
    expect(empty.desks.at(-1)?.name).toBe("Desktop 4");

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

  it("keeps Update and Record mutually exclusive", () => {
    const updating = appReducer(initialState, { type: "SET_UPDATE_ARMED", value: true });
    expect(updating.updateArmed).toBe(true);
    expect(updating.storeArmed).toBe(false);
    const recording = appReducer(updating, { type: "SET_STORE_ARMED", value: true });
    expect(recording.storeArmed).toBe(true);
    expect(recording.updateArmed).toBe(false);
  });

  it("updates stage presentation options and clamps environment brightness", () => {
    const hidden = appReducer(initialState, { type: "SET_STAGE_OPTIONS", groupsVisible: false, showSelection: false, environmentBrightness: 3 });
    expect(hidden.stageGroupsVisible).toBe(false);
    expect(hidden.stageShowSelection).toBe(false);
    expect(hidden.stageEnvironmentBrightness).toBe(2);
    expect(appReducer(hidden, { type: "SET_STAGE_OPTIONS", environmentBrightness: -1 }).stageEnvironmentBrightness).toBe(0);
  });

  it("persists the selected Development catalog on its pane", () => {
    const desks = [{ id: "test", name: "Test", panes: [{ id: "development", kind: "development" as const, title: "Development", x: 1, y: 1, width: 8, height: 8 }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "test" });
    const updated = appReducer(hydrated, { type: "SET_PANE_DEVELOPMENT_VIEW", id: "development", value: "faders" });
    expect(updated.desks[0].panes[0].developmentView).toBe("faders");
  });

  it("persists only non-authoritative Text Editor view state in the pane layout", () => {
    const desks = [{ id: "notes", name: "Notes", panes: [{ id: "editor", kind: "text_editor" as const, title: "Text Editor", x: 1, y: 1, width: 8, height: 8, textFileRoot: "shows", textFilePath: "run.md" }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "notes" });
    const updated = appReducer(hydrated, { type: "SET_TEXT_EDITOR_VIEW", id: "editor", root: "shows", path: "run.md", selectionStart: 12, selectionEnd: 16, scrollTop: 240 });
    expect(updated.desks[0].panes[0].textEditorView).toEqual({ root: "shows", path: "run.md", selectionStart: 12, selectionEnd: 16, scrollTop: 240 });
    expect(updated.desks[0].panes[0]).not.toHaveProperty("text");
  });

  it("persists Text Editor pane mode and read-only settings while older layouts retain safe defaults", () => {
    const desks = [{ id: "notes", name: "Notes", panes: [{ id: "editor", kind: "text_editor" as const, title: "Text Editor", x: 1, y: 1, width: 8, height: 8 }] }];
    const legacy = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "notes" });
    expect(legacy.desks[0].panes[0].textEditorReadOnly ?? false).toBe(false);
    expect(legacy.desks[0].panes[0].textEditorMode ?? "plain").toBe("plain");

    const readOnly = appReducer(legacy, { type: "SET_TEXT_EDITOR_SETTINGS", id: "editor", readOnly: true });
    const rendered = appReducer(readOnly, { type: "SET_TEXT_EDITOR_SETTINGS", id: "editor", mode: "split" });
    expect(rendered.desks[0].panes[0]).toEqual(expect.objectContaining({ textEditorReadOnly: true, textEditorMode: "split" }));
  });

  it("hydrates legacy pane layouts without requiring newly added pane fields", () => {
    const desks = [{
      id: "legacy-workspace",
      name: "Legacy workspace",
      panes: [
        { id: "virtual", kind: "virtual_playbacks" as const, title: "Virtual Playbacks", x: 1, y: 1, width: 8, height: 8 },
        { id: "files", kind: "file_manager" as const, title: "File Manager", x: 9, y: 1, width: 8, height: 8 },
        { id: "notes", kind: "text_editor" as const, title: "Text Editor", x: 17, y: 1, width: 8, height: 8 },
      ],
    }];

    const legacy = appReducer(initialState, {
      type: "HYDRATE_LAYOUT",
      desks,
      activeDeskId: "legacy-workspace",
    });
    const [virtual, files, notes] = legacy.desks[0].panes;

    expect(virtual.virtualPlaybackRows ?? 2).toBe(2);
    expect(virtual.virtualPlaybackColumns ?? 2).toBe(2);
    expect(virtual.virtualPlaybackCells ?? []).toEqual([]);
    expect(virtual.virtualPlaybackExclusionZones ?? []).toEqual([]);
    expect(files.fileManagerShowHidden ?? false).toBe(false);
    expect(notes.textEditorReadOnly ?? false).toBe(false);
    expect(notes.textEditorMode ?? "plain").toBe("plain");
  });

  it("hydrates persisted built-in window settings without requiring them in older layouts", () => {
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId, windowSettings: { builtIn: "dmx", dockMode: "builtins", stageView: "3d", dmxDotSize: "large", fixtureGroupsVisible: false, presetGroupsVisible: false } });
    expect(hydrated.builtIn).toBe("dmx");
    expect(hydrated.dockMode).toBe("builtins");
    expect(hydrated.stageView).toBe("3d");
    expect(hydrated.dmxDotSize).toBe("large");
    expect(hydrated.fixtureGroupsVisible).toBe(false);
    expect(hydrated.presetGroupsVisible).toBe(false);
    const legacy = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId });
    expect(legacy.stageView).toBe(initialState.stageView);
  });

  it("persists preset family independently on a preset pane and migrates legacy panes", () => {
    const desks = [{ id: "test", name: "Test", panes: [{ id: "pool", kind: "presets" as const, title: "Presets", x: 1, y: 1, width: 6, height: 6 }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "test" });
    expect(hydrated.desks[0].panes[0].presetFamily).toBe("All");
    const color = appReducer(hydrated, { type: "SET_PANE_PRESET_FAMILY", id: "pool", family: "Color" });
    expect(color.desks[0].panes[0].presetFamily).toBe("Color");
    expect(color.presetFamily).toBe("All");
  });

  it("migrates the legacy Programming preset pane to the all-presets pool", () => {
    const desks = [{ id: "programming", name: "Programming", panes: [{ id: "presets", kind: "presets" as const, title: "Color & Position Presets", x: 1, y: 1, width: 9, height: 18, presetFamily: "Position" as const }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "programming" });
    expect(hydrated.desks[0].panes[0]).toMatchObject({ title: "All Presets", presetFamily: "All" });
  });

  it("keeps pool colors and Set configuration mode independently configurable", () => {
    const plain = appReducer(initialState, { type: "SET_PRESET_POOL_COLORS", value: false });
    expect(plain.presetPoolColors).toBe(false);
    const armed = appReducer(plain, { type: "SET_PRESET_SET_ARMED", value: true });
    expect(armed.presetSetArmed).toBe(true);
  });

  it("keeps the selected pool playback while Set waits for a fader target", () => {
    const armed = appReducer(initialState, { type: "SET_CUELIST_SET_ARMED", value: true });
    const selected = appReducer(armed, { type: "SET_CUELIST_SET_TARGET", value: 42 });
    expect(selected.cueListSetArmed).toBe(true);
    expect(selected.cueListSetTarget).toBe(42);
    expect(appReducer(selected, { type: "SET_CUELIST_SET_ARMED", value: false }).cueListSetTarget).toBeNull();
  });

  it("arms and clears playback configuration Set selection", () => {
    const armed = appReducer(initialState, { type: "SET_PLAYBACK_SET_ARMED", value: true });
    expect(armed.playbackSetArmed).toBe(true);
    expect(appReducer(armed, { type: "SET_PLAYBACK_SET_ARMED", value: false }).playbackSetArmed).toBe(false);
  });

  it("returns the Cuelists built-in to the pool when its button is clicked from a Cuelist", () => {
    const opened = appReducer(initialState, { type: "OPEN_BUILTIN", kind: "cuelists" });
    const inside = appReducer(opened, { type: "OPEN_BUILTIN_CUELIST", number: 7 });
    const returned = appReducer(inside, { type: "OPEN_BUILTIN", kind: "cuelists" });
    expect(returned).toMatchObject({ builtIn: "cuelists", cuelistBuiltInView: "pool", cuelistBuiltInNumber: 7 });
  });

  it("reopens the remembered Cuelist from another screen before returning to the pool", () => {
    const opened = appReducer(initialState, { type: "OPEN_BUILTIN", kind: "cuelists" });
    const inside = appReducer(opened, { type: "OPEN_BUILTIN_CUELIST", number: 12 });
    const elsewhere = appReducer(inside, { type: "OPEN_BUILTIN", kind: "fixtures" });
    const reopened = appReducer(elsewhere, { type: "OPEN_BUILTIN", kind: "cuelists" });
    expect(reopened).toMatchObject({ builtIn: "cuelists", cuelistBuiltInView: "cues", cuelistBuiltInNumber: 12 });
    expect(appReducer(reopened, { type: "OPEN_BUILTIN", kind: "cuelists" }).cuelistBuiltInView).toBe("pool");
  });
});
