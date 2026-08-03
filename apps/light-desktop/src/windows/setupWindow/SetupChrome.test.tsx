import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SetupNavigation, type SetupSection } from "./SetupChrome";

afterEach(cleanup);

describe("Desk Setup navigation", () => {
	it("shows the approved top-level pages and Preferences children in order", () => {
		const onSelect = vi.fn();
		render(
			<SetupNavigation section="preferences-attributes" onSelect={onSelect} />,
		);

		const navigation = screen.getByRole("navigation", { name: "Desk Setup" });
		expect(
			within(navigation)
				.getAllByRole("button")
				.map((button) => button.textContent),
		).toEqual([
			"Shows & recovery",
			"Outputs",
			"Timecode",
			"Network & Inputs",
			"Screens & playback",
			"Defaults",
			"Attributes & encoders",
			"Highlight",
			"Others",
		]);
		expect(within(navigation).getByText("Preferences")).toBeInTheDocument();
		expect(within(navigation).queryByText("Users & sessions")).toBeNull();
		expect(
			within(navigation).queryByRole("button", { name: "Programmer" }),
		).toBeNull();
		expect(
			within(navigation).getByRole("button", { name: "Attributes & encoders" }),
		).toHaveAttribute("aria-current", "page");

		fireEvent.click(within(navigation).getByRole("button", { name: "Others" }));
		expect(onSelect).toHaveBeenCalledWith(
			"preferences-others" satisfies SetupSection,
		);
	});
});
