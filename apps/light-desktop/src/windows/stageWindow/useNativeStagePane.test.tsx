// @vitest-environment jsdom

import "@testing-library/jest-dom/vitest";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
	DesktopBridge,
	StagePaneGeometry,
} from "../../platform/desktop/types";
import { useNativeStagePane } from "./useNativeStagePane";

const bridge = vi.hoisted(() => ({ current: null as DesktopBridge | null }));


const resizeCallbacks: Array<() => void> = [];
const intersectionCallbacks: Array<(entries: Partial<IntersectionObserverEntry>[]) => void> = [];
let bounds = { left: 0, top: 0, width: 800, height: 600 };
let visibility: ReturnType<typeof vi.spyOn>;
vi.stubGlobal("ResizeObserver", class {
	constructor(callback: () => void) { resizeCallbacks.push(callback); }
	observe() {} unobserve() {} disconnect() {}
});
vi.stubGlobal("IntersectionObserver", class {
	constructor(callback: (entries: Partial<IntersectionObserverEntry>[]) => void) { intersectionCallbacks.push(callback); }
	observe() {} unobserve() {} disconnect() {}
});
beforeEach(() => {
	bounds = { left: 0, top: 0, width: 800, height: 600 };
	resizeCallbacks.length = 0; intersectionCallbacks.length = 0;
	visibility = vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
	vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({
		...bounds, x: bounds.left, y: bounds.top, right: bounds.left + bounds.width,
		bottom: bounds.top + bounds.height, toJSON: () => ({}),
	}));
});
vi.mock("../../platform/desktop", () => ({
	useDesktopBridge: () => bridge.current,
}));

function stubBridge(overrides: Partial<DesktopBridge>): DesktopBridge {
	return {
		available: true,
		stagePaneAvailable: async () => true,
		openStagePane: vi.fn(async () => undefined),
		setStagePane: vi.fn(async () => undefined),
		setStagePaneVisibility: vi.fn(async () => undefined),
		closeStagePane: vi.fn(async () => undefined),
		sendStagePaneInput: vi.fn(async () => undefined),
		stagePaneStatus: async () => ["test renderer", null],
		onCurrentWindowMoved: async () => () => undefined,
		...overrides,
	} as unknown as DesktopBridge;
}

function Probe({ enabled = true }: { enabled?: boolean }) {
	const pane = useNativeStagePane(enabled);
	return (
		<div
			ref={pane.ref}
			data-testid="pane"
			data-active={pane.active ? "yes" : "no"}
			data-trouble={pane.trouble ?? ""}
		/>
	);
}

afterEach(() => {
	cleanup();
	bridge.current = null;
	vi.restoreAllMocks();
	vi.useRealTimers();
});

describe("useNativeStagePane", () => {

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
			async (_paneId: string, _live3d: boolean, _geometry: StagePaneGeometry) =>
				undefined,
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

	it("does not start an inactive pane and suspends its resident attachment when disabled", async () => {
		const openStagePane = vi.fn(async () => undefined);
		const closeStagePane = vi.fn(async () => undefined);
		bridge.current = stubBridge({ openStagePane, closeStagePane });
		const view = render(<Probe enabled={false} />);
		await waitFor(() =>
			expect(view.getByTestId("pane")).toHaveAttribute("data-active", "no"),
		);
		expect(openStagePane).not.toHaveBeenCalled();
		view.rerender(<Probe enabled />);
		await waitFor(() => expect(openStagePane).toHaveBeenCalledOnce());
		view.rerender(<Probe enabled={false} />);
		await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), false));
		expect(closeStagePane).not.toHaveBeenCalled();
		expect(view.getByTestId("pane")).toHaveAttribute("data-active", "yes");
		view.rerender(<Probe enabled />);
		await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), true));
		expect(openStagePane).toHaveBeenCalledOnce();
	});

	it("applies a hidden transition received during asynchronous open without closing the resident pane", async () => {
		let finishOpen: (() => void) | undefined;
		const openStagePane = vi.fn(
			() =>
				new Promise<void>((resolve) => {
					finishOpen = resolve;
				}),
		);
		const closeStagePane = vi.fn(async () => undefined);
		bridge.current = stubBridge({ openStagePane, closeStagePane });
		const view = render(<Probe enabled />);
		await waitFor(() => expect(openStagePane).toHaveBeenCalledOnce());
		view.rerender(<Probe enabled={false} />);
		await act(async () => { finishOpen?.(); });
		await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), false));
		expect(closeStagePane).not.toHaveBeenCalled();
		expect(view.getByTestId("pane")).toHaveAttribute("data-active", "yes");
		view.unmount();
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


