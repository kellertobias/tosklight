import { describe, expect, it } from "vitest";
import type { PaneModel } from "../../types";
import { appReducer, initialState } from "../appReducer";

const legacyPane: PaneModel = {
	id: "stage",
	kind: "stage",
	title: "Tour Stage",
	titleIsCustom: true,
	x: 3,
	y: 2,
	width: 12,
	height: 8,
	stageView: "3d-viz",
	showBeamGuides: true,
};

function reopen(
	stageView: "2d" | "3d" | "3d-viz",
	side: "top" | "front" | "back" | "left" | "right" = "top",
) {
	return appReducer(initialState, {
		type: "HYDRATE_LAYOUT",
		activeDeskId: "tour",
		desks: [{ id: "tour", name: "Tour", panes: [structuredClone(legacyPane)] }],
		windowSettings: {
			stageView,
			stageVizQuality: "ultra",
			stage2dSide: side,
			stageMode: "navigate",
		},
	});
}

describe("lightweight Control Stage compatibility", () => {
	it("opens a saved full-render layout as wireframe without losing its pane or camera controls", () => {
		const loaded = reopen("3d-viz");
		expect(loaded.stageView).toBe("3d");
		expect(loaded.stageVizQuality).toBe("draft");
		expect(loaded.stageMode).toBe("navigate");
		expect(loaded.desks[0].panes[0]).toEqual({
			...legacyPane,
			stageView: "3d",
		});
		const saved = JSON.parse(JSON.stringify(loaded.desks));
		const again = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			activeDeskId: "tour",
			desks: saved,
		});
		expect(again.desks).toEqual(loaded.desks);
	});

	it.each([
		"top",
		"front",
		"back",
		"left",
		"right",
	] as const)("retains the %s plan direction in an older high-quality layout", (side) => {
		const loaded = reopen("2d", side);
		expect(loaded.stageView).toBe("2d");
		expect(loaded.stage2dSide).toBe(side);
		expect(loaded.stageVizQuality).toBe("draft");
	});

	it("keeps inherited pane views inherited and starts fresh desks in the lightweight plan", () => {
		const { stageView: _legacy, ...inherited } = legacyPane;
		const loaded = appReducer(initialState, {
			type: "HYDRATE_LAYOUT",
			desks: [{ id: "tour", name: "Tour", panes: [inherited] }],
			activeDeskId: "tour",
		});
		expect(loaded.stageView).toBe("2d");
		expect(loaded.stageVizQuality).toBe("draft");
		expect(loaded.desks[0].panes[0]).not.toHaveProperty("stageView");
	});

	it("cannot restore full rendering through legacy global or per-pane preference actions", () => {
		let state = reopen("3d");
		state = appReducer(state, { type: "SET_STAGE_VIEW", value: "3d-viz" });
		state = appReducer(state, {
			type: "SET_STAGE_OPTIONS",
			vizQuality: "ultra",
		});
		state = appReducer(state, {
			type: "SET_PANE_STAGE_OPTION",
			id: "stage",
			option: "stageView",
			value: "3d-viz",
		});
		expect(state.stageView).toBe("3d");
		expect(state.stageVizQuality).toBe("draft");
		expect(state.desks[0].panes[0].stageView).toBe("3d");
	});
});
