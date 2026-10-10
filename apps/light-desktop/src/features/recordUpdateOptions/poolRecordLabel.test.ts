import { describe, expect, it } from "vitest";
import { effectiveOption } from "./options";
import { poolRecordLabel } from "./poolRecordLabel";

describe("record tile labels describe the touch plan", () => {
	it("keeps empty targets REC even when Merge or Add Cue is the default", () => {
		for (const kind of ["preset", "cuelist", "other"] as const) {
			expect(poolRecordLabel({kind, exists: false}, "merge")).toBe("REC");
			expect(poolRecordLabel({kind, exists: false}, "add_cue")).toBe("REC");
		}
	});
	it("shows Merge only on existing presets with effective Merge", () => {
		const preset = {kind: "preset", exists: true} as const;
		expect(poolRecordLabel(preset, effectiveOption("RECORD", "RECORD", "merge"))).toBe("REC MRG");
		expect(poolRecordLabel(preset, effectiveOption("RECORD SMART", "RECORD", "merge"))).toBe("REC");
		expect(poolRecordLabel(preset, effectiveOption("RECORD MERGE", "RECORD", "smart"))).toBe("REC MRG");
	});
	it("distinguishes next-Cue recording from Smart's one-Cue choice and Cue edits", () => {
		expect(poolRecordLabel({kind:"cuelist",exists:true,cueCount:1}, "smart")).toBe("REC");
		for (const cueCount of [0, 2, 100]) {
			expect(poolRecordLabel({kind:"cuelist",exists:true,cueCount}, "smart")).toBe("REC CUE");
		}
		expect(poolRecordLabel({kind:"cuelist",exists:true,cueCount:1}, "add_cue")).toBe("REC CUE");
		for (const option of ["merge", "add_existing"] as const) {
			expect(poolRecordLabel({kind:"cuelist",exists:true,cueCount:2}, option)).toBe("REC");
		}
	});
});
