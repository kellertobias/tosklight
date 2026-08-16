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
