import { describe, expect, it, vi } from "vitest";
import { defaultUpdateSettings } from "../../components/control/updateWorkflow";
import { programmingUpdateSettingsView } from "./settingsView";

describe("settings shared by visible recording grids", () => {
	it("coalesces concurrent readers and publishes saved settings to all subscribers", async () => {
		const source = {};
		const view = programmingUpdateSettingsView(source);
		const read = vi.fn(async () => ({...defaultUpdateSettings,record_default:"merge" as const}));
		const changed = vi.fn();
		const unsubscribe = view.subscribe(changed);
		await Promise.all([view.ensure(read), programmingUpdateSettingsView(source).ensure(read)]);
		expect(read).toHaveBeenCalledOnce();
		expect(view.getSnapshot()?.record_default).toBe("merge");
		view.install({...defaultUpdateSettings,record_default:"add_cue"});
		expect(changed).toHaveBeenCalledTimes(2);
		expect(view.getSnapshot()?.record_default).toBe("add_cue");
		unsubscribe();
	});
	it("keeps a replaced desk scope separate and retries failed reads", async () => {
		const old = programmingUpdateSettingsView({});
		const current = programmingUpdateSettingsView({});
		old.install({...defaultUpdateSettings,record_default:"merge"});
		await current.ensure(async () => {throw new Error("offline");});
		expect(current.getSnapshot()).toBeNull();
		await current.ensure(async () => defaultUpdateSettings);
		expect(current.getSnapshot()?.record_default).toBe("smart");
		expect(old.getSnapshot()?.record_default).toBe("merge");
	});
});
