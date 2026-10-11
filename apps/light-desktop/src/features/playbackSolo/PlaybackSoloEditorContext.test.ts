import { describe, expect, it, vi } from "vitest";
import { PlaybackSoloEditorRegistry, type PlaybackSoloEditor } from "./PlaybackSoloEditorContext";
function editor(): PlaybackSoloEditor {
	return {
		available: true, ready: true, selecting: false, selected: [], name: "Draft",
		saving: false, error: null, begin: vi.fn(), beginEdit: vi.fn(),
		setName: vi.fn(), save: vi.fn(async () => {}), cancel: vi.fn(),
	};
}
describe("Solo surface editor registry", () => {
	it("keeps surfaces independent and cannot remove a replacement editor with stale cleanup", () => {
		const registry = new PlaybackSoloEditorRegistry();
		const main = editor();
		const pane = editor();
		const removeOld = registry.register("main", main);
		const removePane = registry.register("pane:one", pane);
		const next = editor();
		const removeNext = registry.register("main", next);
		removeOld();
		expect(registry.get("main")).toBe(next);
		expect(registry.get("pane:one")).toBe(pane);
		removePane();
		expect(registry.get("pane:one")).toBeNull();
		expect(registry.get("main")).toBe(next);
		removeNext();
		expect(registry.get("main")).toBeNull();
	});
});
