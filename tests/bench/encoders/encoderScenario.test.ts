import { describe, expect, expectTypeOf, it } from "vitest";
import type {
	AbsoluteEncoderPort,
	NormalizedEncoderPort,
	RelativeEncoderPort,
} from "./encoderScenario";
import {
	acceptedEncoderDragTarget,
	type EncoderDragReport,
	recordEncoderDragFrame,
} from "./encoderScenario";

describe("encoder route capabilities", () => {
	it("keeps absolute value entry and relative OSC detents distinct in the type contract", () => {
		expectTypeOf<AbsoluteEncoderPort>().toHaveProperty("set");
		expectTypeOf<AbsoluteEncoderPort>().not.toHaveProperty("add");
		expectTypeOf<RelativeEncoderPort>().toHaveProperty("add");
		expectTypeOf<RelativeEncoderPort>().toHaveProperty("subtract");
		expectTypeOf<RelativeEncoderPort>().not.toHaveProperty("set");
		expectTypeOf<NormalizedEncoderPort>().toHaveProperty("set");
		expectTypeOf<NormalizedEncoderPort>().toHaveProperty("add");
		expect(true).toBe(true);
	});
});

function dragReport(pulses: number): EncoderDragReport {
	const delta = 0.002064039408866995;
	return {
		box: { x: 5, y: 496, width: 142.65625, height: 219 },
		displacement: 20,
		pointerEvents: [],
		inputs: Array.from({ length: pulses }, (_, index) => ({
			attribute: "intensity",
			fixtureIds: ["fixture-1"],
			requestId: `pulse-${index}`,
			delta,
			sentAt: index * 80,
			acknowledged: true,
		})),
		normal: {
			projection: {
				fixture_values: [
					{
						fixture_id: "fixture-1",
						attribute: "intensity",
						value: { kind: "normalized", value: pulses * 0.002064 },
						fade: false,
					},
				],
			},
		},
	};
}

describe("physical drag accepted-input oracle", () => {
	it("distinguishes the actual two-pulse local byte1 and three-pulse CI byte2 without fitting DMX", () => {
		expect(Math.round(acceptedEncoderDragTarget(dragReport(2)) * 255)).toBe(1);
		expect(Math.round(acceptedEncoderDragTarget(dragReport(3)) * 255)).toBe(2);
	});
	it.each([
		"missing",
		"duplicate",
		"rejected",
		"wrong-target",
		"fading",
		"zero",
		"excessive",
		"wrong-fixture",
	])("refuses %s evidence before asserting output", (kind) => {
		const report = dragReport(3);
		const row = report.normal?.projection.fixture_values[0];
		if (!row) throw new Error("Missing test fixture Intensity");
		if (kind === "missing") report.inputs = [];
		if (kind === "duplicate")
			report.inputs[1].requestId = report.inputs[0].requestId;
		if (kind === "rejected") report.inputs[1].acknowledged = false;
		if (kind === "wrong-target") row.value.value += report.inputs[0].delta;
		if (kind === "fading") row.fade = true;
		if (kind === "zero") row.value.value = 0;
		if (kind === "excessive") row.value.value = 0.2;
		if (kind === "wrong-fixture")
			report.inputs[0].fixtureIds = ["another-fixture"];
		expect(() => acceptedEncoderDragTarget(report)).toThrow();
	});
	it("records only relative request quantity and matching successful receipt, excluding credentials", () => {
		const report = dragReport(0);
		recordEncoderDragFrame(
			report,
			JSON.stringify({
				session_id: "private",
				action: {
					type: "programming_values",
					request: {
						request_id: "relative-1",
						action: {
							attribute: "intensity",
							fixture_ids: ["fixture-1"],
							operation: { type: "relative_step", delta: 0.01 },
						},
					},
				},
			}),
			true,
		);
		expect(report.inputs[0].acknowledged).toBe(false);
		recordEncoderDragFrame(
			report,
			JSON.stringify({ request_id: "other", ok: true }),
			false,
		);
		expect(report.inputs[0].acknowledged).toBe(false);
		recordEncoderDragFrame(
			report,
			JSON.stringify({ request_id: "relative-1", ok: true }),
			false,
		);
		expect(report.inputs[0]).toMatchObject({ delta: 0.01, acknowledged: true });
		expect(JSON.stringify(report)).not.toContain("private");
	});
});
