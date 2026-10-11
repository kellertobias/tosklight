import {
	cleanup,
	fireEvent,
	render,
	screen,
	waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";
vi.mock("./PlaybackPaneTitleControls", () => ({ PlaybackPaneTitleControls: () => null }));
const mocks = vi.hoisted(() => ({
	dispatch: vi.fn(),
	update: vi.fn(),
	registry: {} as Record<string, unknown> | null,
	pane: {
		id: "pane-1",
		title: "My Playbacks",
		surface_id: undefined as string | undefined,
		playbackViewMode: undefined as "screen" | "free-region" | undefined,
		playbackScreenSurfaceId: undefined as string | undefined,
	},
}));
vi.mock("../features/screens/ScreensContext", () => ({
	useScreens: () => ({
		surfaces:
			mocks.registry === null ? null : { playback_surfaces: mocks.registry },
		updatePlaybackSurface: mocks.update,
	}),
}));
vi.mock("../state/AppContext", () => ({
	useApp: () => ({
		state: { desks: [{ panes: [mocks.pane] }] },
		dispatch: mocks.dispatch,
	}),
}));
vi.mock("../components/control/PlaybackFaderBank", () => ({
	PlaybackFaderBank: ({ surfaceId, layoutSourceSurfaceId, scrollable }: { surfaceId: string; layoutSourceSurfaceId?: string; scrollable?: boolean }) => (
		<div data-testid="shared-bank" data-layout-source={layoutSourceSurfaceId} data-scrollable={Boolean(scrollable)}>{surfaceId}</div>
	),
}));
afterEach(cleanup);
beforeEach(() => {
	mocks.dispatch.mockReset();
	mocks.update.mockReset().mockResolvedValue(undefined);
	mocks.registry = {};
	mocks.pane.surface_id = undefined;
	mocks.pane.playbackViewMode = undefined;
	mocks.pane.playbackScreenSurfaceId = undefined;
});
describe("unified Playback pane", () => {
	it("renders the existing central surface through the shared bank", () => {
		mocks.pane.surface_id = "screen-linked";
		mocks.registry = { "screen-linked": {} };
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		expect(screen.getByTestId("shared-bank")).toHaveTextContent(
			"screen-linked",
		);
		expect(mocks.update).not.toHaveBeenCalled();
	});
	it("mirrors live Screen rows while keeping pane page identity and disabling scrolling", () => {
		mocks.pane.surface_id = "pane:pane-1";
		mocks.pane.playbackViewMode = "screen";
		mocks.pane.playbackScreenSurfaceId = "screen:second";
		mocks.registry = { "pane:pane-1": {}, "screen:second": {} };
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		expect(screen.getByTestId("shared-bank")).toHaveTextContent("pane:pane-1");
		expect(screen.getByTestId("shared-bank")).toHaveAttribute("data-layout-source", "screen:second");
		expect(screen.getByTestId("shared-bank")).toHaveAttribute("data-scrollable", "false");
	});
	it("reports a missing Screen reference instead of silently substituting Free region", () => {
		mocks.pane.surface_id = "pane:pane-1";
		mocks.pane.playbackViewMode = "screen";
		mocks.pane.playbackScreenSurfaceId = "screen:missing";
		mocks.registry = { "pane:pane-1": {} };
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		expect(screen.getByRole("alert")).toHaveTextContent("selected screen layout is unavailable");
		expect(screen.queryByTestId("shared-bank")).not.toBeInTheDocument();
	});
	it("does no surface work while inactive", () => {
		render(<VirtualPlaybacksWindow paneId="pane-1" active={false} />);
		expect(screen.queryByRole("region")).not.toBeInTheDocument();
		expect(mocks.update).not.toHaveBeenCalled();
	});
	it("waits for authoritative registry before permitting creation", () => {
		mocks.registry = null;
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		expect(
			screen.getByRole("button", { name: "Create playback surface" }),
		).toBeDisabled();
	});
	it("creates only this surface with exact acceptance geometry then binds the pane", async () => {
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		fireEvent.click(
			screen.getByRole("button", { name: "Create playback surface" }),
		);
		await waitFor(() =>
			expect(mocks.dispatch).toHaveBeenCalledWith({
				type: "SET_PANE_PLAYBACK_SURFACE",
				id: "pane-1",
				surfaceId: "pane:pane-1",
			}),
		);
		expect(mocks.update).toHaveBeenCalledExactlyOnceWith("pane:pane-1", {
			name: "My Playbacks",
			page_source: { type: "follow", surface_id: "main" },
			layout: {
				rows: Array.from({ length: 14 }, (_, i) => ({
					first_position: 401 + i * 20,
					count: 20,
					presentation: "buttons",
					button_count: 1,
				})),
			},
		});
	});
	it("retains visible pending state until intent completes", async () => {
		let resolve!: () => void;
		mocks.update.mockImplementation(
			() =>
				new Promise<void>((r) => {
					resolve = r;
				}),
		);
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		fireEvent.click(
			screen.getByRole("button", { name: "Create playback surface" }),
		);
		expect(
			screen.getByRole("button", { name: "Creating surface…" }),
		).toBeDisabled();
		expect(mocks.dispatch).not.toHaveBeenCalled();
		resolve();
		await waitFor(() => expect(mocks.dispatch).toHaveBeenCalledOnce());
	});
	it("reports creation failure without binding an absent surface", async () => {
		mocks.update.mockRejectedValue(new Error("Surface cycle rejected"));
		render(<VirtualPlaybacksWindow paneId="pane-1" />);
		fireEvent.click(
			screen.getByRole("button", { name: "Create playback surface" }),
		);
		expect(await screen.findByRole("alert")).toHaveTextContent(
			"Surface cycle rejected",
		);
		expect(mocks.dispatch).not.toHaveBeenCalled();
		expect(
			screen.getByRole("button", { name: "Create playback surface" }),
		).toBeEnabled();
	});
});
