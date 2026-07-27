import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { EncoderGroupTabs } from "./EncoderGroupTabs";

afterEach(cleanup);

describe("EncoderGroupTabs", () => {
	it("cycles the active group pages and opens another group on page one", () => {
		const onChange = vi.fn();
		const groups = [
			{ id: "curves", label: "Curves", pageCount: 2 },
			{ id: "speed", label: "Speed" },
		] as const;
		const { rerender } = render(
			<EncoderGroupTabs
				groups={groups}
				activeGroup="curves"
				page={1}
				onChange={onChange}
			/>,
		);

		fireEvent.click(screen.getByRole("button", { name: "Curves (1/2)" }));
		expect(onChange).toHaveBeenLastCalledWith("curves", 2);

		rerender(
			<EncoderGroupTabs
				groups={groups}
				activeGroup="curves"
				page={2}
				onChange={onChange}
			/>,
		);
		fireEvent.click(screen.getByRole("button", { name: "Curves (2/2)" }));
		expect(onChange).toHaveBeenLastCalledWith("curves", 1);

		fireEvent.click(screen.getByRole("button", { name: "Speed" }));
		expect(onChange).toHaveBeenLastCalledWith("speed", 1);
	});
});
