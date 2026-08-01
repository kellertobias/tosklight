import { renderHook, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { describe, expect, it, vi } from "vitest";
import type { VisualizerView } from "../../../api/client/visualizerView";
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
			views: vi.fn(async () => [
				view("main", "top_down"),
				view("front-of-house", "simple_3d"),
			]),
			update: vi.fn(),
		};
		const { result } = harness(actions);

		await waitFor(() => expect(result.current.view).not.toBeNull());
		expect(result.current.view?.mode).toBe("top_down");
		expect(result.current.targets).toEqual(["main", "front-of-house"]);
	});

	/// What is displayed is what the desk accepted, not what was asked for.
	it("shows the authoritative answer to an edit", async () => {
		const actions: VisualizerViewActions = {
			views: vi.fn(async () => [view("main", "top_down")]),
			update: vi.fn(async () => ({
				...view("main", "lines_3d"),
				revision: 2,
			})),
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
			views: vi.fn(async () => [view("main", "top_down")]),
			update: vi.fn(async () => {
				throw new Error("exposure must be within 0.05-4.0");
			}),
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

	it("is unavailable outside a mounted desk boundary", () => {
		const { result } = renderHook(() => useVisualizerViewControls(true));
		expect(result.current.available).toBe(false);
		expect(result.current.view).toBeNull();
	});
});
