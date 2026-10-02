import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { FamilyEncoderComponentSlot } from "../../../../api/familyEncoderModels";
import type { ParameterController } from "../useParameterController";
import { FamilyEncoderSlotSurface } from "./FamilyEncoderSlotSurface";
import { PAN, RED, WHEEL } from "./familyEncoderTestSupport";

/**
 * PROG-002 on semantic family encoders: `A THRU B …` typed in the software value pad or the
 * hardware encoder modal (also driven from OSC `encode/N press` and the keypad) spreads the
 * component over the ordered selection, exactly as a legacy attribute spreads.
 */

afterEach(cleanup);

function controller(
	slot: FamilyEncoderComponentSlot,
	hardwareConnected: boolean,
	options: { unsupported?: boolean } = {},
) {
	const setRange = vi.fn();
	const set = vi.fn();
	const value = {
		hardwareConnected,
		canWriteValues: true,
		hasProgrammerValue: () => false,
		releaseParameter: async () => undefined,
		familyEncoders: {
			componentSlot: () => ({ ...slot, label: slot === PAN ? "Pan" : slot === RED ? "Red" : "Wheel" }),
			display: () =>
				options.unsupported
					? { value: null, text: "—", source: "none" as const, unsupported: true as const }
					: { value: 0, text: "0", source: "requested" as const },
			step: () => undefined,
			set,
			setRange,
		},
	} as unknown as ParameterController;
	return { value, set, setRange };
}

function typeKeys(dialogName: RegExp | string, keys: readonly string[]) {
	const dialog = screen.getByRole("dialog", { name: dialogName });
	for (const key of keys)
		fireEvent.click(
			Array.from(dialog.querySelectorAll("button")).find(
				(button) => (button.getAttribute("aria-label") ?? button.textContent) === key,
			) ?? screen.getByRole("button", { name: key }),
		);
}

const PAN_SPREAD = ["2", "7", "0", "THRU", "−", "2", "7", "0", "THRU", "2", "7", "0", "ENTER"];

describe("semantic family encoder THRU spread", () => {
	it("spreads typed Pan degrees from the software value pad", () => {
		const { value, set, setRange } = controller(PAN, false);
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		fireEvent.click(screen.getByRole("button", { name: "Set Enc 1 · Pan value" }));
		typeKeys("Enc 1 · Pan value", PAN_SPREAD);
		expect(setRange).toHaveBeenCalledExactlyOnceWith(0, [270, -270, 270]);
		expect(set).not.toHaveBeenCalled();
	});

	it("spreads typed Pan degrees from the hardware encoder modal", () => {
		const { value, set, setRange } = controller(PAN, true);
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		fireEvent.click(screen.getByRole("button", { name: /^Encoder 1: Pan,/ }));
		typeKeys("Encoder 1 value", PAN_SPREAD);
		expect(setRange).toHaveBeenCalledExactlyOnceWith(0, [270, -270, 270]);
		expect(set).not.toHaveBeenCalled();
	});

	it.each([false, true])("converts typed percentages to descriptor units (hardware %s)", (hardwareConnected) => {
		const { value, setRange } = controller(RED, hardwareConnected);
		render(<FamilyEncoderSlotSurface controller={value} index={0} />);
		if (hardwareConnected) {
			fireEvent.click(screen.getByRole("button", { name: /^Encoder 1: Red,/ }));
			typeKeys("Encoder 1 value", ["8", "0", "THRU", "2", "0", "ENTER"]);
		} else {
			fireEvent.click(screen.getByRole("button", { name: "Set Enc 1 · Red value" }));
			typeKeys("Enc 1 · Red value", ["8", "0", "THRU", "2", "0", "ENTER"]);
		}
		expect(setRange).toHaveBeenCalledOnce();
		const [index, points] = setRange.mock.calls[0] as [number, number[]];
		expect(index).toBe(0);
		expect(points[0]).toBeCloseTo(0.8, 9);
		expect(points[1]).toBeCloseTo(0.2, 9);
	});

	it("offers no THRU for a slot without published spread or an unsupported Position slot", () => {
		const wheel = controller(WHEEL, false);
		const view = render(<FamilyEncoderSlotSurface controller={wheel.value} index={0} />);
		const open = screen.queryByRole("button", { name: "Set Enc 1 · Wheel value" });
		if (open) {
			fireEvent.click(open);
			expect(screen.queryByRole("button", { name: "THRU" })).toBeNull();
		}
		view.unmount();
		const unsupported = controller(PAN, true, { unsupported: true });
		render(<FamilyEncoderSlotSurface controller={unsupported.value} index={0} />);
		// No value modal opens at all, so no THRU can be typed.
		expect(screen.queryByRole("button", { name: /^Encoder 1: Pan/ })).toBeNull();
		expect(screen.queryByRole("dialog", { name: "Encoder 1 value" })).toBeNull();
		expect(unsupported.setRange).not.toHaveBeenCalled();
		expect(wheel.setRange).not.toHaveBeenCalled();
	});
});
