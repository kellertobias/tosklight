import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CyclingValueToggle } from "./CyclingValueToggle";
import { FadedDivider } from "./FadedDivider";

const options = [
	{ value: "keyframes", label: "Keyframes" },
	{ value: "max_min", label: "Max / min" },
	{ value: "middle_amplitude", label: "Middle / amplitude" },
] as const;

describe("CyclingValueToggle", () => {
	it("shows every value and advances to the next value on press", () => {
		const onChange = vi.fn();
		render(
			<CyclingValueToggle
				ariaLabel="Curve method"
				value="max_min"
				options={options}
				onChange={onChange}
			/>,
		);

		const toggle = screen.getByRole("button", {
			name: "Curve method: Max / min. Press to select Middle / amplitude.",
		});
		expect(within(toggle).getByText("Keyframes")).not.toHaveClass("is-active");
		expect(within(toggle).getByText("Max / min")).toHaveClass("is-active");
		expect(within(toggle).getByText("Middle / amplitude")).not.toHaveClass(
			"is-active",
		);

		fireEvent.click(toggle);

		expect(onChange).toHaveBeenCalledWith("middle_amplitude");
	});

	it("wraps from the last value back to the first", () => {
		const onChange = vi.fn();
		render(
			<CyclingValueToggle
				ariaLabel="Curve method"
				value="middle_amplitude"
				options={options}
				onChange={onChange}
			/>,
		);

		fireEvent.click(
			screen.getByRole("button", {
				name: "Curve method: Middle / amplitude. Press to select Keyframes.",
			}),
		);

		expect(onChange).toHaveBeenCalledWith("keyframes");
	});
});

describe("FadedDivider", () => {
	it("renders an orientation-specific decorative divider", () => {
		const { container } = render(<FadedDivider orientation="vertical" />);

		expect(container.firstChild).toHaveClass("ui-faded-divider", "is-vertical");
		expect(container.firstChild).toHaveAttribute("aria-hidden", "true");
	});
});
