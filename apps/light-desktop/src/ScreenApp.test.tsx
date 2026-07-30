import { cleanup, render, screen } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ScreenConfiguration } from "./api/types";
import { ScreenApp } from "./ScreenApp";

const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	saveScreen: vi.fn(async () => undefined),
	screens: null as { screens: ScreenConfiguration[] } | null,
}));

vi.mock("./api/ServerRuntime", () => ({
	ServerRuntime: ({ children }: PropsWithChildren) => (
		<div data-testid="server-runtime">{children}</div>
	),
}));
vi.mock("./state/AppContext", () => ({
	AppProvider: ({ children }: PropsWithChildren) => children,
	useApp: () => ({
		state: { desks: [], activeDeskId: "" },
		dispatch: mocks.dispatch,
	}),
}));
vi.mock("./features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: PropsWithChildren) => (
		<div data-testid="patch-authority">{children}</div>
	),
}));
vi.mock("./features/screens/ScreensContext", () => ({
	useScreens: () => ({
		screens: mocks.screens,
		saveScreen: mocks.saveScreen,
	}),
}));
vi.mock("./platform/desktop", () => ({
	useScreenWindowPersistence: () => ({ current: false }),
}));
vi.mock("./components/common/LoadingSurface", () => ({
	LoadingSurface: () => <div data-testid="screen-surface" />,
}));
vi.mock("./components/shell/ConnectionState", () => ({
	ConnectionState: () => <div data-testid="connection-state" />,
}));
vi.mock("./components/shell/DeskLoadingOverlay", () => ({
	DeskLoadingOverlay: () => <div data-testid="desk-loading" />,
}));
vi.mock("./components/modals/DeskLockOverlay", () => ({
	DeskLockOverlay: () => <div data-testid="desk-lock" />,
}));
vi.mock("./components/shell/LeftDock", () => ({
	LeftDock: () => <div data-testid="left-dock" />,
}));
vi.mock("./components/shell/WorkspaceView", () => ({
	WorkspaceView: () => <div data-testid="workspace" />,
}));
vi.mock("./components/shell/NativeDragStrip", () => ({
	NativeDragStrip: () => null,
}));
vi.mock("./features/screens/ScreenPlaybackSection", () => ({
	ScreenPlaybackSection: () => <div data-testid="screen-playbacks" />,
}));
vi.mock("./features/screens/FixedScreenPane", () => ({
	FixedScreenPane: ({ pane }: { pane: { type: string } }) => (
		<div data-testid="fixed-pane" data-pane-type={pane.type} />
	),
}));

function configuredScreen(
	overrides: Partial<ScreenConfiguration> = {},
): ScreenConfiguration {
	return {
		id: "screen-1",
		name: "Output",
		layout: { desks: [], activeDeskId: "desk" },
		content: {
			type: "fixed_pane",
			pane: {
				type: "stage_2d",
				follow_preload: false,
				show_floor_grid: true,
			},
		},
		show_dock: false,
		show_playbacks: true,
		playback_count: 8,
		playback_rows: 1,
		first_playback_slot: 1,
		page_mode: "follow_main",
		show_page_controls: true,
		desired_open: true,
		display_id: null,
		bounds: null,
		fullscreen: false,
		playback_layout: null,
		...overrides,
	};
}

describe("ScreenApp", () => {
	beforeEach(() => {
		mocks.dispatch.mockReset();
		mocks.saveScreen.mockClear();
		mocks.screens = null;
	});
	afterEach(cleanup);

	it("provides Patch authority to console-screen panes", () => {
		render(<ScreenApp id="screen-1" />);

		const authority = screen.getByTestId("patch-authority");
		expect(authority).toContainElement(screen.getByTestId("screen-surface"));
		expect(authority).toContainElement(screen.getByTestId("connection-state"));
		expect(authority).toContainElement(screen.getByTestId("desk-loading"));
		expect(authority).not.toContainElement(screen.getByTestId("desk-lock"));
	});

	it("renders fixed content without hydrating a Desktop or exposing Desktop chrome", () => {
		mocks.screens = { screens: [configuredScreen()] };

		render(<ScreenApp id="screen-1" />);

		expect(screen.getByTestId("fixed-pane")).toHaveAttribute(
			"data-pane-type",
			"stage_2d",
		);
		expect(screen.queryByTestId("workspace")).not.toBeInTheDocument();
		expect(screen.queryByTestId("left-dock")).not.toBeInTheDocument();
		expect(mocks.dispatch).not.toHaveBeenCalledWith(
			expect.objectContaining({ type: "HYDRATE_LAYOUT" }),
		);
		expect(screen.getByTestId("screen-playbacks")).toBeInTheDocument();
	});

	it("keeps Page Controls independently available when Playbacks are hidden", () => {
		mocks.screens = {
			screens: [
				configuredScreen({
					show_playbacks: false,
					show_page_controls: true,
				}),
			],
		};

		render(<ScreenApp id="screen-1" />);

		expect(screen.getByTestId("screen-playbacks")).toBeInTheDocument();
	});
});
