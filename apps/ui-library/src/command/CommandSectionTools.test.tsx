import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammerKeypadView } from "./CommandSectionTools";

afterEach(cleanup);

describe("ProgrammerKeypadView", () => {
	it("applies desk key colors, Clear states, and disabled navigation inputs", () => {
		const { rerender } = render(
			<ProgrammerKeypadView
				programmerFade={<div>Fade</div>}
				highlightControls={<div>Highlight</div>}
				onPress={vi.fn()}
				clearState="selection"
				disabledKeys={["TRU"]}
			/>,
		);

		for (const key of [
			"DEL",
			"MOV",
			"CPY",
			"SET",
			"GRP",
			"CUE",
			"TIME",
			"DIV",
			"-",
			"+",
			"TRU",
			"AT",
		])
			expect(screen.getByRole("button", { name: key })).toHaveClass("action");
		expect(screen.getByRole("button", { name: "ENT" })).toHaveClass("enter");
		expect(screen.getByRole("button", { name: "TRU" })).toBeDisabled();
		expect(screen.getByRole("button", { name: "CLR" })).toHaveClass(
			"clear-active",
		);

		rerender(
			<ProgrammerKeypadView
				programmerFade={<div>Fade</div>}
				highlightControls={<div>Highlight</div>}
				onPress={vi.fn()}
				clearState="active-values"
			/>,
		);
		expect(screen.getByRole("button", { name: "CLR" })).toHaveClass(
			"clear-warning",
		);
	});
});
