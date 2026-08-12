import { act, renderHook, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { describe, expect, it, vi } from "vitest";
import type {
	VisualizerView,
	VisualizerViewSnapshot,
} from "../../../api/client/visualizerView";
import {
	type VisualizerViewActions,
	VisualizerViewProvider,
} from "../../../features/visualizerView/VisualizerViewContext";
import { useVisualizerViewControls } from "./useVisualizerViewControls";

function view(target: string, mode: VisualizerView["mode"]): VisualizerView {
	return {
		target,
		mode,
		quality: "high",
		exposure: 1,
		ambient: 0.06,
		revision: 1,
	};
}

function harness(actions: VisualizerViewActions) {
	return renderHook(() => useVisualizerViewControls(true), {
		wrapper: ({ children }: PropsWithChildren) => (
			<VisualizerViewProvider actions={actions}>
				{children}
			</VisualizerViewProvider>
		),
	});
}

describe("useVisualizerViewControls", () => {
	it("reads the desk's views and follows the addressed renderer", async () => {
		const actions: VisualizerViewActions = {
			snapshot: vi.fn(async () => ({
				connected: true,
				views: [view("main", "top_down"), view("front-of-house", "simple_3d")],
			})),
			update: vi.fn(),
			onConnectionChanged: vi.fn(() => vi.fn()),
		};
		const { result } = harness(actions);

		await waitFor(() => expect(result.current.view).not.toBeNull());
		expect(result.current.view?.mode).toBe("top_down");
		expect(result.current.targets).toEqual(["main", "front-of-house"]);
		expect(result.current.connected).toBe(true);
	});

	it("tracks connection events without polling the view snapshot", async () => {
		let reportConnection = (_connected: boolean) => {};
		let resolveSnapshot!: (snapshot: VisualizerViewSnapshot) => void;
		const actions: VisualizerViewActions = {
			snapshot: vi.fn(
				() =>
					new Promise<VisualizerViewSnapshot>((resolve) => {
						resolveSnapshot = resolve;
					}),
			),
			update: vi.fn(),
			onConnectionChanged: vi.fn((listener) => {
				reportConnection = listener;
				return vi.fn();
			}),
		};
		const { result } = harness(actions);
		await waitFor(() => expect(actions.onConnectionChanged).toHaveBeenCalled());

		act(() => reportConnection(true));
		resolveSnapshot({
			connected: false,
			views: [view("main", "top_down")],
		});
		await waitFor(() => expect(result.current.connected).toBe(true));
		expect(result.current.view).not.toBeNull();
		expect(actions.snapshot).toHaveBeenCalledOnce();
	});

	/// What is displayed is what the desk accepted, not what was asked for.
	it("shows the authoritative answer to an edit", async () => {
		const actions: VisualizerViewActions = {
			snapshot: vi.fn(async () => ({
				connected: true,
				views: [view("main", "top_down")],
			})),
			update: vi.fn(async () => ({
				...view("main", "lines_3d"),
				revision: 2,
			})),
			onConnectionChanged: vi.fn(() => vi.fn()),
		};
		const { result } = harness(actions);
		await waitFor(() => expect(result.current.view).not.toBeNull());

		result.current.selectMode("lines_3d");
		await waitFor(() => expect(result.current.view?.mode).toBe("lines_3d"));
		expect(result.current.view?.revision).toBe(2);
		expect(actions.update).toHaveBeenCalledWith("main", {
			mode: "lines_3d",
		});
		expect(result.current.error).toBeNull();
	});

	it("says what the desk refused instead of showing the edit as applied", async () => {
		const actions: VisualizerViewActions = {
			snapshot: vi.fn(async () => ({
				connected: true,
				views: [view("main", "top_down")],
			})),
			update: vi.fn(async () => {
				throw new Error("exposure must be within 0.05-4.0");
			}),
			onConnectionChanged: vi.fn(() => vi.fn()),
		};
		const { result } = harness(actions);
		await waitFor(() => expect(result.current.view).not.toBeNull());

		result.current.selectQuality("ultra");
		await waitFor(() =>
			expect(result.current.error).toBe("exposure must be within 0.05-4.0"),
		);
		expect(result.current.view?.mode).toBe("top_down");
		expect(result.current.busy).toBe(false);
	});

	it("reports no connected visualizer outside a mounted desk boundary", () => {
		const { result } = renderHook(() => useVisualizerViewControls(true));
		expect(result.current.connected).toBe(false);
		expect(result.current.view).toBeNull();
	});
});
