import { describe, expect, it, vi } from "vitest";
import {
	FrontendWarmupCoordinator,
	type FrontendWarmupTask,
} from "./coordinator";

function deferred() {
	let resolve!: () => void;
	const promise = new Promise<void>((next) => {
		resolve = next;
	});
	return { promise, resolve };
}

async function settle() {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

describe("FrontendWarmupCoordinator", () => {
	it("runs priority tasks with bounded concurrency and yields between starts", async () => {
		const gates = [deferred(), deferred(), deferred()];
		const starts: string[] = [];
		const yields: number[] = [];
		let clock = 0;
		const coordinator = new FrontendWarmupCoordinator({
			concurrency: 2,
			now: () => ++clock,
			yieldToMain: async () => {
				yields.push(clock);
			},
		});
		const task = (
			key: string,
			priority: FrontendWarmupTask["priority"],
			gate: (typeof gates)[number],
		): FrontendWarmupTask => ({
			key,
			priority,
			run: async () => {
				starts.push(key);
				await gate.promise;
			},
		});
		coordinator.enqueue(task("idle", "idle", gates[2]));
		coordinator.enqueue(task("foreground", "foreground", gates[0]));
		coordinator.enqueue(task("near", "near-future", gates[1]));
		coordinator.start();
		await settle();

		expect(starts).toEqual(["foreground", "near"]);
		expect(coordinator.getDiagnostics().active).toBe(2);
		expect(yields).toHaveLength(2);

		gates[0].resolve();
		await settle();
		expect(starts).toEqual(["foreground", "near", "idle"]);
		gates[1].resolve();
		gates[2].resolve();
		await settle();
		expect(coordinator.getDiagnostics().status).toBe("ready");
	});

	it("deduplicates keys, holds completed leases, and releases them on cancellation", async () => {
		const release = vi.fn();
		const coordinator = new FrontendWarmupCoordinator();
		expect(
			coordinator.enqueue({
				key: "groups",
				priority: "near-future",
				run: async () => ({ release, retainedBytes: 128 }),
			}),
		).toBe(true);
		expect(
			coordinator.enqueue({
				key: "groups",
				priority: "foreground",
				run: async () => undefined,
			}),
		).toBe(false);
		coordinator.start();
		await settle();

		expect(coordinator.getDiagnostics()).toMatchObject({
			status: "ready",
			retainedBytes: 128,
		});
		expect(release).not.toHaveBeenCalled();
		coordinator.cancel();
		expect(release).toHaveBeenCalledOnce();
		expect(coordinator.getDiagnostics().status).toBe("cancelled");
	});

	it("refuses work beyond the explicit task budget", () => {
		const coordinator = new FrontendWarmupCoordinator({ taskBudget: 1 });
		expect(
			coordinator.enqueue({
				key: "first",
				priority: "foreground",
				run: async () => undefined,
			}),
		).toBe(true);
		expect(
			coordinator.enqueue({
				key: "second",
				priority: "idle",
				run: async () => undefined,
			}),
		).toBe(false);
		expect(coordinator.getDiagnostics().taskBudget).toBe(1);
	});

	it("aborts running work and rejects a lease that exceeds the retained budget", async () => {
		const observedAbort = vi.fn();
		const release = vi.fn();
		const gate = deferred();
		const coordinator = new FrontendWarmupCoordinator({
			concurrency: 1,
			retainedByteBudget: 8,
		});
		coordinator.enqueue({
			key: "oversized",
			priority: "foreground",
			run: async () => ({ release, retainedBytes: 9 }),
		});
		coordinator.enqueue({
			key: "cancelled",
			priority: "idle",
			run: async (signal) => {
				signal.addEventListener("abort", observedAbort);
				await gate.promise;
			},
		});
		coordinator.start();
		await settle();

		expect(release).toHaveBeenCalledOnce();
		expect(coordinator.getDiagnostics().tasks[0]).toMatchObject({
			key: "oversized",
			status: "error",
		});
		await settle();
		coordinator.cancel();
		expect(observedAbort).toHaveBeenCalledOnce();
		gate.resolve();
	});
});
