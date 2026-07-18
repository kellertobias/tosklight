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

  it("closes File Manager back to the built-in that launched it", () => {
    const setup = appReducer(initialState, { type: "OPEN_BUILTIN", kind: "setup" });
    const manager = appReducer(setup, { type: "OPEN_BUILTIN", kind: "file_manager" });
    expect(manager.fileManagerReturn).toMatchObject({ dockMode: "builtins", builtIn: "setup" });
    expect(manager.lastBuiltIn).toBe("setup");

    const closed = appReducer(manager, { type: "CLOSE_FILE_MANAGER" });
    expect(closed).toMatchObject({ dockMode: "builtins", builtIn: "setup", fileManagerReturn: null });
  });

  it("closes File Manager back to the active Desktop that launched it", () => {
    const playback = appReducer(initialState, { type: "OPEN_DESK", id: "playback" });
    const manager = appReducer(playback, { type: "OPEN_BUILTIN", kind: "file_manager" });
    const closed = appReducer(manager, { type: "CLOSE_FILE_MANAGER" });
    expect(closed).toMatchObject({ dockMode: "desks", activeDeskId: "playback", builtIn: null, fileManagerReturn: null });
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
    const hidden = appReducer(initialState, { type: "SET_STAGE_OPTIONS", groupsVisible: false, showSelection: false, showFloorGrid: false, showBeamGuides: false, environmentBrightness: 3 });
    expect(hidden.stageGroupsVisible).toBe(false);
    expect(hidden.stageShowSelection).toBe(false);
    expect(hidden.stageShowFloorGrid).toBe(false);
    expect(hidden.stageShowBeamGuides).toBe(false);
    expect(hidden.stageEnvironmentBrightness).toBe(2);
    expect(appReducer(hidden, { type: "SET_STAGE_OPTIONS", environmentBrightness: -1 }).stageEnvironmentBrightness).toBe(0);
  });

  it("stores beam direction guides independently on a Stage pane", () => {
    const updated = appReducer(initialState, { type: "SET_PANE_STAGE_OPTION", id: "stage", option: "showBeamGuides", value: false });
    expect(updated.desks.find((desk) => desk.id === updated.activeDeskId)?.panes.find((pane) => pane.id === "stage")?.showBeamGuides).toBe(false);
    expect(updated.stageShowBeamGuides).toBe(true);
  });

  it("persists the selected Development catalog on its pane", () => {
    const desks = [{ id: "test", name: "Test", panes: [{ id: "development", kind: "development" as const, title: "Development", x: 1, y: 1, width: 8, height: 8 }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "test" });
    const updated = appReducer(hydrated, { type: "SET_PANE_DEVELOPMENT_VIEW", id: "development", value: "faders" });
    expect(updated.desks[0].panes[0].developmentView).toBe("faders");
  });

  it("persists Cue sidebar visibility while older pane layouts keep it visible", () => {
    const desks = [{ id: "cues", name: "Cues", panes: [{ id: "cues-1", kind: "cues" as const, title: "Cues · Main", x: 1, y: 1, width: 12, height: 12 }] }];
    const legacy = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "cues" });
    expect(legacy.desks[0].panes[0].showCueSidebar ?? true).toBe(true);

    const hidden = appReducer(legacy, { type: "SET_PANE_CUE_SIDEBAR", id: "cues-1", value: false });
    expect(hidden.desks[0].panes[0].showCueSidebar).toBe(false);
  });

  it("persists each Cues pane's fixed or follow-selection display choice", () => {
    const desks = [{ id: "cues", name: "Cues", panes: [
      { id: "cues-1", kind: "cues" as const, title: "Cues 1", x: 1, y: 1, width: 12, height: 9 },
      { id: "cues-2", kind: "cues" as const, title: "Cues 2", x: 13, y: 1, width: 12, height: 9 },
    ] }];
    const legacy = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "cues" });
    expect(legacy.desks[0].panes[0].cueListSource ?? "fixed").toBe("fixed");

    const fixed = appReducer(legacy, { type: "SET_PANE_CUELIST", id: "cues-1", number: 7 });
    const followed = appReducer(fixed, { type: "SET_PANE_CUELIST", id: "cues-2", source: "follow-selection" });
    expect(followed.desks[0].panes[0]).toEqual(expect.objectContaining({ cueListSource: "fixed", fixedCueListNumber: 7 }));
    expect(followed.desks[0].panes[1]).toEqual(expect.objectContaining({ cueListSource: "follow-selection" }));
    expect(followed.desks[0].panes[1]).not.toHaveProperty("fixedCueListNumber");
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
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId, windowSettings: { builtIn: "dmx", dockMode: "builtins", stageView: "3d", dmxDotSize: "large", fixtureSheetColumns: ["id", "name", "dimmer"], fixtureSheetShowType: false, fixtureSheetShowPatch: false, fixtureSheetShowSubheads: false, fixtureSheetShowMasterHeads: true, fixtureGroupsVisible: false, presetGroupsVisible: false } });
    expect(hydrated.builtIn).toBe("dmx");
    expect(hydrated.dockMode).toBe("builtins");
    expect(hydrated.stageView).toBe("3d");
    expect(hydrated.dmxDotSize).toBe("large");
    expect(hydrated.fixtureSheetColumns).toEqual(["id", "name", "dimmer"]);
    expect(hydrated.fixtureSheetShowType).toBe(false);
    expect(hydrated.fixtureSheetIncludedHeads).toBe("no-sub-heads");
    expect(hydrated.fixtureGroupsVisible).toBe(false);
    expect(hydrated.presetGroupsVisible).toBe(false);
    const legacy = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId });
    expect(legacy.stageView).toBe(initialState.stageView);
    expect(legacy.stageShowFloorGrid).toBe(true);
    expect(legacy.stageShowBeamGuides).toBe(true);
    expect(legacy.fixtureSheetColumns).toEqual(initialState.fixtureSheetColumns);
    expect(legacy.fixtureSheetShowType).toBe(true);
    expect(legacy.fixtureSheetIncludedHeads).toBe("all");

    const current = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId, windowSettings: { fixtureSheetIncludedHeads: "no-master-heads" } });
    expect(current.fixtureSheetIncludedHeads).toBe("no-master-heads");

    const oldPatchDetail = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks: initialState.desks, activeDeskId: initialState.activeDeskId, windowSettings: { fixtureSheetColumns: ["id", "name", "dimmer"], fixtureSheetShowPatch: true } });
    expect(oldPatchDetail.fixtureSheetColumns).toEqual(["id", "name", "patch", "dimmer"]);
  });

  it("keeps at least one valid fixture-sheet column when updating or migrating settings", () => {
    const oneColumn = appReducer(initialState, { type: "SET_FIXTURE_SHEET_OPTIONS", columns: ["name"] });
    expect(oneColumn.fixtureSheetColumns).toEqual(["name"]);
    const rejectedEmpty = appReducer(oneColumn, { type: "SET_FIXTURE_SHEET_OPTIONS", columns: [] });
    expect(rejectedEmpty.fixtureSheetColumns).toEqual(["name"]);

    const migrated = appReducer(initialState, {
      type: "HYDRATE_LAYOUT",
      desks: initialState.desks,
      activeDeskId: initialState.activeDeskId,
      windowSettings: { fixtureSheetColumns: [] },
    });
    expect(migrated.fixtureSheetColumns).toEqual(initialState.fixtureSheetColumns);
  });

  it("persists preset family independently on a preset pane and migrates legacy panes", () => {
    const desks = [{ id: "test", name: "Test", panes: [{ id: "pool", kind: "presets" as const, title: "Presets", x: 1, y: 1, width: 6, height: 6 }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "test" });
    expect(hydrated.desks[0].panes[0].presetFamily).toBe("Mixed");
    const color = appReducer(hydrated, { type: "SET_PANE_PRESET_FAMILY", id: "pool", family: "Color" });
    expect(color.desks[0].panes[0].presetFamily).toBe("Color");
    expect(color.presetFamily).toBe("Mixed");
  });

  it("migrates legacy Programming preset panes and All family state to Mixed", () => {
    const desks = [{ id: "programming", name: "Programming", panes: [{ id: "presets", kind: "presets" as const, title: "All Presets", x: 1, y: 1, width: 9, height: 18, presetFamily: "All" as never }] }];
    const hydrated = appReducer(initialState, { type: "HYDRATE_LAYOUT", desks, activeDeskId: "programming", windowSettings: { presetFamily: "All" as never } });
    expect(hydrated.desks[0].panes[0]).toMatchObject({ title: "Mixed Presets", presetFamily: "Mixed" });
    expect(hydrated.presetFamily).toBe("Mixed");
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
