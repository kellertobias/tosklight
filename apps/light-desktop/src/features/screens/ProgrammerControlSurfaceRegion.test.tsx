// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "../../api/types";
import { ProgrammerControlSurfaceRegion } from "./ProgrammerControlSurfaceRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

vi.mock("../../components/control/ControlSection", () => ({
	ControlSection: () => <div data-testid="control-surface" />,
}));

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

	it("renders the one configured main-screen owner", () => {
		mount(null);
		expect(screen.getByTestId("control-surface")).toBeInTheDocument();
	});

	it("moves ownership from main to the named secondary screen", () => {
		const main = mount("screen-2");
		expect(screen.queryByTestId("control-surface")).toBeNull();
		main.unmount();

		mount("screen-2", "screen-2");
		expect(screen.getByTestId("control-surface")).toBeInTheDocument();
	});

	it("lets the main surface explicitly recover controls from a closed owner", () => {
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

		expect(screen.getByRole("alert")).toHaveTextContent(
			"Programmer controls unavailable — assigned to Stage manager",
		);
		fireEvent.click(
			screen.getByRole("button", { name: "Use controls on this screen" }),
		);
		expect(context.updateProgrammerControlSurface).toHaveBeenCalledWith({
			assign_to_main: true,
		});
	});
});
