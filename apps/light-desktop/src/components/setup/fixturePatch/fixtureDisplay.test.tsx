import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { FixtureTypeIcon } from "./fixtureDisplay";

afterEach(cleanup);

describe("FixtureTypeIcon", () => {
	it("uses repository fixture-type assets instead of parallel inline glyphs", () => {
		const { rerender } = render(<FixtureTypeIcon type="wash" />);
		let icon = screen.getByRole("img", { name: "Type: wash" });
		expect(icon.querySelector("svg")).not.toBeInTheDocument();
		expect(
			decodeURIComponent(icon.querySelector("img")?.getAttribute("src") ?? ""),
		).toContain("fixture type led wash moving light lenses");

		rerender(<FixtureTypeIcon type="moving_head" />);
		icon = screen.getByRole("img", { name: "Type: moving_head" });
		expect(
			decodeURIComponent(icon.querySelector("img")?.getAttribute("src") ?? ""),
		).toContain("fixture type profile moving light");
	});
});
