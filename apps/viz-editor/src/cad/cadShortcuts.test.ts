import { describe, expect, it } from "vitest";
import {
	CAD_MAX_ZOOM,
	CAD_MIN_ZOOM,
	CAD_TOOL_SHORTCUTS,
	cadShortcutFor,
	pannedCamera,
	zoomedCamera,
} from "./cadShortcuts";

const press = (key: string, modifiers: Partial<Record<"ctrlKey" | "metaKey" | "altKey", boolean>> = {}) =>
	cadShortcutFor({ key, ctrlKey: false, metaKey: false, altKey: false, ...modifiers });

describe("CAD keyboard shortcuts", () => {
	it("picks each drawing tool by its letter, in either case", () => {
		expect(
			["v", "l", "p", "t", "m", "r"].map((key) => press(key)),
		).toEqual(
			["select", "polyline", "box", "text", "measure", "erase"].map((tool) => ({ type: "tool", tool })),
		);
		expect(press("L")).toEqual({ type: "tool", tool: "polyline" });
	});

	it("names each tool's key for the title's tooltips from the keys it actually handles", () => {
		expect(CAD_TOOL_SHORTCUTS).toEqual({
			select: "V",
			polyline: "L",
			box: "P",
			text: "T",
			measure: "M",
			erase: "R",
		});
		for (const [tool, key] of Object.entries(CAD_TOOL_SHORTCUTS))
			expect(press(key.toLowerCase())).toEqual({ type: "tool", tool });
	});

	it("chooses the five views by 1 to 5, in the order the view menu lists them", () => {
		expect(["1", "2", "3", "4", "5"].map((key) => press(key))).toEqual(
			["top_down", "left_to_right", "right_to_left", "front_to_back", "back_to_front"].map((view) => ({
				type: "view",
				view,
			})),
		);
		expect(press("6")).toBeNull();
	});

	it("zooms with + and − and pans with W, A, S and D", () => {
		expect(press("+")).toMatchObject({ type: "zoom" });
		expect(press("=")).toMatchObject({ type: "zoom" });
		expect((press("+") as { factor: number }).factor).toBeGreaterThan(1);
		expect((press("-") as { factor: number }).factor).toBeLessThan(1);
		expect(press("w")).toEqual({ type: "pan", horizontal: 0, vertical: 1 });
		expect(press("a")).toEqual({ type: "pan", horizontal: -1, vertical: 0 });
		expect(press("s")).toEqual({ type: "pan", horizontal: 0, vertical: -1 });
		expect(press("d")).toEqual({ type: "pan", horizontal: 1, vertical: 0 });
	});

	it("leaves keys with Ctrl, Cmd or Alt to the window", () => {
		expect(press("v", { metaKey: true })).toBeNull();
		expect(press("s", { ctrlKey: true })).toBeNull();
		expect(press("1", { altKey: true })).toBeNull();
	});

	it("groups with Cmd or Ctrl+G and ungroups with Shift added", () => {
		expect(press("g", { metaKey: true })).toEqual({ type: "group" });
		expect(press("g", { ctrlKey: true })).toEqual({ type: "group" });
		expect(
			cadShortcutFor({ key: "G", metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }),
		).toEqual({ type: "ungroup" });
		expect(press("g", { metaKey: true, altKey: true })).toBeNull();
	});

	it("deletes with Delete or Backspace, undoes with Cmd or Ctrl+Z and redoes with Shift added", () => {
		expect(press("Delete")).toEqual({ type: "delete" });
		expect(press("Backspace")).toEqual({ type: "delete" });
		expect(press("Backspace", { metaKey: true })).toBeNull();
		expect(press("z", { metaKey: true })).toEqual({ type: "undo" });
		expect(press("z", { ctrlKey: true })).toEqual({ type: "undo" });
		expect(
			cadShortcutFor({ key: "Z", metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }),
		).toEqual({ type: "redo" });
		expect(press("y", { ctrlKey: true })).toEqual({ type: "redo" });
		expect(press("z", { metaKey: true, altKey: true })).toBeNull();
		// A plain Z is no shortcut at all.
		expect(press("z")).toBeNull();
	});

	it("duplicates with Cmd or Ctrl+D, leaving a plain D to pan", () => {
		expect(press("d", { metaKey: true })).toEqual({ type: "duplicate" });
		expect(press("d", { ctrlKey: true })).toEqual({ type: "duplicate" });
		expect(
			cadShortcutFor({ key: "D", metaKey: true, ctrlKey: false, altKey: false, shiftKey: true }),
		).toBeNull();
		expect(press("d", { metaKey: true, altKey: true })).toBeNull();
		expect(press("d")).toEqual({ type: "pan", horizontal: 1, vertical: 0 });
	});

	it("keeps zoom within the viewport's range and pans by the same screen distance at every zoom", () => {
		expect(zoomedCamera({ pan: [0, 0], zoom: CAD_MAX_ZOOM }, 2).zoom).toBe(CAD_MAX_ZOOM);
		expect(zoomedCamera({ pan: [0, 0], zoom: CAD_MIN_ZOOM }, 0.5).zoom).toBe(CAD_MIN_ZOOM);
		// Looking right or up moves the plan's centre, which is minus the pan, that way.
		expect(pannedCamera({ pan: [0, 0], zoom: 0.1 }, 1, 0).pan).toEqual([-800, 0]);
		expect(pannedCamera({ pan: [0, 0], zoom: 0.2 }, 0, 1).pan).toEqual([0, -400]);
	});
});
