import { describe, expect, it } from "vitest";
import type { DynamicRuntimeSnapshotProjection } from "../api/types";
import { runningFixtureCount } from "./DynamicsWindow";

describe("Dynamic pool fixture counts", () => {
	it("counts distinct started targets across owners, ignoring pending and other Dynamics", () => {
		const runtime = {
			instances: [
				{ dynamic_id: "a", pending: false, targets: ["f1", "f2", "f2"] },
				{
					dynamic_id: "a",
					pending: false,
					paused: true,
					targets: ["f2", "f3"],
				},
				{ dynamic_id: "a", pending: true, targets: ["f4"] },
				{ dynamic_id: "b", pending: false, targets: ["f5"] },
			],
		} as DynamicRuntimeSnapshotProjection;
		expect(runningFixtureCount(runtime, "a")).toBe(3);
		expect(runningFixtureCount(runtime, "b")).toBe(1);
		expect(runningFixtureCount(runtime, "missing")).toBe(0);
	});
	it("reports zero before runtime arrives", () => {
		expect(runningFixtureCount(null, "a")).toBe(0);
	});
});
