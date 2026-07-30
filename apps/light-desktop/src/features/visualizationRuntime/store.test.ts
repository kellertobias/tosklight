import { afterEach, describe, expect, it, vi } from "vitest";
import type { VisualizationSnapshot } from "../../api/types";
import { VisualizationRuntimeStore } from "./store";

describe("VisualizationRuntimeStore", () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it("coalesces streamed Live and Preload notifications without waiting for a display frame", async () => {
		const store = new VisualizationRuntimeStore();
		const listener = vi.fn();
		store.subscribe(listener);

		store.installStreamed("normal", snapshot(false));
		store.installStreamed("preload", snapshot(true));

		expect(store.getSnapshot().normal.snapshot?.revision).toBe(1);
		expect(store.getSnapshot().preload.snapshot?.revision).toBe(1);
		expect(listener).not.toHaveBeenCalled();

		await Promise.resolve();

		expect(listener).toHaveBeenCalledOnce();
	});
});

function snapshot(preload: boolean): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "2026-07-29T20:00:00Z",
		grand_master: 1,
		blackout: false,
		preload,
		values: [],
		dynamic_stack: [],
		profile_output_values: [],
	};
}
