import { render, screen } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { describe, expect, it, vi } from "vitest";
import { ScreenApp } from "./ScreenApp";

vi.mock("./api/ServerRuntime", () => ({
	ServerRuntime: ({ children }: PropsWithChildren) => (
		<div data-testid="server-runtime">{children}</div>
	),
}));
vi.mock("./state/AppContext", () => ({
	AppProvider: ({ children }: PropsWithChildren) => children,
	useApp: () => ({
		state: { desks: [], activeDeskId: "" },
		dispatch: vi.fn(),
	}),
}));
vi.mock("./features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: PropsWithChildren) => (
		<div data-testid="patch-authority">{children}</div>
	),
}));
vi.mock("./features/screens/ScreensContext", () => ({
	useScreens: () => ({
		screens: null,
		saveScreen: vi.fn(),
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
vi.mock("./components/shell/LeftDock", () => ({ LeftDock: () => null }));
vi.mock("./components/shell/WorkspaceView", () => ({
	WorkspaceView: () => null,
}));
vi.mock("./components/shell/NativeDragStrip", () => ({
	NativeDragStrip: () => null,
}));
vi.mock("./features/screens/ScreenPlaybackSection", () => ({
	ScreenPlaybackSection: () => null,
}));

describe("ScreenApp", () => {
	it("provides Patch authority to console-screen panes", () => {
		render(<ScreenApp id="screen-1" />);

		const authority = screen.getByTestId("patch-authority");
		expect(authority).toContainElement(screen.getByTestId("screen-surface"));
		expect(authority).toContainElement(screen.getByTestId("connection-state"));
		expect(authority).toContainElement(screen.getByTestId("desk-loading"));
		expect(authority).not.toContainElement(screen.getByTestId("desk-lock"));
	});
});
