import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ColorDialog } from "./color";

afterEach(cleanup);

describe("Color special dialog Tint control", () => {
	it("shows an independent green-magenta control only for compatible selections", () => {
		const changeTint = vi.fn();
		const base = {
			brightness: 0.85,
			colorRangePreview: null,
			colorSheet: createRef<HTMLDivElement>(),
			hue: 0.52,
			saturation: 0.8,
			swatch: "rgb(43,202,217)",
			disabled: false,
			shiftArmed: false,
			cancelColor: vi.fn(),
			changeBrightness: vi.fn(),
			changeTint,
			changeGrayscale: vi.fn(),
			completeColor: vi.fn(),
			moveColor: vi.fn(),
			startColor: vi.fn(),
		};
		const { rerender } = render(
			<ColorDialog
				{...base}
				tint={0.5}
				tintAvailable={false}
				grayscale={0}
				grayscaleAvailable={false}
			/>,
		);
		expect(
			screen.queryByRole("button", { name: "Shift tint toward magenta" }),
		).toBeNull();

		rerender(
			<ColorDialog
				{...base}
				tint={0.6}
				tintAvailable
				grayscale={0}
				grayscaleAvailable={false}
			/>,
		);
		expect(screen.getByText("Magenta 20%")).toBeInTheDocument();
		fireEvent.click(
			screen.getByRole("button", { name: "Shift tint toward green" }),
		);
		expect(changeTint).toHaveBeenCalledWith(-0.05);
	});

	it("shows media grayscale in the Color dialog only for compatible selections", () => {
		const changeGrayscale = vi.fn();
		const base = {
			brightness: 0.85,
			colorRangePreview: null,
			colorSheet: createRef<HTMLDivElement>(),
			hue: 0.52,
			saturation: 0.8,
			tint: 0.5,
			tintAvailable: false,
			swatch: "rgb(43,202,217)",
			disabled: false,
			shiftArmed: false,
			cancelColor: vi.fn(),
			changeBrightness: vi.fn(),
			changeTint: vi.fn(),
			changeGrayscale,
			completeColor: vi.fn(),
			moveColor: vi.fn(),
			startColor: vi.fn(),
		};
		const { rerender } = render(
			<ColorDialog {...base} grayscale={0} grayscaleAvailable={false} />,
		);
		expect(
			screen.queryByRole("button", { name: "Increase grayscale" }),
		).toBeNull();

		rerender(<ColorDialog {...base} grayscale={0.25} grayscaleAvailable />);
		expect(screen.getByText("25%")).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Increase grayscale" }));
		expect(changeGrayscale).toHaveBeenCalledWith(0.05);
	});
});
