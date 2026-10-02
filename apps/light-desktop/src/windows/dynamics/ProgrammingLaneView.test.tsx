import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	DynamicDefinitionProjection,
	DynamicLaneProjection,
} from "../../api/types";
import { type ProgrammingDynamicLane } from "../../features/dynamics/laneModel";
import { curveEditorEncoderSlots } from "./CurveEncoderSlots";
import {
	createDefaultDynamicDefinition,
	createDefaultDynamicLane,
} from "./DynamicsEditor";
import { ProgrammingLaneView } from "./ProgrammingLaneView";

afterEach(cleanup);
function typed(): ProgrammingDynamicLane {
	return {
		id: "typed-pan",
		speed_multiplier: { numerator: 1, denominator: 1 },
		width: 1,
		programming: {
			address: {
				representation: { kind: "angles" },
				component: { kind: "pan" },
			},
			configuration: {
				mode: "middle_amplitude",
				configuration: {
					middle: { kind: "current" },
					amplitude: { kind: "scalar", value: 720 },
					function: "sinus",
					size: 1,
					pwm: {
						attack: 0,
						on: 0.5,
						decay: 0,
						off: 0.5,
						attack_interpolation: "linear",
						decay_interpolation: "linear",
					},
					invert_waveform: false,
				},
			},
		},
	};
}
function definition(): DynamicDefinitionProjection {
	return {
		...createDefaultDynamicDefinition(1, "intensity"),
		lanes: [typed(), createDefaultDynamicLane("intensity", "scalar")],
	};
}

describe("typed Dynamic inspection and scalar edit boundaries", () => {
	it("keeps mixed lane order and selection without a fabricated percentage curve", () => {
		const onSelect = vi.fn();
		render(
			<ProgrammingLaneView
				dynamic={definition()}
				lane={typed()}
				selectedLanes={new Set(["typed-pan"])}
				onSelect={onSelect}
			/>,
		);
		const buttons = screen.getAllByRole("button");
		expect(buttons.map((button) => button.getAttribute("aria-label"))).toEqual([
			"Select lane 1, pan",
			"Select lane 2, intensity",
		]);
		fireEvent.click(buttons[1]);
		expect(onSelect).toHaveBeenCalledWith("scalar", false);
		expect(screen.queryByLabelText("Curve Composer")).toBeNull();
		expect(screen.queryByRole("img")).toBeNull();
	});

	it("keeps typed intent exact when common encoders edit speed and width", async () => {
		let current: DynamicLaneProjection = typed();
		const intent = structuredClone(current.programming);
		const onChange = async (
			update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		) => {
			current = update(current);
		};
		const slots = curveEditorEncoderSlots(
			current,
			definition(),
			0,
			() => {},
			onChange,
			[],
		);
		expect(slots.slice(0, 4).every((slot) => slot.disabled)).toBe(true);
		await slots[4].apply(0.5, "width");
		await slots[5].apply(2, "speed");
		expect(current).toMatchObject({
			width: 0.5,
			speed_multiplier: { numerator: 2, denominator: 1 },
			programming: intent,
		});
		expect("attribute" in current).toBe(false);
	});

	it("a stale scalar encoder cannot add scalar fields to a replacement intent lane", async () => {
		const current = typed();
		const onChange = vi.fn(
			async (
				update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
			) => {
				expect(update(current)).toBe(current);
			},
		);
		const slots = curveEditorEncoderSlots(
			createDefaultDynamicLane("pan"),
			definition(),
			0,
			() => {},
			onChange,
			[],
		);
		await slots[0].apply(0.75, "gesture");
		expect(onChange).toHaveBeenCalledTimes(1);
		expect(current.programming.configuration.mode).toBe("middle_amplitude");
	});
});
