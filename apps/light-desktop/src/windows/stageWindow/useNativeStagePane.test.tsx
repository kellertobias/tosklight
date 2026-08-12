// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	DesktopBridge,
	StagePaneGeometry,
} from "../../platform/desktop/types";
import { useNativeStagePane } from "./useNativeStagePane";

const bridge = vi.hoisted(() => ({ current: null as DesktopBridge | null }));

// jsdom has no layout, so it has no ResizeObserver either. The pane only needs to be told when
// the element moves, and nothing moves here.
vi.stubGlobal(
	"ResizeObserver",
	class {
		observe() {}
		unobserve() {}
		disconnect() {}
	},
);

vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => bridge.current,
}));

function stubBridge(overrides: Partial<DesktopBridge>): DesktopBridge {
	return {
		available: true,
		stagePaneAvailable: async () => true,
		openStagePane: vi.fn(async () => undefined),
		setStagePane: vi.fn(async () => undefined),
		closeStagePane: vi.fn(async () => undefined),
		sendStagePaneInput: vi.fn(async () => undefined),
		stagePaneStatus: async () => [null, null],
		onCurrentWindowMoved: async () => () => undefined,
		...overrides,
	} as unknown as DesktopBridge;
}

function Probe() {
	const pane = useNativeStagePane();
	return (
		<div
			ref={pane.ref}
			data-testid="pane"
			data-active={pane.active ? "yes" : "no"}
			data-trouble={pane.trouble ?? ""}
		/>
	);
}

describe("useNativeStagePane", () => {
	afterEach(() => {
		cleanup();
		bridge.current = null;
	});

	/** The whole point of the switch: no native pane means the web renderer keeps drawing. */
	it("stays inactive where the desk cannot draw the Stage itself", async () => {
		const openStagePane = vi.fn(async () => undefined);
		bridge.current = stubBridge({
			stagePaneAvailable: async () => false,
			openStagePane,
		});
		const { getByTestId } = render(<Probe />);
		await waitFor(() =>
			expect(getByTestId("pane")).toHaveAttribute("data-active", "no"),
		);
		expect(openStagePane).not.toHaveBeenCalled();
	});

	/** A browser has no window to draw underneath, and must not be asked. */
	it("never asks a bridge that is not a desktop", async () => {
		const stagePaneAvailable = vi.fn(async () => true);
		bridge.current = stubBridge({ available: false, stagePaneAvailable });
		render(<Probe />);
		await waitFor(() => expect(stagePaneAvailable).not.toHaveBeenCalled());
	});

	it("reports where the pane is and takes it back down again", async () => {
		const openStagePane = vi.fn(
			async (
				_paneId: string,
				_live3d: boolean,
				_geometry: StagePaneGeometry,
			) => undefined,
		);
		const closeStagePane = vi.fn(async () => undefined);
		bridge.current = stubBridge({ openStagePane, closeStagePane });
		const { getByTestId, unmount } = render(<Probe />);
		await waitFor(() =>
			expect(getByTestId("pane")).toHaveAttribute("data-active", "yes"),
		);
		expect(openStagePane).toHaveBeenCalledOnce();
		const geometry = openStagePane.mock.calls[0][2];
		expect(geometry).toMatchObject({
			x: expect.any(Number),
			y: expect.any(Number),
			width: expect.any(Number),
			height: expect.any(Number),
		});
		expect(geometry.scale).toBeGreaterThan(0);
		expect(geometry.surfaceWidth).toBeGreaterThan(0);

		unmount();
		await waitFor(() => expect(closeStagePane).toHaveBeenCalledOnce());
	});

	/** A renderer that will not start is shown rather than swallowed, and the Stage falls back. */
	it("keeps the web renderer and says why when the pane will not open", async () => {
		bridge.current = stubBridge({
			openStagePane: vi.fn(async () => {
				throw new Error("the visualizer is not beside this application");
			}),
		});
		const { getByTestId } = render(<Probe />);
		await waitFor(() =>
			expect(getByTestId("pane").getAttribute("data-trouble")).toContain(
				"not beside this application",
			),
		);
		expect(getByTestId("pane")).toHaveAttribute("data-active", "no");
	});
});
