import { describe, expect, it, vi } from "vitest";
import { SchedulerRuntimeStore } from "./runtimeStore";

describe("SchedulerRuntimeStore", () => {
	it("notifies the active Scheduler view when semantic runtime state changes", () => {
		const store = new SchedulerRuntimeStore();
		const listener = vi.fn();
		const unsubscribe = store.subscribe(listener);
		const change = {
			show_id: "00000000-0000-0000-0000-000000000001",
			schedule_id: "00000000-0000-0000-0000-000000000002",
			next_occurrence: null,
			last_result: null,
			validation_error: null,
		};

		store.install(change);
		expect(listener).toHaveBeenCalledOnce();
		expect(store.snapshot()).toEqual({ revision: 1, change });

		unsubscribe();
		store.install({ ...change, validation_error: "Playback moved" });
		expect(listener).toHaveBeenCalledOnce();
	});
});
