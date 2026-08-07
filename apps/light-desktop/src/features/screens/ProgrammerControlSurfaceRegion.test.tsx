// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { ProgrammerControlSurfaceRegion } from "./ProgrammerControlSurfaceRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

vi.mock("../../components/control/ControlSection", async () => {
	const { useControlSurfacePolicy } = await import(
		"../../components/control/ControlSurfaceMode"
	);
	return {
		ControlSection: () => {
			const policy = useControlSurfacePolicy();
			return (
				<div
					data-testid="control-surface"
					data-mode={policy?.mode ?? "follow"}
					data-can-toggle={policy?.canToggle ? "true" : "false"}
				/>
			);
		},
	};
});

function source(
	ownerScreenId: string | null,
	configuredScreens: ScreenConfiguration[] = [],
): ScreensContextValue {
	return {
		screens: {
			screens: configuredScreens,
			active_pages: {},
			programmer_control_surface: {
				owner_screen_id: ownerScreenId,
				visible_encoders: 4,
			},
		},
		bootstrap: null,
		session: null,
		saveScreen: vi.fn(),
		deleteScreen: vi.fn(),
		setScreenPage: vi.fn(),
		updateProgrammerControlSurface: vi.fn(),
		updateControlDesk: vi.fn(),
		selectControlDesk: vi.fn(),
		removeClient: vi.fn(),
	};
}

function mount(
	ownerScreenId: string | null,
	surfaceScreenId?: string,
	configuredScreens: ScreenConfiguration[] = [],
) {
	const context = source(ownerScreenId, configuredScreens);
	return render(
		<ScreensProvider source={context}>
			<ProgrammerControlSurfaceRegion screenId={surfaceScreenId} />
		</ScreensProvider>,
	);
}

describe("ProgrammerControlSurfaceRegion", () => {
	afterEach(cleanup);

	it("keeps both sections and the toggle while the main screen holds the encoders", () => {
		mount(null);
		const surface = screen.getByTestId("control-surface");
		expect(surface).toHaveAttribute("data-mode", "follow");
		expect(surface).toHaveAttribute("data-can-toggle", "true");
	});

	it("leaves Playbacks without a toggle on main once the encoders move to a screen", () => {
		const main = mount("screen-2");
		const mainSurface = screen.getByTestId("control-surface");
		expect(mainSurface).toHaveAttribute("data-mode", "playbacks");
		expect(mainSurface).toHaveAttribute("data-can-toggle", "false");
		main.unmount();

		mount("screen-2", "screen-2");
		const encoderSurface = screen.getByTestId("control-surface");
		expect(encoderSurface).toHaveAttribute("data-mode", "programmer");
		expect(encoderSurface).toHaveAttribute("data-can-toggle", "false");
	});

	it("keeps main Playbacks silent about a closed encoder screen", () => {
		const configured = {
			id: "screen-2",
			name: "Stage manager",
			desired_open: false,
		} as ScreenConfiguration;
		const context = source("screen-2", [configured]);
		render(
			<ScreensProvider source={context}>
				<ProgrammerControlSurfaceRegion />
			</ScreensProvider>,
		);

		expect(screen.queryByRole("alert")).toBeNull();
		expect(
			screen.queryByRole("button", { name: "Use encoders on this screen" }),
		).toBeNull();
		expect(screen.getByTestId("control-surface")).toHaveAttribute(
			"data-mode",
			"playbacks",
		);
	});

	it("renders nothing on a screen that does not hold the encoders", () => {
		const configured = {
			id: "screen-2",
			name: "Stage manager",
			desired_open: false,
		} as ScreenConfiguration;
		const context = source("screen-2", [configured]);
		const { container } = render(
			<ScreensProvider source={context}>
				<ProgrammerControlSurfaceRegion screenId="screen-3" />
			</ScreensProvider>,
		);

		expect(container).toBeEmptyDOMElement();
	});
});
