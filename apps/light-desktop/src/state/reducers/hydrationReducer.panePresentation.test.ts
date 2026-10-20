import { describe, expect, it } from "vitest";
import { desksWithoutFixtureSheetCompactModes } from "../../components/shell/fixtureSheetCompactModePersistence";
import type { PaneModel } from "../../types";
import { appReducer, initialState } from "../appReducer";

function hydrate(panes: PaneModel[]) {
	return appReducer(initialState, {
		type: "HYDRATE_LAYOUT",
		activeDeskId: "main",
		desks: [{ id: "main", name: "Main", panes }],
	});
}

function pane(options: Partial<PaneModel> = {}): PaneModel {
	return { id: "pool", kind: "groups", title: "Rig Groups", x: 1, y: 1, width: 8, height: 8, ...options };
}

function reopen(panes: PaneModel[]) {
	const state = hydrate(panes);
	return hydrate(JSON.parse(JSON.stringify(desksWithoutFixtureSheetCompactModes(state.desks)))[0].panes).desks[0].panes;
}

describe("additive pane presentation compatibility", () => {
	it("retains automatic pools and legacy fixed-column pools without inventing rows", () => {
		const old = [pane(), pane({ id: "fixed-columns", poolColumns: 4 }), pane({ id: "playbacks", kind: "playbacks", title: "Show Playbacks", surface_id: "pane:playbacks", playbackViewMode: "free-region" })];
		expect(reopen(old)).toEqual(old);
		for (const restored of reopen(old)) expect(restored).not.toHaveProperty("poolTileLayout");
	});

	it.each([
		{ mode: "automatic" as const },
		{ mode: "fixed" as const, rows: 3, columns: 6 },
	])("roundtrips an explicit %s choice without dropping other pane options", (poolTileLayout) => {
		const saved = pane({ title: "Tour Rig", titleIsCustom: true, poolColumns: 4, poolTileLayout, showGroupShortcuts: true });
		expect(reopen([saved])[0]).toEqual(saved);
	});

	it("bounds finite fixed dimensions to the supported range", () => {
		expect(reopen([pane({ poolTileLayout: { mode: "fixed", rows: 2.8, columns: 999 } })])[0].poolTileLayout).toEqual({ mode: "fixed", rows: 2, columns: 24 });
		expect(reopen([pane({ poolTileLayout: { mode: "fixed", rows: -1, columns: 0 } })])[0].poolTileLayout).toEqual({ mode: "fixed", rows: 1, columns: 1 });
	});

	it.each([null, false, "fixed", { mode: "unknown" }, { mode: "fixed", rows: "3", columns: 6 }, { mode: "fixed", rows: NaN, columns: Infinity }])("recovers a malformed additive field without losing the pane: %j", (poolTileLayout) => {
		const saved = pane({ poolColumns: 4, showGroupShortcuts: true, poolTileLayout } as unknown as Partial<PaneModel>);
		const restored = hydrate([saved]).desks[0].panes[0];
		expect(restored).toEqual({ ...saved, poolTileLayout: { mode: "automatic" } });
	});

	it.each([
		["cuelists", "Tour Cuelists"], ["cuelist_pool", "Set List"], ["cuelist_pool", "Tour Qlist_pool Notes"], ["cues", "My QList Notes"], ["file_manager", "Tour Files"],
	] as const)("preserves custom %s titles through portable save and hydration", (kind, title) => {
		expect(reopen([pane({ kind, title })])[0].title).toBe(title);
	});

	it.each([
		["cuelists", "QLists"], ["cuelist_pool", "Playback Pool"], ["cuelist_pool", "Qlist_pool"], ["cuelist_pool", "Cuelist_pool"], ["cues", "Sequence"], ["cues", "Qs · Opening"], ["presets", "All Presets"], ["presets", "Color & Position Presets"],
	] as const)("preserves an explicitly authored legacy-looking %s title: %s", (kind, title) => {
		expect(reopen([pane({ id: "presets", kind, title, titleIsCustom: true })])[0].title).toBe(title);
	});

	it.each([
		["qlists", "QLists", "cuelists", "Cuelists"], ["qlist_pool", "QList Pool", "cuelist_pool", "Cuelist Pool"], ["qlist_pool", "Qlist_pool", "cuelist_pool", "Cuelist Pool"], ["playback_pool", "Playback_pool", "cuelist_pool", "Cuelist Pool"], ["cuelist_pool", "Cuelist_pool", "cuelist_pool", "Cuelist Pool"], ["qs", "Qs · Opening", "cues", "Cues · Opening"], ["presets", "All Presets", "presets", "Mixed Presets"],
	] as const)("still migrates the old default %s/%s", (kind, title, nextKind, nextTitle) => {
		const restored = reopen([pane({ kind, title })])[0];
		expect(restored.kind).toBe(nextKind);
		expect(restored.title).toBe(nextTitle);
	});

	it.each(["cuelist_pool", "qlist_pool", "playback_pool"] as const)("creates %s with the human-readable default title", (kind) => {
		const empty = hydrate([]);
		const opened = appReducer(empty, {
			type: "OPEN_WINDOW_PICKER", rect: { x: 1, y: 1, width: 8, height: 8 },
		});
		const added = appReducer(opened, { type: "ADD_WINDOW", kind });
		const created = added.desks[0].panes[0];
		expect(created.kind).toBe("cuelist_pool");
		expect(created.title).toBe("Cuelist Pool");
		expect(created.poolTileLayout).toEqual({ mode: "automatic" });
		expect(reopen([created])[0]).toEqual(created);
	});

});
