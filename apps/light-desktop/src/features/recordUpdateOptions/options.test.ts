import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateSettings } from "../../api/types";
import {
	configuredUpdateMode,
	cueUpdateTarget,
	defaultUpdateSettings,
} from "../../components/control/updateWorkflow";
import {
	armedCommandLine,
	commandLineOption,
	legacyRecordDefault,
	migrateLegacyRecordDefaults,
	resolveRecordOption,
	touchCueRecordPlan,
	updateModeForOption,
} from "./options";

let values: Map<string, string>;
beforeEach(() => {
	values = new Map();
	vi.stubGlobal("localStorage", {
		getItem: (key: string) => values.get(key) ?? null,
		setItem: (key: string, value: string) => values.set(key, value),
		removeItem: (key: string) => values.delete(key),
	});
});
afterEach(() => vi.unstubAllGlobals());

function capability(settings: UpdateSettings | null) {
	return {
		loadSettings: vi.fn(async () => settings),
		saveSettings: vi.fn(async (next: UpdateSettings) => next),
	};
}

describe("Record and Update options on the command line", () => {
	it("reads the one-off option an armed line names", () => {
		expect(commandLineOption("RECORD ", "RECORD")).toBeNull();
		expect(commandLineOption("RECORD MERGE ", "RECORD")).toBe("merge");
		expect(commandLineOption("record add existing PBK 3", "RECORD")).toBe(
			"add_existing",
		);
		expect(commandLineOption("UPDATE ADD CUE ", "UPDATE")).toBe("add_cue");
		expect(commandLineOption("UPDATE SMART", "UPDATE")).toBe("smart");
		expect(commandLineOption("UPDATE MERGE", "RECORD")).toBeNull();
		expect(commandLineOption("RECORD MERGED", "RECORD")).toBeNull();
	});

	it("names the choice only when it differs from the stored default", () => {
		expect(armedCommandLine("RECORD ", "RECORD", "merge", "smart")).toBe(
			"RECORD MERGE ",
		);
		expect(armedCommandLine("RECORD MERGE ", "RECORD", "merge", "merge")).toBe(
			"RECORD ",
		);
		expect(
			armedCommandLine("RECORD ADD CUE PBK 2", "RECORD", "smart", "merge"),
		).toBe("RECORD SMART PBK 2");
		expect(armedCommandLine("", "UPDATE", "add_existing", "smart")).toBe(
			"UPDATE ADD EXISTING ",
		);
	});
});

describe("Touch Record with an option", () => {
	it("asks only for Smart and records the other options directly", async () => {
		const ask = vi.fn(async () => "merge" as const);
		await expect(touchCueRecordPlan("smart", ["1"], ask)).resolves.toEqual({
			operation: "merge",
			cueNumber: "1",
		});
		expect(ask).toHaveBeenCalledOnce();
		ask.mockClear();
		await expect(touchCueRecordPlan("merge", ["1"], ask)).resolves.toEqual({
			operation: "merge",
		});
		await expect(
			touchCueRecordPlan("add_existing", ["1"], ask),
		).resolves.toEqual({ operation: "add_missing" });
		await expect(touchCueRecordPlan("add_cue", ["1"], ask)).resolves.toEqual({
			operation: "add_cue",
		});
		expect(ask).not.toHaveBeenCalled();
		await expect(
			touchCueRecordPlan("smart", ["1"], async () => null),
		).resolves.toBeNull();
	});

	it("uses the one-off choice before the stored default and Smart when nothing loads", async () => {
		const stored = capability({
			...defaultUpdateSettings,
			record_default: "add_cue",
		});
		await expect(resolveRecordOption("RECORD ", stored)).resolves.toBe(
			"add_cue",
		);
		await expect(resolveRecordOption("RECORD MERGE ", stored)).resolves.toBe(
			"merge",
		);
		await expect(resolveRecordOption("RECORD ", null)).resolves.toBe("smart");
		const failing = {
			loadSettings: vi.fn(async () => {
				throw new Error("offline");
			}),
		};
		await expect(resolveRecordOption("RECORD ", failing)).resolves.toBe(
			"smart",
		);
	});
});

describe("Touch Update with an option", () => {
	const cue = cueUpdateTarget("list", 1, null);
	const preset = {
		family: { type: "preset" as const },
		object_id: "color:1",
	};
	const settings: UpdateSettings = {
		...defaultUpdateSettings,
		cue_mode: "existing_only",
	};
	it("maps each option onto an Update mode or a new Cue", () => {
		const mode = (option: Parameters<typeof updateModeForOption>[0], target = cue) =>
			updateModeForOption(option, settings, target, configuredUpdateMode);
		expect(mode("smart")).toEqual({ target_type: "cue", mode: "existing_only" });
		expect(mode("merge")).toEqual({ target_type: "cue", mode: "add_new" });
		expect(mode("add_existing")).toEqual({
			target_type: "cue",
			mode: "add_to_current_cue",
		});
		expect(mode("add_cue")).toBeNull();
		expect(mode("merge", preset)).toEqual({
			target_type: "existing_content",
			mode: "add_new",
		});
		expect(mode("add_existing", preset)).toEqual({
			target_type: "existing_content",
			mode: "update_existing",
		});
		expect(mode("add_cue", preset)).toEqual({
			target_type: "existing_content",
			mode: "update_existing",
		});
	});
});

describe("Retired browser Record defaults", () => {
	it("moves Merge into active Cue into the desk default once and removes both keys", async () => {
		values.set("light.store-mode", "overwrite");
		values.set("light.store-merge-active-cue", "true");
		values.set("light.store-cue-only", "true");
		const update = capability(defaultUpdateSettings);
		await expect(migrateLegacyRecordDefaults(update)).resolves.toBe(true);
		expect(update.saveSettings).toHaveBeenCalledWith({
			...defaultUpdateSettings,
			record_default: "merge",
		});
		expect(values.has("light.store-mode")).toBe(false);
		expect(values.has("light.store-merge-active-cue")).toBe(false);
		expect(values.get("light.store-cue-only")).toBe("true");
		expect(legacyRecordDefault()).toBeNull();
		await expect(migrateLegacyRecordDefaults(update)).resolves.toBe(false);
		expect(update.saveSettings).toHaveBeenCalledOnce();
	});

	it("drops the never-applied Record mode without touching the desk", async () => {
		values.set("light.store-mode", "overwrite");
		values.set("light.store-merge-active-cue", "false");
		const update = capability(defaultUpdateSettings);
		await expect(migrateLegacyRecordDefaults(update)).resolves.toBe(false);
		expect(update.loadSettings).not.toHaveBeenCalled();
		expect(values.size).toBe(0);
	});

	it("never replaces an explicit desk default", async () => {
		values.set("light.store-merge-active-cue", "true");
		const update = capability({
			...defaultUpdateSettings,
			record_default: "add_cue",
		});
		await expect(migrateLegacyRecordDefaults(update)).resolves.toBe(false);
		expect(update.saveSettings).not.toHaveBeenCalled();
		expect(values.size).toBe(0);
	});

	it("keeps the browser keys when the desk cannot take the default yet", async () => {
		values.set("light.store-merge-active-cue", "true");
		const unavailable = capability(null);
		await expect(migrateLegacyRecordDefaults(unavailable)).resolves.toBe(false);
		expect(values.get("light.store-merge-active-cue")).toBe("true");
	});
});
