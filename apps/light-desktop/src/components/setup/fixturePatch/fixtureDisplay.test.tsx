import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { FixtureIcon, FixtureTypeIcon } from "./fixtureDisplay";

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

describe("FixtureIcon", () => {
	it("prefers an explicit fixture icon and otherwise renders the fixture-type icon", () => {
		const { rerender } = render(
			<FixtureIcon
				definition={{
					name: "Touring Wash",
					device_type: "wash",
					icon_asset: "data:image/svg+xml,explicit-fixture-icon",
				}}
			/>,
		);
		let icon = screen.getByRole("img", {
			name: "Fixture icon: Touring Wash",
		});
		expect(icon.querySelector("img")).toHaveAttribute(
			"src",
			"data:image/svg+xml,explicit-fixture-icon",
		);
		expect(screen.queryByRole("img", { name: "Type: wash" })).toBeNull();

		rerender(
			<FixtureIcon
				definition={{
					name: "Touring Wash",
					device_type: "wash",
					icon_asset: null,
				}}
			/>,
		);
		icon = screen.getByRole("img", { name: "Type: wash" });
		expect(
			decodeURIComponent(icon.querySelector("img")?.getAttribute("src") ?? ""),
		).toContain("fixture type led wash moving light lenses");
	});
});
