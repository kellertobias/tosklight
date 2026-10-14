import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	FixtureIcon,
	FixtureModeSelect,
	FixtureTypeIcon,
} from "./fixtureDisplay";

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

		rerender(<FixtureTypeIcon type="venue" />);
		icon = screen.getByRole("img", { name: "Type: venue" });
		expect(
			decodeURIComponent(icon.querySelector("img")?.getAttribute("src") ?? ""),
		).toContain("fixture type venue");

		rerender(<FixtureTypeIcon type="rigging" />);
		icon = screen.getByRole("img", { name: "Type: rigging" });
		expect(
			decodeURIComponent(icon.querySelector("img")?.getAttribute("src") ?? ""),
		).toContain("fixture type rigging");
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

function mode(profileId: string, revision = 1, name = "Robin DLS Profile") {
	return {
		id: "mode-definition",
		profile_id: profileId,
		revision,
		name,
		mode: "Mode 3",
		mode_id: "mode-3",
		footprint: 36,
		profile_snapshot: { name },
		patch_policy: "dmx",
	} as unknown as import("../../../api/types").FixtureDefinition;
}
describe("Mode picker installed profile identity", () => {
	it("distinguishes same-name/revision profiles and selects their exact composite keys", () => {
		const modes = [
			mode("79e6cc1e-0000-4000-8000-000000000001"),
			mode("602fcfc8-0000-4000-8000-000000000002"),
		];
		const changed = vi.fn();
		const { rerender } = render(
			<FixtureModeSelect modes={modes} value="" onChange={changed} />,
		);
		for (const [index, id] of ["79e6cc1e", "602fcfc8"].entries()) {
			fireEvent.click(screen.getByRole("button", { name: /^Mode/ }));
			fireEvent.click(
				screen.getByRole("option", {
					name: new RegExp(`Mode 3.*36ch.*Robin DLS Profile.*Rev 1.*${id}`),
				}),
			);
			const key = `${modes[index].profile_id}:1:mode-3`;
			expect(changed).toHaveBeenLastCalledWith(key);
			rerender(
				<FixtureModeSelect modes={modes} value={key} onChange={changed} />,
			);
			expect(screen.getByRole("button", { name: /^Mode/ })).toHaveTextContent(
				id,
			);
			rerender(<FixtureModeSelect modes={modes} value="" onChange={changed} />);
		}
	});
	it("distinguishes revisions while leaving unambiguous choices concise", () => {
		const modes = [mode("profile", 1), mode("profile", 2)];
		const { rerender } = render(
			<FixtureModeSelect
				modes={modes}
				value="profile:1:mode-3"
				onChange={vi.fn()}
			/>,
		);
		expect(screen.getByRole("button", { name: /^Mode/ })).toHaveTextContent(
			/Robin DLS Profile.*Rev 1/,
		);
		rerender(
			<FixtureModeSelect
				modes={[modes[0]]}
				value="profile:1:mode-3"
				onChange={vi.fn()}
			/>,
		);
		expect(screen.getByRole("button", { name: /^Mode/ })).toHaveTextContent(
			"Mode 336ch",
		);
		expect(screen.getByRole("button", { name: /^Mode/ })).not.toHaveTextContent(
			"Rev",
		);
	});
	it("extends stable identity when short UUID prefixes still collide", () => {
		const modes = [mode("12345678-a"), mode("12345678-b")];
		render(<FixtureModeSelect modes={modes} value="" onChange={vi.fn()} />);
		fireEvent.click(screen.getByRole("button", { name: /^Mode/ }));
		expect(screen.getByRole("option", { name: /12345678-a/ })).toBeVisible();
		expect(screen.getByRole("option", { name: /12345678-b/ })).toBeVisible();
	});
});
