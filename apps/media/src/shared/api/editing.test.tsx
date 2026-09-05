import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useVisualizerEditing } from "../../features/visualizers/editing";
import { aVisualizer } from "../../testing/server";
import { api } from "./client";
import { useEditing } from "./editing";

afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe("autosave survives navigation and object changes", () => {
	it("flushes the latest text or audio edit when its page unmounts", async () => {
		vi.useFakeTimers();
		const first = vi.fn(async () => undefined);
		const latest = vi.fn(async () => undefined);
		const { result, unmount } = renderHook(() => useEditing(vi.fn()));
		act(() => {
			result.current.saveLive(first);
			result.current.saveLive(latest);
		});
		await act(async () => unmount());
		expect(first).not.toHaveBeenCalled();
		expect(latest).toHaveBeenCalledOnce();
	});

	it("coalesces one text source without erasing another source's edit", async () => {
		vi.useFakeTimers();
		const first = vi.fn(async () => undefined);
		const second = vi.fn(async () => undefined);
		const latest = vi.fn(async () => undefined);
		const { result } = renderHook(() => useEditing(vi.fn()));
		act(() => {
			result.current.saveLive(first, "200/1");
			result.current.saveLive(second, "200/2");
			result.current.saveLive(latest, "200/1");
		});
		await act(async () => vi.advanceTimersByTimeAsync(180));
		expect(first).not.toHaveBeenCalled();
		expect(latest).toHaveBeenCalledOnce();
		expect(second).toHaveBeenCalledOnce();
	});

	it("saves both visualizers after changing selection and navigating away", async () => {
		vi.useFakeTimers();
		const first = aVisualizer();
		const second = { ...first, address: { ...first.address, file: 2 } };
		const save = vi.spyOn(api, "updateVisualizer").mockResolvedValue(first);
		const reload = vi.fn();
		const { result, unmount } = renderHook(() => useVisualizerEditing(reload));
		act(() => {
			result.current.saveLive(first, { requestId: "old", name: "Old" });
			result.current.saveLive(second, { requestId: "second", name: "Second" });
			result.current.saveLive(first, { requestId: "latest", name: "Latest" });
		});
		await act(async () => unmount());
		expect(save.mock.calls).toEqual([
			[250, 1, { requestId: "latest", name: "Latest" }],
			[250, 2, { requestId: "second", name: "Second" }],
		]);
		expect(reload).toHaveBeenCalledOnce();
	});

	it("retains a later edit while an earlier save is in flight during navigation", async () => {
		vi.useFakeTimers();
		let finish!: () => void;
		const first = vi.fn(
			() => new Promise<void>((resolve) => { finish = resolve; }),
		);
		const next = vi.fn(async () => undefined);
		const reload = vi.fn();
		const { result, unmount } = renderHook(() => useEditing(reload));
		act(() => result.current.saveLive(first, "200/1"));
		await act(async () => vi.advanceTimersByTimeAsync(180));
		act(() => result.current.saveLive(next, "200/2"));
		unmount();
		await act(async () => finish());
		expect(next).toHaveBeenCalledOnce();
		expect(reload).toHaveBeenCalledOnce();
	});
});
