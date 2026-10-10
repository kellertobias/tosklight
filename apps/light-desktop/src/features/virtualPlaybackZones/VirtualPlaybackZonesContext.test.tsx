import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
	VirtualPlaybackZone,
	VirtualPlaybackZonesCapability,
	VirtualPlaybackZonesEventObserver,
	VirtualPlaybackZonesSnapshot,
	VirtualPlaybackZonesTransport,
} from "./contracts";
import {
	useVirtualPlaybackZones,
	VirtualPlaybackZonesProvider,
	VirtualPlaybackZonesController,
} from "./VirtualPlaybackZonesContext";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const AUTHORITY = {
	authorityId: "authority-a",
	scope: { showId: SHOW_ID },
};
const ZONES = [
	{ id: "paired", name: "Paired", playbackNumbers: [1001, 1301] },
] as const;
const UPDATED = [
	{ id: "paired", name: "Updated", playbackNumbers: [1001, 1301, 1601] },
] as const;

function snapshot(
	zones: readonly VirtualPlaybackZone[] = ZONES,
	revision = 4,
): VirtualPlaybackZonesSnapshot {
	return { showId: SHOW_ID, revision, zones };
}

function harness(transport: VirtualPlaybackZonesTransport) {
	const current = { capability: null as VirtualPlaybackZonesCapability | null };
	function Probe() {
		current.capability = useVirtualPlaybackZones();
		return null;
	}
	render(
		<VirtualPlaybackZonesProvider authority={AUTHORITY} transport={transport}>
			<Probe />
		</VirtualPlaybackZonesProvider>,
	);
	return current;
}

afterEach(() => {
	cleanup();
	vi.useRealTimers();
});

describe("Virtual Playback zone event recovery", () => {
	function eventHarness() {
		const observers: VirtualPlaybackZonesEventObserver[] = [];
		const close = vi.fn();
		const report = vi.fn();
		const loadSnapshot = vi.fn(async () => snapshot(UPDATED, 5));
		let current = true;
		const subscribe = vi.fn(
			(_scope, observer: VirtualPlaybackZonesEventObserver) => {
				observers.push(observer);
				return { close };
			},
		);
		const controller = new VirtualPlaybackZonesController(
			AUTHORITY.scope,
			{ loadSnapshot, save: vi.fn(), subscribe },
			() => current,
			report,
		);
		return {
			controller,
			observers,
			close,
			report,
			loadSnapshot,
			subscribe,
			replaceAuthority: () => {
				current = false;
			},
		};
	}

	it("reconnects and repairs missed zones before retiring the local connection error", async () => {
		vi.useFakeTimers();
		const test = eventHarness();
		const deactivate = test.controller.activate();
		test.observers[0].error(new Error("socket failed"));
		await vi.advanceTimersByTimeAsync(250);
		expect(test.subscribe).toHaveBeenCalledTimes(2);
		test.observers[0].closed(); // late callback from the replaced stream
		test.observers[1].ready?.();
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();
		expect(test.controller.getZones()).toEqual(UPDATED);
		expect(test.report).toHaveBeenLastCalledWith(null);
		test.loadSnapshot.mockResolvedValueOnce(snapshot(ZONES, 6));
		test.observers[1].changed({ showId: SHOW_ID, revision: 6 });
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();
		expect(test.controller.getZones()).toEqual(ZONES);
		deactivate();
	});

	it("cancels retry and ignores stale callbacks when the pane deactivates", async () => {
		vi.useFakeTimers();
		const test = eventHarness();
		const deactivate = test.controller.activate();
		test.observers[0].closed();
		deactivate();
		test.report.mockClear();
		test.observers[0].error(new Error("late teardown error"));
		await vi.advanceTimersByTimeAsync(10_000);
		expect(test.subscribe).toHaveBeenCalledOnce();
		expect(test.report).not.toHaveBeenCalled();
	});

	it("stops retries after five failed attempts and offers explicit pane retry", async () => {
		vi.useFakeTimers();
		const test = eventHarness();
		const deactivate = test.controller.activate();
		for (const delay of [250, 500, 1000, 2000, 4000]) {
			test.observers.at(-1)?.closed();
			await vi.advanceTimersByTimeAsync(delay);
		}
		test.observers.at(-1)?.closed();
		await vi.advanceTimersByTimeAsync(20_000);
		expect(test.subscribe).toHaveBeenCalledTimes(6);
		expect(test.report.mock.lastCall?.[0]?.message).toContain(
			"Reopen the pane",
		);
		deactivate();
	});

	it("does not reconnect after authenticated authority replacement", async () => {
		vi.useFakeTimers();
		const test = eventHarness();
		const deactivate = test.controller.activate();
		test.observers[0].closed();
		test.replaceAuthority();
		await vi.advanceTimersByTimeAsync(10_000);
		expect(test.subscribe).toHaveBeenCalledOnce();
		deactivate();
	});

	it("drops an in-flight repair after the pane deactivates", async () => {
		const test = eventHarness();
		let resolve!: (value: VirtualPlaybackZonesSnapshot) => void;
		test.loadSnapshot.mockReturnValueOnce(
			new Promise((done) => {
				resolve = done;
			}),
		);
		const deactivate = test.controller.activate();
		test.observers[0].ready?.();
		deactivate();
		test.report.mockClear();
		resolve(snapshot(UPDATED, 5));
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();
		expect(test.controller.getZones()).toBeNull();
		expect(test.report).not.toHaveBeenCalled();
	});

	it("does not hide an outstanding event failure behind a successful snapshot read", async () => {
		vi.useFakeTimers();
		const test = eventHarness();
		const deactivate = test.controller.activate();
		const failure = new Error("socket failed");
		test.observers[0].error(failure);
		await test.controller.load();
		expect(test.report).toHaveBeenLastCalledWith(failure);
		deactivate();
	});
});

