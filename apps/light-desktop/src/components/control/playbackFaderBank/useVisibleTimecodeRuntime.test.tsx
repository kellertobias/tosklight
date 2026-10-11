import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { TimecodeTransportSnapshot } from "../../../api/types/timecode";
import { useVisibleTimecodeRuntime } from "./useVisibleTimecodeRuntime";

const mocks = vi.hoisted(() => ({ runtime: vi.fn(), listen: vi.fn(), present: true }));
vi.mock("../../../features/timecode/TimecodeActionsContext", () => ({
	useTimecodeActions: () => mocks.present ? authority : null,
}));
const authority = { api: { runtime: mocks.runtime }, events: { onRuntimeChanged: mocks.listen } };
const snapshot = (id: string, revision: number, state: TimecodeTransportSnapshot["state"]): TimecodeTransportSnapshot => ({
	timecode_id: id, revision, state, frame: 15, duration_frame: 2640,
	audio_linked: false, cue_list_clips: [],
});
beforeEach(() => {
	mocks.present = true;
	mocks.runtime.mockReset();
	mocks.listen.mockReset();
});
afterEach(cleanup);

it("keeps a newer event over late hydration and filters invisible transports", async () => {
	let resolve!: (values: TimecodeTransportSnapshot[]) => void;
	mocks.runtime.mockReturnValue(new Promise<TimecodeTransportSnapshot[]>((done) => { resolve = done; }));
	let listener!: (value: TimecodeTransportSnapshot) => void;
	const unsubscribe = vi.fn();
	mocks.listen.mockImplementation((callback) => { listener = callback; return unsubscribe; });
	const { result, rerender, unmount } = renderHook(() => useVisibleTimecodeRuntime("show", ["visible", "visible"]));
	act(() => listener(snapshot("visible", 8, "playing")));
	await act(async () => resolve([snapshot("visible", 7, "stopped"), snapshot("hidden", 9, "playing")]));
	expect(result.current.snapshots.get("visible")?.state).toBe("playing");
	expect(result.current.snapshots.has("hidden")).toBe(false);
	rerender();
	expect(mocks.runtime).toHaveBeenCalledTimes(1);
	unmount();
	expect(unsubscribe).toHaveBeenCalledTimes(1);
});

it("discards old-show hydration and events after the bank changes", async () => {
	let resolve!: (values: TimecodeTransportSnapshot[]) => void;
	mocks.runtime.mockImplementationOnce(() => new Promise<TimecodeTransportSnapshot[]>((done) => { resolve = done; }));
	mocks.runtime.mockResolvedValueOnce([snapshot("new", 2, "paused")]);
	const listeners: ((value: TimecodeTransportSnapshot) => void)[] = [];
	const unsubscribe = vi.fn();
	mocks.listen.mockImplementation((callback) => { listeners.push(callback); return unsubscribe; });
	const { result, rerender } = renderHook(({ show, ids }) => useVisibleTimecodeRuntime(show, ids), {
		initialProps: { show: "old-show", ids: ["old"] },
	});
	rerender({ show: "new-show", ids: ["new"] });
	await waitFor(() => expect(result.current.snapshots.get("new")?.state).toBe("paused"));
	await act(async () => { resolve([snapshot("old", 99, "playing")]); listeners[0](snapshot("old", 100, "playing")); });
	expect([...result.current.snapshots.keys()]).toEqual(["new"]);
	expect(unsubscribe).toHaveBeenCalledTimes(1);
});

it("safely skips unused families and reports missing authority without guessing stopped", () => {
	mocks.present = false;
	const { result, rerender } = renderHook(({ ids }) => useVisibleTimecodeRuntime("show", ids), {
		initialProps: { ids: [] as string[] },
	});
	expect(result.current.error).toBeNull();
	rerender({ ids: ["visible"] });
	expect(result.current.error).toBe("Timecode authority unavailable");
	expect(result.current.snapshots.size).toBe(0);
	expect(mocks.runtime).not.toHaveBeenCalled();
});


it("keeps proven event authority healthy if initial hydration later fails", async () => {
	let reject!: (error: Error) => void;
	mocks.runtime.mockReturnValue(new Promise((_, fail) => { reject = fail; }));
	let listener!: (value: TimecodeTransportSnapshot) => void;
	mocks.listen.mockImplementation((callback) => { listener = callback; return vi.fn(); });
	const { result } = renderHook(() => useVisibleTimecodeRuntime("show", ["visible"]));
	act(() => listener(snapshot("visible", 8, "playing")));
	await act(async () => reject(new Error("Initial hydration unavailable")));
	expect(result.current.snapshots.get("visible")?.state).toBe("playing");
	expect(result.current.error).toBeNull();
});
