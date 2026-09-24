import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AttributeConfigurationActionsContextForTest } from "../../../features/attributeConfiguration/AttributeConfigurationActions";
import { ColorDialog } from "./color";

afterEach(cleanup);

describe("Color special dialog Tint control", () => {
	it("shows an independent green-magenta control only for compatible selections", () => {
		const changeTint = vi.fn();
		const base = {
			intent: false,
			selectedFixtureIds: [],
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
			intent: false,
			selectedFixtureIds: [],
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

describe("Color special dialog in Color Intent", () => {
	it("leaves the level to Intensity and names how each selected fixture takes the colour", async () => {
		const colorIntentReport = vi.fn().mockResolvedValue({
			color_model: "intent",
			heads: [
				{
					fixture_id: "a",
					fixture_number: 1,
					fixture_name: "Wash",
					owner_id: "a",
					head_name: "Main",
					has_target: true,
					quality: "exact",
					engine: "additive",
					delta_uv: 0.001,
					calibration_revision: 2,
				},
				{
					fixture_id: "b",
					fixture_number: 2,
					fixture_name: "Spot",
					owner_id: "b",
					head_name: "Main",
					has_target: true,
					quality: "wheel_limited",
					engine: "wheel",
					delta_uv: 0.041,
					calibration_revision: 1,
				},
				{
					fixture_id: "c",
					fixture_number: 3,
					fixture_name: "Dimmer",
					owner_id: "c",
					head_name: "Main",
					has_target: true,
					quality: "unsupported",
					engine: null,
					delta_uv: null,
					calibration_revision: null,
				},
			],
		});
		render(
			<AttributeConfigurationActionsContextForTest
				value={{ colorIntentReport } as never}
			>
				<ColorDialog
					intent
					selectedFixtureIds={["a", "b", "c"]}
					brightness={1}
					colorRangePreview={null}
					colorSheet={createRef<HTMLDivElement>()}
					hue={0}
					saturation={1}
					swatch="rgb(255,0,0)"
					disabled={false}
					shiftArmed={false}
					tint={0.5}
					tintAvailable={false}
					grayscale={0}
					grayscaleAvailable
					cancelColor={vi.fn()}
					changeBrightness={vi.fn()}
					changeTint={vi.fn()}
					changeGrayscale={vi.fn()}
					completeColor={vi.fn()}
					moveColor={vi.fn()}
					startColor={vi.fn()}
				/>
			</AttributeConfigurationActionsContextForTest>,
		);
		expect(
			screen.queryByRole("button", { name: "Increase brightness" }),
		).toBeNull();
		expect(
			screen.getByRole("button", { name: "Increase grayscale" }),
		).toBeInTheDocument();
		const results = await screen.findByRole("region", {
			name: "Color Intent results",
		});
		expect(await screen.findByText(/Fixture 2/)).toBeInTheDocument();
		expect(results.textContent).toContain("Wheel-limited");
		expect(results.textContent).toContain("Unsupported");
		expect(results.textContent).not.toContain("Fixture 1");
		expect(colorIntentReport).toHaveBeenCalledWith(["a", "b", "c"]);
	});
});
