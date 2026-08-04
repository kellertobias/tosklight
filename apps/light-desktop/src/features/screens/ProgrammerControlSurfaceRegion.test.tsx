// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProgrammerControlSurfaceRegion } from "./ProgrammerControlSurfaceRegion";
import { ScreensProvider } from "./ScreensContext";
import type { ScreensContextValue } from "./types";

vi.mock("../../components/control/ControlSection", () => ({
	ControlSection: () => <div data-testid="control-surface" />,
}));

function source(ownerScreenId: string | null): ScreensContextValue {
	return {
		screens: {
			screens: [],
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

function mount(ownerScreenId: string | null, surfaceScreenId?: string) {
	return render(
		<ScreensProvider source={source(ownerScreenId)}>
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
});
