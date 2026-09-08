// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TimecodeFrameField } from "./TimecodeFrameField";

afterEach(cleanup);

describe("TimecodeFrameField", () => {
	it("preserves partial typing and immediately saves a complete position", () => {
		const changed = vi.fn();
		function Field() {
			const [value, setValue] = useState(440);
			return (
				<TimecodeFrameField
					label="Duration"
					value={value}
					fps={44}
					minimum={1}
					onChange={(next) => {
						setValue(next);
						changed(next);
					}}
				/>
			);
		}
		render(<Field />);
		const input = screen.getByRole("textbox", { name: "Duration" });
		fireEvent.focus(input);
		for (const value of ["", "0", "00:", "00:00:", "00:00:20.", "00:00:20.0"]) {
			fireEvent.change(input, { target: { value } });
			expect(input).toHaveValue(value);
			expect(changed).not.toHaveBeenCalled();
		}
		fireEvent.change(input, { target: { value: "00:00:20.01" } });
		expect(changed).toHaveBeenLastCalledWith(881);
		fireEvent.change(input, { target: { value: "00:00:20." } });
		fireEvent.keyDown(input, { key: "Escape" });
		expect(input).toHaveValue("00:00:20.01");
	});

	it("explains malformed and out-of-range values on blur without saving them", () => {
		const changed = vi.fn();
		render(
			<TimecodeFrameField
				label="Duration"
				value={440}
				fps={44}
				minimum={1}
				maximum={880}
				onChange={changed}
			/>,
		);
		const input = screen.getByRole("textbox", { name: "Duration" });
		for (const value of ["bad", "00:00:00.00", "00:00:10.44", "00:00:21.00"]) {
			fireEvent.focus(input);
			fireEvent.change(input, { target: { value } });
			expect(input).not.toHaveAttribute("aria-invalid", "true");
			fireEvent.blur(input);
			expect(input).toHaveValue(value);
			expect(input).toHaveAttribute("aria-invalid", "true");
			expect(screen.getByText(/Enter HH:MM:SS.FF/)).toHaveTextContent("44 fps");
		}
		expect(changed).not.toHaveBeenCalled();
		fireEvent.keyDown(input, { key: "Escape" });
		expect(input).toHaveValue("00:00:10.00");
		expect(input).not.toHaveAttribute("aria-invalid", "true");
	});

	it("accepts colon frames and follows external values without interrupting focused edits", () => {
		const changed = vi.fn();
		const { rerender } = render(
			<TimecodeFrameField
				label="Offset"
				value={0}
				fps={44}
				onChange={changed}
			/>,
		);
		const input = screen.getByRole("textbox", { name: "Offset" });
		rerender(
			<TimecodeFrameField
				label="Offset"
				value={44}
				fps={44}
				onChange={changed}
			/>,
		);
		expect(input).toHaveValue("00:00:01.00");
		fireEvent.focus(input);
		fireEvent.change(input, { target: { value: "00:00:" } });
		rerender(
			<TimecodeFrameField
				label="Offset"
				value={88}
				fps={44}
				onChange={changed}
			/>,
		);
		expect(input).toHaveValue("00:00:");
		fireEvent.change(input, { target: { value: "00:00:03:04" } });
		expect(changed).toHaveBeenLastCalledWith(136);
		fireEvent.blur(input);
		expect(input).toHaveValue("00:00:03.04");
	});
});