describe("VirtualPlaybackZonesProvider", () => {
	it("is dormant until explicitly loaded and coalesces reads", async () => {
		const loadSnapshot = vi.fn(async () => snapshot());
		const current = harness({ loadSnapshot, save: vi.fn() });
		expect(loadSnapshot).not.toHaveBeenCalled();
		await act(async () => {
			await Promise.all([
				current.capability?.load(),
				current.capability?.load(),
			]);
		});
		expect(loadSnapshot).toHaveBeenCalledOnce();
		expect(current.capability?.getZones()).toEqual(ZONES);
	});

	it("saves against the one show-level revision and installs the result", async () => {
		const save = vi.fn(async () => ({
			...snapshot(UPDATED, 5),
			requestId: "request-a",
			replayed: false,
			changed: true,
		}));
		const current = harness({
			loadSnapshot: vi.fn(async () => snapshot()),
			save,
		});
		await act(async () => {
			await current.capability?.load();
			await current.capability?.save(UPDATED);
		});
		expect(save).toHaveBeenCalledWith(
			{ showId: SHOW_ID },
			4,
			UPDATED,
			expect.any(String),
		);
		expect(current.capability?.getZones()).toEqual(UPDATED);
	});

	it("serializes edits so the second uses the first result revision", async () => {
		const save = vi
			.fn<VirtualPlaybackZonesTransport["save"]>()
			.mockResolvedValueOnce({
				...snapshot(UPDATED, 5),
				requestId: "one",
				replayed: false,
				changed: true,
			})
			.mockResolvedValueOnce({
				...snapshot(ZONES, 6),
				requestId: "two",
				replayed: false,
				changed: true,
			});
		const current = harness({
			loadSnapshot: vi.fn(async () => snapshot()),
			save,
		});
		await act(async () => {
			await Promise.all([
				current.capability?.save(UPDATED),
				current.capability?.save(ZONES),
			]);
		});
		expect(save).toHaveBeenCalledTimes(2);
		expect(save.mock.calls[0][1]).toBe(4);
		expect(save.mock.calls[1][1]).toBe(5);
	});

	it("reloads the shared snapshot when another desk publishes a revision", async () => {
		let observer: VirtualPlaybackZonesEventObserver | null = null;
		const loadSnapshot = vi
			.fn<() => Promise<VirtualPlaybackZonesSnapshot>>()
			.mockResolvedValueOnce(snapshot())
			.mockResolvedValueOnce(snapshot(UPDATED, 5));
		const current = harness({
			loadSnapshot,
			save: vi.fn(),
			subscribe: (_scope, next) => {
				observer = next;
				return { close: vi.fn() };
			},
		});
		await act(async () => {
			current.capability?.activate();
			await current.capability?.load();
		});
		act(() => observer?.changed({ showId: SHOW_ID, revision: 5 }));
		await waitFor(() =>
			expect(current.capability?.getZones()).toEqual(UPDATED),
		);
		expect(loadSnapshot).toHaveBeenCalledTimes(2);
	});

	it("clears cached zones when the authority changes", async () => {
		const transport = {
			loadSnapshot: vi.fn(async () => snapshot()),
			save: vi.fn(),
		};
		const current = {
			capability: null as VirtualPlaybackZonesCapability | null,
		};
		function Probe() {
			current.capability = useVirtualPlaybackZones();
			return null;
		}
		const view = render(
			<VirtualPlaybackZonesProvider authority={AUTHORITY} transport={transport}>
				<Probe />
			</VirtualPlaybackZonesProvider>,
		);
		await act(async () => void (await current.capability?.load()));
		view.rerender(
			<VirtualPlaybackZonesProvider
				authority={{
					authorityId: "authority-b",
					scope: { showId: "33333333-3333-4333-8333-333333333333" },
				}}
				transport={transport}
			>
				<Probe />
			</VirtualPlaybackZonesProvider>,
		);
		expect(current.capability?.getZones()).toBeNull();
	});
});
