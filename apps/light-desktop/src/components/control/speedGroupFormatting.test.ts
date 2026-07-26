import { describe, expect, it } from "vitest";
import { formatSpeedGroupBpm } from "./speedGroupFormatting";

describe("formatSpeedGroupBpm", () => {
	it("shows exactly one decimal and rounds fractional BPM values", () => {
		expect(formatSpeedGroupBpm(120)).toBe("120.0");
		expect(formatSpeedGroupBpm(128.54)).toBe("128.5");
		expect(formatSpeedGroupBpm(128.56)).toBe("128.6");
	});
});
