import { describe, expect, it } from "vitest";
import type {
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../../api/types";
import { playbackFaderDisplay } from "./feedback";

const playback = {
	target: { type: "dynamic" },
} as PlaybackDefinition;

function projection(
	state: "pending" | "off" | "failed",
	missingTargetCount = 0,
): PlaybackRuntimeProjection {
	return {
		target: "dynamic",
		runtime: {
			state,
			controller_status: "missing",
			controller_id: "controller-123",
			instance_id: null,
			size: 1,
			master: 1,
			effective_speed_multiplier: 1,
			speed_source: "fixed",
			compatible_target_count: 1,
			target_count: 1,
			supported_address_count: 1,
			lane_count: 1,
			missing_target_count: missingTargetCount,
			unpatched_target_count: 0,
		},
	} as PlaybackRuntimeProjection;
}

describe("Dynamic fader feedback", () => {
	it("shows known dormant Dynamic authority as Off at zero without inventing running state", () => {
		const dormant = {
			target: "dynamic",
			runtime: null,
		} as PlaybackRuntimeProjection;
		expect(playbackFaderDisplay(playback, undefined, 0, dormant)).toBe(
			"OFF · 0%",
		);
		for (const unavailable of [
			undefined,
			{ target: "missing" },
			{ target: "dynamic" },
		]) {
			expect(
				playbackFaderDisplay(
					playback,
					undefined,
					0,
					unavailable as PlaybackRuntimeProjection | undefined,
				),
			).toBe("Unavailable");
		}
	});
	it.each([
		"pending",
		"off",
	] as const)("shows %s without a misleading missing-controller diagnostic", (state) => {
		const text = playbackFaderDisplay(
			playback,
			undefined,
			100,
			projection(state),
		);
		expect(text).toMatch(new RegExp(`^${state.toUpperCase()} · Size`));
		expect(text).not.toContain(" · MISSING");
	});
	it("retains Failed controller and actual missing-target diagnostics", () => {
		const text = playbackFaderDisplay(
			playback,
			undefined,
			100,
			projection("failed", 2),
		);
		expect(text).toContain("FAILED · MISSING");
		expect(text).toContain("2 missing");
	});
	it("retains missing-target counts while a controller is pending", () => {
		const text = playbackFaderDisplay(
			playback,
			undefined,
			100,
			projection("pending", 2),
		);
		expect(text).toContain("2 missing");
	});
});
