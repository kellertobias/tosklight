import { describe, expect, it } from "vitest";
import { decodeShowObject } from "../../api/showObjectWire";
import { appReducer, initialState } from "../../state/appReducer";
import {
	normalizeRecordedScroll,
	restorePaneScroll,
} from "./recordedPaneScroll";

describe("recorded pane presentation", () => {
	it("round-trips additive pane positions while legacy views retain their original shape", () => {
		const pane = initialState.desks[0].panes[0];
		const snapshot = {
			"groups:0": { top: 500, left: 0, poolPosition: 99, poolOffset: -4 },
		};
		for (const value of [pane, { ...pane, recordedScroll: snapshot }]) {
			const body = {
				desks: [{ id: "saved", name: "Saved", panes: [value] }],
				activeDeskId: "saved",
			};
			const loaded = decodeShowObject(
				JSON.parse(
					JSON.stringify({
						kind: "user_layout",
						id: "layout",
						revision: 1,
						updated_at: "",
						body,
					}),
				),
				"user_layout",
			);
			expect(loaded.body.desks[0].panes[0]).toEqual(value);
		}
	});
	it("records only the addressed view and clones current positions without overwriting its source", () => {
		const source = initialState.desks[0];
		const positions = {
			[source.panes[0].id]: {
				"groups:0": { top: 500, left: 0, poolPosition: 99, poolOffset: 0 },
			},
		};
		const recorded = appReducer(initialState, {
			type: "RECORD_DESK_SCROLL",
			id: source.id,
			positions,
		});
		expect(recorded.desks[1]).toBe(initialState.desks[1]);
		const current = {
			[source.panes[0].id]: {
				"groups:0": { top: 1000, left: 0, poolPosition: 199, poolOffset: 0 },
			},
		};
		const clone = appReducer(
			appReducer(recorded, { type: "START_SAVE_DESK", positions: current }),
			{ type: "NEW_DESK" },
		);
		expect(clone.desks[0].panes[0].recordedScroll).toEqual(
			positions[source.panes[0].id],
		);
		expect(clone.desks.at(-1)!.panes[0].recordedScroll).toEqual(
			current[source.panes[0].id],
		);
		expect(clone.desks.at(-1)!.panes[0].id).not.toBe(source.panes[0].id);
	});
	it("ignores malformed recorded coordinates without blocking legacy layout recovery", () => {
		expect(
			normalizeRecordedScroll({
				bad: { top: NaN, left: 0 },
				negative: { top: -1, left: 0 },
				valid: { top: 15, left: 3 },
			}),
		).toEqual({ valid: { top: 15, left: 3 } });
		expect(normalizeRecordedScroll(null)).toEqual({});
	});
	it("clamps smaller content and waits when a recorded slot has not rendered", () => {
		const node = document.createElement("div");
		Object.defineProperties(node, {
			clientHeight: { value: 100 },
			scrollHeight: { value: 200 },
			clientWidth: { value: 100 },
			scrollWidth: { value: 120 },
		});
		expect(
			restorePaneScroll(node, {
				top: 800,
				left: 60,
				poolPosition: 99,
				poolOffset: 0,
			}),
		).toBe(false);
		expect(node.scrollTop).toBe(100);
		expect(node.scrollLeft).toBe(20);
	});
	it("waits for delayed horizontal extent before completing a list restoration", () => {
		const node = document.createElement("div");
		let width = 100;
		Object.defineProperties(node, {
			clientHeight: { value: 100 },
			scrollHeight: { value: 100 },
			clientWidth: { value: 100 },
			scrollWidth: { get: () => width },
		});
		expect(restorePaneScroll(node, { top: 0, left: 60 })).toBe(false);
		width = 300;
		expect(restorePaneScroll(node, { top: 0, left: 60 })).toBe(true);
		expect(node.scrollLeft).toBe(60);
	});
});
