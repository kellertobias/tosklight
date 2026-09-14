import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../wire";
import {
	CHAIN_MODES,
	chainModeLabel,
	chainModeOf,
	sceneryOptionChange,
} from "./sceneryOptions";

const chain = (scenery_options?: PatchedFixture["scenery_options"]) =>
	({ id: "chain-1", scenery_options }) as unknown as PatchedFixture;

describe("chain mode", () => {
	it("offers the three ways a chain is rigged", () => {
		expect(CHAIN_MODES.map((mode) => mode.label)).toEqual([
			"Plain chain",
			"Motor on top",
			"Motor on bottom",
		]);
	});

	it("reads the mode from the hoist, with an absent top hanging from one", () => {
		expect(chainModeOf(chain())).toBe("motor_top");
		expect(chainModeOf(chain({ chain_bottom: "motor" }))).toBe("motor_top");
		expect(chainModeOf(chain({ chain_top: "motor", chain_bottom: "direct" }))).toBe(
			"motor_top",
		);
		expect(
			chainModeOf(chain({ chain_top: "steelflex_loop", chain_bottom: "motor" })),
		).toBe("motor_bottom");
		expect(chainModeOf(chain({ chain_top: "direct", chain_bottom: "direct" }))).toBe(
			"plain",
		);
		expect(chainModeLabel("motor_bottom")).toBe("Motor on bottom");
	});

	it("writes both ends for every mode and keeps the colour", () => {
		const fixture = chain({ colour_srgb: "#FF0000", chain_top: "motor" });
		expect(sceneryOptionChange(fixture, "chain", "plain")).toEqual({
			options: { colour_srgb: "#FF0000", chain_top: "direct", chain_bottom: "direct" },
		});
		expect(sceneryOptionChange(fixture, "chain", "motor_top")).toEqual({
			options: {
				colour_srgb: "#FF0000",
				chain_top: "motor",
				chain_bottom: "steelflex_loop",
			},
		});
		expect(sceneryOptionChange(fixture, "chain", "motor_bottom")).toEqual({
			options: {
				colour_srgb: "#FF0000",
				chain_top: "steelflex_loop",
				chain_bottom: "motor",
			},
		});
		expect(sceneryOptionChange(fixture, "chain", "hoist")).toEqual({
			error: "Choose Plain chain, Motor on top or Motor on bottom.",
		});
	});
});
