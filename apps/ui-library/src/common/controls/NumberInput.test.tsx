import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NumberInput } from "./textInputs";

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe("NumberInput", () => {
	it("keeps its own options off the DOM input, so React reports no unknown props", () => {
		const errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
		render(
			<NumberInput
				aria-label="Left"
				keyboardLabel="Region 1 left"
				allowThrough
				modalReleaseLabel="Release"
				onStepCommit={() => undefined}
				onKeyboardCommit={() => undefined}
				onRangeCommit={() => undefined}
				onModalRelease={() => undefined}
				unit="px"
				value="4"
				onChange={() => undefined}
			/>,
		);
		const input = screen.getByRole("textbox", { name: "Left" });
		for (const name of [
			"keyboardlabel",
			"keyboardLabel",
			"allowthrough",
			"allowThrough",
			"modalreleaselabel",
			"unit",
		])
			expect(input).not.toHaveAttribute(name);
		expect(errors).not.toHaveBeenCalled();
	});

	it("takes a decimal point when its step is fractional, unless told otherwise", () => {
		function Held({ step, allowDecimal }: { step: number; allowDecimal?: boolean }) {
			const [value, setValue] = useState(0);
			return (
				<NumberInput
					aria-label="Size"
					step={step}
					allowDecimal={allowDecimal}
					value={String(value)}
					onChange={(event) => setValue(Number(event.target.value))}
				/>
			);
		}
		const { unmount } = render(<Held step={0.05} />);
		const size = screen.getByRole("textbox", { name: "Size" });
		expect(size).toHaveAttribute("inputmode", "decimal");
		for (const typed of ["0", "0.", "0.0", "0.05"])
			fireEvent.change(size, { target: { value: typed } });
		expect(size).toHaveValue("0.05");
		unmount();

		render(<Held step={0.05} allowDecimal={false} />);
		const whole = screen.getByRole("textbox", { name: "Size" });
		fireEvent.change(whole, { target: { value: "1.5" } });
		expect(whole).toHaveValue("15");
	});

	it("steps a fractional value without floating-point noise", () => {
		function Held() {
			const [value, setValue] = useState("0.2");
			return (
				<NumberInput
					aria-label="Speed"
					step={0.1}
					value={value}
					onChange={(event) => setValue(event.target.value)}
				/>
			);
		}
		render(<Held />);
		fireEvent.click(screen.getByRole("button", { name: "Increase value" }));
		expect(screen.getByRole("textbox", { name: "Speed" })).toHaveValue("0.3");
	});
});
