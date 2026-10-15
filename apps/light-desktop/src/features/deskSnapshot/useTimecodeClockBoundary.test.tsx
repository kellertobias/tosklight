import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { BootstrapSnapshot, SessionResponse } from "../../api/types";
import type { ServerState } from "../server/useServerState";
import { DeskSnapshotStore } from "./store";
import { useTimecodeClockBoundary } from "./useTimecodeClockBoundary";

afterEach(cleanup);
const snapshot = {
	revision: 2,
	selected_source: "artnet:127.0.0.1:2",
	active_source: "artnet:127.0.0.1:2",
	state: "locked",
	current: "00:00:02:22",
	source_rate: "25",
	desk_rate: "44",
	rate_warning: null,
};
function deferred() {
	let resolve!: (value: typeof snapshot) => void;
	const promise = new Promise<typeof snapshot>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

it("hydrates once, preserves a newer stream event and never reloads bootstrap", async () => {
	const pending = deferred();
	const store = new DeskSnapshotStore();
	const session = { session_id: "session-a" } as SessionResponse;
	store.install({ active_timecode: null } as BootstrapSnapshot, session);
	const read = vi.fn(() => pending.promise);
	const bootstrap = vi.fn();
	const state = {
		api: { runtime: { timecodeClock: read, bootstrap } },
		deskSnapshotStore: store,
		session,
		status: "connected",
		connectionGeneration: 1,
	} as unknown as ServerState;
	const mounted = renderHook(() => useTimecodeClockBoundary(state));
	await act(async () => {
		await Promise.resolve();
	});
	expect(read).toHaveBeenCalledTimes(1);
	act(() =>
		store.installClock({ ...snapshot, revision: 3, current: "00:00:03:00" }),
	);
	await act(async () => pending.resolve(snapshot));
	expect(store.getSnapshot().clock?.current).toBe("00:00:03:00");
	mounted.rerender();
	expect(read).toHaveBeenCalledTimes(1);
	expect(bootstrap).not.toHaveBeenCalled();
});

it("retires an old connection read after reconnect even if it resolves last", async () => {
	const first = deferred();
	const second = deferred();
	const store = new DeskSnapshotStore();
	const read = vi
		.fn()
		.mockReturnValueOnce(first.promise)
		.mockReturnValueOnce(second.promise);
	const api = { runtime: { timecodeClock: read } };
	const session = { session_id: "session-a" } as SessionResponse;
	const state = {
		api,
		deskSnapshotStore: store,
		session,
		status: "connected",
		connectionGeneration: 1,
	} as unknown as ServerState;
	const mounted = renderHook(({ value }) => useTimecodeClockBoundary(value), {
		initialProps: { value: state },
	});
	await act(async () => {
		await Promise.resolve();
	});
	mounted.rerender({ value: { ...state, connectionGeneration: 2 } });
	await act(async () => {
		await Promise.resolve();
		second.resolve({ ...snapshot, revision: 1, current: "00:00:04:00" });
	});
	await act(async () => first.resolve({ ...snapshot, revision: 100 }));
	expect(store.getSnapshot().clock?.current).toBe("00:00:04:00");
	expect(read).toHaveBeenCalledTimes(2);
});

it("exposes a failed first read without polling or reviving bootstrap authority", async () => {
	const store = new DeskSnapshotStore();
	const read = vi.fn().mockRejectedValue(new Error("offline"));
	const session = { session_id: "session-a" } as SessionResponse;
	store.install(
		{ active_timecode: "00:00:01:00" } as BootstrapSnapshot,
		session,
	);
	const state = {
		api: { runtime: { timecodeClock: read } },
		deskSnapshotStore: store,
		session,
		status: "connected",
		connectionGeneration: 1,
	} as unknown as ServerState;
	renderHook(() => useTimecodeClockBoundary(state));
	await act(async () => {
		await Promise.resolve();
	});
	expect(store.getSnapshot()).toMatchObject({
		clock: null,
		clockReady: true,
		clockError: "Timecode status unavailable",
	});
	expect(read).toHaveBeenCalledTimes(1);
});

it("does not let a failed read erase a newer valid pushed clock", async () => {
	let reject!: (reason: Error) => void;
	const read = vi.fn(
		() =>
			new Promise((_resolve, no) => {
				reject = no;
			}),
	);
	const store = new DeskSnapshotStore();
	const state = {
		api: { runtime: { timecodeClock: read } },
		deskSnapshotStore: store,
		session: { session_id: "a" },
		status: "connected",
		connectionGeneration: 1,
	} as unknown as ServerState;
	renderHook(() => useTimecodeClockBoundary(state));
	await act(async () => {
		await Promise.resolve();
		store.installClock(snapshot);
		reject(new Error("offline"));
	});
	expect(store.getSnapshot().clock?.current).toBe(snapshot.current);
	expect(store.getSnapshot()).not.toHaveProperty(
		"clockError",
		"Timecode status unavailable",
	);
});
