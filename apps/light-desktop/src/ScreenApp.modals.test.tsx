import { ModalProvider } from "@tosklight/ui/modals";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ScreenApp } from "./ScreenApp";

vi.mock("./api/ServerRuntime", () => ({
	ServerRuntime: ({ children }: PropsWithChildren) => children,
}));
vi.mock("./features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: PropsWithChildren) => children,
}));
vi.mock("./features/selectiveImport/SelectiveImportContext", () => ({
	useSelectiveImport: () => ({
		catalog: vi.fn(),
		preview: vi.fn(),
		apply: vi.fn(),
	}),
}));
vi.mock("./features/patch/PatchState", () => ({
	usePatchedFixturesView: () => [],
}));
vi.mock("./features/deskSnapshot/DeskSnapshotState", () => ({
	useActiveShow: () => null,
	useBootstrapSnapshot: () => null,
	useSessionSnapshot: () => null,
}));
vi.mock("./features/screens/ScreensContext", () => ({
	useScreens: () => ({
		screens: {
			screens: [
				{
					id: "screen-1",
					name: "Desktop Screen",
					content: { type: "desktop" },
					layout: { desks: [], activeDeskId: "desk" },
					show_dock: true,
					show_playbacks: false,
					show_programmer: false,
				},
			],
		},
		saveScreen: async () => undefined,
	}),
}));
vi.mock("./platform/desktop", async (importOriginal) => ({
	...(await importOriginal<object>()),
	useScreenWindowPersistence: () => ({ current: false }),
	useDesktopBridge: () => ({ available: false }),
}));
vi.mock("./components/shell/WorkspaceView", () => ({
	WorkspaceView: () => null,
}));
vi.mock("./components/shell/NativeDragStrip", () => ({
	NativeDragStrip: () => null,
}));
vi.mock("./components/shell/Clock", () => ({ Clock: () => null }));
vi.mock("./components/shell/ConnectionState", () => ({
	ConnectionState: () => null,
}));
vi.mock("./components/shell/DeskLoadingOverlay", () => ({
	DeskLoadingOverlay: () => null,
}));
vi.mock("./components/modals/DeskLockOverlay", () => ({
	DeskLockOverlay: () => null,
}));
vi.mock("./components/modals/DebugModal", () => ({ DebugModal: () => null }));
vi.mock("./components/modals/SpecialDialogsModal", () => ({
	SpecialDialogsModal: () => null,
}));
vi.mock("./components/modals/SystemControlsModal", () => ({
	SystemControlsModal: () => null,
}));
vi.mock("./components/modals/PreloadStoreModal", () => ({
	PreloadStoreModal: () => null,
}));
vi.mock("./components/modals/StoreSettingsModal", () => ({
	StoreSettingsModal: () => null,
}));
vi.mock("./components/modals/RecordChoiceHost", () => ({
	RecordChoiceHost: () => null,
}));
vi.mock("./components/modals/UpdateWorkflow", () => ({
	UpdateWorkflow: () => null,
}));
vi.mock("./components/modals/CommandChoiceModal", () => ({
	CommandChoiceModal: () => null,
}));
vi.mock("./components/modals/ShowRecoveryModal", () => ({
	ShowRecoveryModal: () => null,
}));

beforeEach(() => {
	vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
});
afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

describe("Screen desk modal host", () => {
	it("opens and closes the real Show menu through the Desktop Screen dock and reducer", () => {
		render(
			<ModalProvider>
				<ScreenApp id="screen-1" />
			</ModalProvider>,
		);
		expect(
			screen.queryByRole("dialog", { name: "Show" }),
		).not.toBeInTheDocument();
		fireEvent.click(screen.getByRole("button", { name: /^Open show menu/ }));
		expect(screen.getByRole("dialog", { name: "Show" })).toBeVisible();
		fireEvent.keyDown(window, { key: "Escape" });
		expect(
			screen.queryByRole("dialog", { name: "Show" }),
		).not.toBeInTheDocument();
	});
});
