import {
	cleanup,
	fireEvent,
	render,
	screen,
	within,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SetupWindowController } from "./controller";
import { SetupHeader, SetupNavigation, type SetupSection } from "./SetupChrome";

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

describe("Desk Setup focused title tabs", () => {
	function controller(
		overrides: Partial<SetupWindowController>,
	): SetupWindowController {
		return {
			section: "network",
			draft: {} as SetupWindowController["draft"],
			restartRequired: false,
			networkTab: "control-server",
			setNetworkTab: vi.fn(),
			defaultsTab: "record-update",
			setDefaultsTab: vi.fn(),
			programmerSettingsLoaded: true,
			attributeTab: "encoder-groups",
			setAttributeTab: vi.fn(),
			screenCanUndo: false,
			screenUndo: { current: null },
			setDeskLockSettingsOpen: vi.fn(),
			setEncoderPlacementOpen: vi.fn(),
			...overrides,
		} as unknown as SetupWindowController;
	}

	it("offers the literal Network and Defaults tab sets", () => {
		const network = controller({ section: "network" });
		const { rerender } = render(<SetupHeader controller={network} />);
		for (const label of ["Control & server", "Sound", "Bridges"])
			expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Sound" }));
		expect(network.setNetworkTab).toHaveBeenCalledWith("sound");

		const defaults = controller({ section: "preferences-defaults" });
		rerender(<SetupHeader controller={defaults} />);
		for (const label of ["Record & Update", "Playback", "Pool colors"])
			expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Save changes" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: "Playback" }));
		expect(defaults.setDefaultsTab).toHaveBeenCalledWith("playback");
	});

	it("moves encoder placement to an action and uses the exact desk-lock wording", () => {
		const screens = controller({ section: "screens" });
		render(<SetupHeader controller={screens} />);
		fireEvent.click(
			screen.getByRole("button", { name: "Configure encoder placement" }),
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Configure desk lock" }),
		);
		expect(screens.setEncoderPlacementOpen).toHaveBeenCalledWith(true);
		expect(screens.setDeskLockSettingsOpen).toHaveBeenCalledWith(true);
		expect(screen.queryByRole("button", { name: "Desk Lock" })).toBeNull();
	});
});