it("suspends for document, intersection and collapsed bounds and resumes without reopening", async () => {
	bridge.current = stubBridge({});
	const view = render(<Probe />);
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), true));
	await act(async () => { visibility.mockReturnValue("hidden"); document.dispatchEvent(new Event("visibilitychange")); });
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), false));
	await act(async () => { visibility.mockReturnValue("visible"); document.dispatchEvent(new Event("visibilitychange")); });
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), true));
	const pane = view.getByTestId("pane");
	await act(async () => { intersectionCallbacks.forEach((callback) => callback([{ target: pane, isIntersecting: false, intersectionRatio: 0 }])); });
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), false));
	await act(async () => { intersectionCallbacks.forEach((callback) => callback([{ target: pane, isIntersecting: true, intersectionRatio: 1 }])); });
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), true));
	await act(async () => { bounds.height = 0; resizeCallbacks.forEach((callback) => callback()); });
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), false));
	expect(bridge.current!.setStagePane).not.toHaveBeenCalledWith(expect.anything(), expect.objectContaining({ height: 0 }));
	await act(async () => { bounds.height = 600; resizeCallbacks.forEach((callback) => callback()); });
	await waitFor(() => expect(bridge.current!.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), true));
	expect(bridge.current!.openStagePane).toHaveBeenCalledOnce();
	expect(bridge.current!.closeStagePane).not.toHaveBeenCalled();
});

it("does not suspend an unfocused visible window and stops observer updates after cleanup", async () => {
	bridge.current = stubBridge({});
	const desktop = bridge.current;
	const view = render(<Probe />);
	await waitFor(() => expect(desktop.setStagePaneVisibility).toHaveBeenLastCalledWith(expect.any(String), true));
	const before = vi.mocked(desktop.setStagePaneVisibility).mock.calls.length;
	await act(async () => { window.dispatchEvent(new Event("blur")); window.dispatchEvent(new Event("focus")); });
	expect(desktop.setStagePaneVisibility).toHaveBeenCalledTimes(before);
	view.unmount();
	await waitFor(() => expect(desktop.closeStagePane).toHaveBeenCalledOnce());
	await act(async () => { resizeCallbacks.forEach((callback) => callback()); document.dispatchEvent(new Event("visibilitychange")); });
	expect(desktop.setStagePaneVisibility).toHaveBeenCalledTimes(before);
});

it("waits for a delayed open to settle before closing an unmounted attachment", async () => {
	let finishOpen!: () => void;
	const open = vi.fn(() => new Promise<void>((resolve) => { finishOpen = resolve; }));
	bridge.current = stubBridge({ openStagePane: open });
	const desktop = bridge.current;
	const view = render(<Probe />);
	await waitFor(() => expect(open).toHaveBeenCalledOnce());
	view.unmount();
	expect(desktop.closeStagePane).not.toHaveBeenCalled();
	await act(async () => { finishOpen(); });
	await waitFor(() => expect(desktop.closeStagePane).toHaveBeenCalledOnce());
	expect(desktop.setStagePaneVisibility).not.toHaveBeenCalled();
});


it("restarts loss before the first status witness only after the asynchronous open completed", async () => {
	vi.useFakeTimers();
	let finishOpen!: () => void;
	const open = vi.fn(async () => {});
	open.mockImplementationOnce(() => new Promise<void>((resolve) => { finishOpen = resolve; }));
	const status = vi.fn<DesktopBridge["stagePaneStatus"]>().mockResolvedValue(["recovered renderer", null]);
	status.mockResolvedValueOnce([null, "renderer died before its first status"]);
	bridge.current = stubBridge({ openStagePane: open, stagePaneStatus: status });
	const view = render(<Probe />);
	await act(async () => { await vi.advanceTimersByTimeAsync(0); });
	expect(open).toHaveBeenCalledOnce();
	await act(async () => { await vi.advanceTimersByTimeAsync(5_000); });
	expect(status).not.toHaveBeenCalled();
	expect(open).toHaveBeenCalledOnce();
	await act(async () => { finishOpen(); });
	expect(status).toHaveBeenCalledOnce();
	await act(async () => { await vi.advanceTimersByTimeAsync(249); });
	expect(open).toHaveBeenCalledOnce();
	await act(async () => { await vi.advanceTimersByTimeAsync(1); });
	expect(open).toHaveBeenCalledTimes(2);
	expect(bridge.current!.closeStagePane).toHaveBeenCalledOnce();
	expect(view.getByTestId("pane")).toHaveAttribute("data-active", "yes");
});

it("backs off repeated post-open losses and stops after five automatic restarts", async () => {
	vi.useFakeTimers();
	const open = vi.fn(async () => {});
	bridge.current = stubBridge({ openStagePane: open, stagePaneStatus: async () => [null, null] });
	const view = render(<Probe />);
	await act(async () => { await vi.advanceTimersByTimeAsync(0); });
	expect(open).toHaveBeenCalledOnce();
	for (const [index, delay] of [250, 500, 1_000, 2_000, 4_000].entries()) {
		await act(async () => { await vi.advanceTimersByTimeAsync(delay); });
		expect(open).toHaveBeenCalledTimes(index + 2);
	}
	expect(open).toHaveBeenCalledTimes(6);
	expect(view.getByTestId("pane").getAttribute("data-trouble")).toContain("reopen the Stage");
	await act(async () => { await vi.advanceTimersByTimeAsync(60_000); });
	expect(open).toHaveBeenCalledTimes(6);
});
