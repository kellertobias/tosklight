import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../api/types";
import { PatchWindow } from "./PatchWindow";

vi.mock("../components/setup/FixturePatchSetup", () => ({
	FixturePatchSetupContent: ({
		active,
		onStagePreview,
		onMedia,
	}: {
		active?: boolean;
		onStagePreview?: () => void;
		onMedia?: () => void;
	}) => (
		<div data-testid="patch-content" data-active={String(active)}>
			<div role="button" tabIndex={0} onClick={onStagePreview}>
				Preview Stage
			</div>
			<div role="button" tabIndex={0} onClick={onMedia}>
				Media Servers
			</div>
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

vi.mock("../features/patch/PatchContext", () => ({
	usePatch: () => ({ fixtures: [{ fixture_id: "projected-fixture" }] }),
}));

vi.mock("../api/ServerContext", () => ({
	useServer: () => ({
		configuration: { patch_preview_highlight_dmx: false },
		selectedFixtures: [],
		setPatchPreviewHighlight: vi.fn(),
	}),
}));

vi.mock("../platform/desktop", () => ({
	useDesktopBridge: () => ({ available: false }),
}));

vi.mock("./StageWindow", () => ({
	StageWindow: ({
		patchedFixtures,
	}: {
		patchedFixtures?: PatchedFixture[];
	}) => (
		<div data-testid="stage-fixtures">
			{patchedFixtures?.map((fixture) => fixture.fixture_id).join(",")}
		</div>
	),
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

describe("Patch window Stage preview", () => {
	it("renders from the active Patch projection", () => {
		render(<PatchWindow />);
		fireEvent.click(screen.getByRole("button", { name: "Preview Stage" }));

		expect(screen.getByTestId("stage-fixtures")).toHaveTextContent(
			"projected-fixture",
		);
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
