import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { PatchWindow } from "./PatchWindow";

const desktop = vi.hoisted(() => ({
	available: true,
	openVisualizer: vi.fn(),
}));

vi.mock("../components/setup/FixturePatchSetup", () => ({
	FixturePatchSetupContent: ({
		active,
		onOpenStageWindow,
		onMedia,
	}: {
		active?: boolean;
		onOpenStageWindow?: () => void;
		onMedia?: () => void;
	}) => (
		<div data-testid="patch-content" data-active={String(active)}>
			<button type="button" onClick={onOpenStageWindow}>
				Open Stage Renderer
			</button>
			<button type="button" onClick={onMedia}>
				Media Servers
			</button>
		</div>
	),
}));

vi.mock("../features/patch/PatchFeatureBoundary", () => ({
	PatchFeatureBoundary: ({ children }: { children: ReactNode }) => (
		<div data-testid="patch-boundary">{children}</div>
	),
}));

vi.mock("../components/setup/MediaServerSetup", () => ({
	MediaServerSetup: () => <div>Media setup</div>,
}));

vi.mock("../components/setup/PsnSetup", () => ({
	PsnSetup: () => <div>Tracking setup</div>,
}));

vi.mock("../components/setup/fixturePatch/ShowPatchSettings", () => ({
	ShowPatchSettings: ({ initialTab }: { initialTab?: string }) => (
		<div role="dialog" aria-label="Show Patch">
			{`Settings on ${initialTab}`}
		</div>
	),
}));

const discovery = vi.hoisted(() => ({
	discoverMediaServers: vi.fn(),
}));

vi.mock("../features/mediaServers/MediaServersContext", () => ({
	useMediaServers: () => discovery,
}));

vi.mock("../platform/desktop", () => ({
	useDesktopBridge: () => desktop,
}));

beforeAll(() => {
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});

afterEach(cleanup);

describe("Patch window Stage renderer", () => {
	it("opens the dedicated Stage renderer", () => {
		render(<PatchWindow />);
		fireEvent.click(
			screen.getByRole("button", { name: "Open Stage Renderer" }),
		);
		expect(desktop.openVisualizer).toHaveBeenCalledOnce();
		expect(screen.queryByText("Preview Stage")).toBeNull();
	});

	it("keeps one Patch boundary across fixture and media views", () => {
		const { rerender } = render(<PatchWindow active={false} />);
		expect(screen.getByTestId("patch-boundary")).toBeInTheDocument();
		expect(screen.getByTestId("patch-content")).toHaveAttribute(
			"data-active",
			"false",
		);

		rerender(<PatchWindow active />);
		expect(screen.getByTestId("patch-content")).toHaveAttribute(
			"data-active",
			"true",
		);

		fireEvent.click(screen.getByRole("button", { name: "Media Servers" }));

		expect(screen.getByTestId("patch-boundary")).toBeInTheDocument();
		expect(screen.getByText("Media setup")).toBeInTheDocument();
	});
});

describe("Show Patch Media Servers and Tracking header", () => {
	const header = () =>
		screen.getByText("Show Patch").closest("header") as HTMLElement;
	const tabs = () =>
		[...header().querySelectorAll('[role="tab"]')].map((tab) => [
			tab.textContent,
			tab.getAttribute("aria-selected"),
		]);

	it("keeps the same tab strip and top-right Settings on both views", () => {
		render(<PatchWindow patchView="media" />);
		expect(tabs()).toEqual([
			["Fixtures", "false"],
			["Media Servers", "true"],
			["Tracking", "false"],
		]);
		expect(screen.getByRole("button", { name: "Settings" })).toBeEnabled();
		// The Settings button is the header's last control, after the tab strip.
		const controls = [...header().querySelectorAll("button")];
		expect(controls.at(-1)).toHaveAccessibleName("Settings");
		expect(screen.getByText("Media setup")).toBeInTheDocument();

		fireEvent.click(screen.getByRole("tab", { name: "Tracking" }));
		expect(tabs()).toEqual([
			["Fixtures", "false"],
			["Media Servers", "false"],
			["Tracking", "true"],
		]);
		expect(screen.getByText("Tracking setup")).toBeInTheDocument();
		expect(
			[...header().querySelectorAll("button")].at(-1),
		).toHaveAccessibleName("Settings");
	});

	it("opens each view's own Settings page", () => {
		render(<PatchWindow patchView="media" />);
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(screen.getByRole("dialog")).toHaveTextContent("Settings on media");
		cleanup();
		render(<PatchWindow patchView="media" />);
		fireEvent.click(screen.getByRole("tab", { name: "Tracking" }));
		fireEvent.click(screen.getByRole("button", { name: "Settings" }));
		expect(screen.getByRole("dialog")).toHaveTextContent(
			"Settings on tracking",
		);
	});

	it("puts Refresh Discovery in its own title group before the view switch, on Media Servers only", async () => {
		discovery.discoverMediaServers.mockReset();
		discovery.discoverMediaServers.mockResolvedValue({
			servers: [],
			discoveryError: null,
		});
		render(<PatchWindow patchView="media" />);
		const refresh = await screen.findByRole("button", {
			name: "Refresh Discovery",
		});
		const group = refresh.closest(".ui-title-chrome-group");
		expect(group).not.toBeNull();
		expect(group?.querySelectorAll("button")).toHaveLength(1);
		const controls = [...header().querySelectorAll("button, [role='tab']")];
		expect(controls.indexOf(refresh)).toBeLessThan(
			controls.indexOf(screen.getByRole("tab", { name: "Fixtures" })),
		);
		expect(discovery.discoverMediaServers).toHaveBeenCalledOnce();
		fireEvent.click(refresh);
		expect(discovery.discoverMediaServers).toHaveBeenCalledTimes(2);

		fireEvent.click(screen.getByRole("tab", { name: "Tracking" }));
		expect(
			screen.queryByRole("button", { name: /Refresh Discovery|Discovering/ }),
		).toBeNull();
	});

	it("scrolls the view in one area filling the window", () => {
		const { container } = render(<PatchWindow patchView="media" />);
		const window = container.querySelector(".patch-configuration-window");
		expect(
			window?.querySelector(
				":scope > .ui-window-scroll-area .patch-configuration-content",
			),
		).not.toBeNull();
	});

	it("leaves Settings to the pane when the Show Patch is a pane", () => {
		render(<PatchWindow patchView="media" compact />);
		expect(screen.queryByRole("button", { name: "Settings" })).toBeNull();
		expect(tabs()).toHaveLength(3);
	});
});
