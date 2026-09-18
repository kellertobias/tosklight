import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NumberInput } from "./textInputs";

afterEach(() => {
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
});
