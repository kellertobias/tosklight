import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
	type RunningDynamicsApi,
	type RunningDynamicsEvent,
	type RunningDynamicsEventSource,
	type RunningDynamicsSnapshot,
	useRunningDynamicsAuthority,
} from "./runningDynamicsAuthority";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";

class Events implements RunningDynamicsEventSource {
	listeners = new Set<(event: RunningDynamicsEvent) => void>();
	unsubscribes: ReturnType<typeof vi.fn>[] = [];

	onEvent(listener: (event: RunningDynamicsEvent) => void) {
		this.listeners.add(listener);
		const unsubscribe = vi.fn(() => this.listeners.delete(listener));
		this.unsubscribes.push(unsubscribe);
		return unsubscribe;
	}

	emit(event: RunningDynamicsEvent) {
		for (const listener of this.listeners) listener(event);
	}
}

function runtime(controllerIds = ["controller-a", "controller-b"]) {
	return {
		global_paused: false,
		definitions: [],
		instances: [
			{
				instance_id: "instance-a",
				dynamic_id: "dynamic-a",
				pool_number: 17,
				name: "Circle",
				targets: ["fixture-a", "fixture-b"],
				pending: false,
				pending_until_millis: null,
				paused: false,
				speed_source: "Speed Group A",
				activation_boundary: "beat",
				effective_cycle_millis: 2_000n,
				effective_bpm: 120,
				beat_phase: 0.25,
				phase_advancing: true,
				aliasing_warning: null,
				controllers: controllerIds.map((controllerId, index) => ({
					controller_id: controllerId,
					source: index === 0 ? "Programmer" : "Playback 12",
					priority: 10 - index,
					size: 1,
					speed_multiplier: 1,
					phase_offset_degrees: 0,
					paused: false,
					winning: index === 0,
					releasing: false,
					activation_mix: 1,
				})),
			},
		],
	} satisfies RunningDynamicsSnapshot;
}

function api(initial = runtime()) {
	return {
		runtime: vi.fn(async () => initial),
		offLive: vi.fn(async (controllerId: string) => ({
			request_id: "request",
			runtime_instance_id: "instance-a",
			controller_id: controllerId,
			targets: [],
			started: false,
		})),
	} satisfies RunningDynamicsApi;
}

afterEach(cleanup);

describe("useRunningDynamicsAuthority", () => {
	it("stays dormant while disabled and unsubscribes when hidden", async () => {
		const backend = api();
		const events = new Events();
		const rendered = renderHook(
			({ enabled }) =>
				useRunningDynamicsAuthority(enabled, SHOW_ID, backend, events),
			{ initialProps: { enabled: false } },
		);

		expect(backend.runtime).not.toHaveBeenCalled();
		expect(events.listeners).toHaveLength(0);
		rendered.rerender({ enabled: true });
		await waitFor(() => expect(rendered.result.current.ready).toBe(true));
		expect(backend.runtime).toHaveBeenCalledOnce();
		expect(events.listeners).toHaveLength(1);

		rendered.rerender({ enabled: false });
		expect(events.unsubscribes[0]).toHaveBeenCalledOnce();
		expect(rendered.result.current.rows).toEqual([]);
	});

	it("projects one truthful row per exact runtime controller", async () => {
		const backend = api();
		const events = new Events();
		const rendered = renderHook(() =>
			useRunningDynamicsAuthority(true, SHOW_ID, backend, events),
		);

		await waitFor(() => expect(rendered.result.current.ready).toBe(true));
		expect(rendered.result.current.rows).toMatchObject([
			{
				key: "instance-a:controller-a",
				instanceId: "instance-a",
				controllerId: "controller-a",
				name: "Circle",
				poolNumber: 17,
				source: "Programmer",
				winning: true,
			},
			{
				key: "instance-a:controller-b",
				controllerId: "controller-b",
				source: "Playback 12",
				winning: false,
			},
		]);
	});

	it("repairs only after Dynamic runtime push invalidations", async () => {
		const backend = api();
		const events = new Events();
		const rendered = renderHook(() =>
			useRunningDynamicsAuthority(true, SHOW_ID, backend, events),
		);
		await waitFor(() => expect(rendered.result.current.ready).toBe(true));

		act(() => {
			events.emit({
				type: "hardware_connection_changed",
				change: { revision: 2, connected: true },
			});
		});
		expect(backend.runtime).toHaveBeenCalledOnce();

		backend.runtime.mockResolvedValue(runtime(["controller-c"]));
		act(() => {
			events.emit({
				type: "dynamic_runtime_changed",
				change: {
					kind: "controller_updated",
					dynamic_id: "dynamic-a",
					runtime_instance_id: "instance-a",
					controller_id: "controller-c",
					winning_controller_id: "controller-c",
					occurred_at_millis: 10,
					message: null,
				},
			});
		});
		await waitFor(() =>
			expect(rendered.result.current.rows[0]?.controllerId).toBe(
				"controller-c",
			),
		);
		expect(backend.runtime).toHaveBeenCalledTimes(2);
	});

	it("sends Off for the exact controller and refreshes authority", async () => {
		const backend = api();
		const events = new Events();
		const rendered = renderHook(() =>
			useRunningDynamicsAuthority(true, SHOW_ID, backend, events),
		);
		await waitFor(() => expect(rendered.result.current.ready).toBe(true));
		const row = rendered.result.current.rows[1];
		if (!row) throw new Error("expected second controller");

		await act(() => rendered.result.current.off(row));

		expect(backend.offLive).toHaveBeenCalledOnce();
		expect(backend.offLive).toHaveBeenCalledWith("controller-b");
		expect(backend.runtime).toHaveBeenCalledTimes(2);
		expect(rendered.result.current.stoppingControllerIds.size).toBe(0);
	});

	it("surfaces an Off failure and clears the pending controller", async () => {
		const backend = api();
		const events = new Events();
		backend.offLive.mockRejectedValueOnce(new Error("controller rejected"));
		const rendered = renderHook(() =>
			useRunningDynamicsAuthority(true, SHOW_ID, backend, events),
		);
		await waitFor(() => expect(rendered.result.current.ready).toBe(true));
		const row = rendered.result.current.rows[0];
		if (!row) throw new Error("expected controller");

		let changed = true;
		await act(async () => {
			changed = await rendered.result.current.off(row);
		});

		expect(changed).toBe(false);
		expect(rendered.result.current.error).toBe("controller rejected");
		expect(rendered.result.current.stoppingControllerIds.size).toBe(0);
	});
});
