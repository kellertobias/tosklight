import { describe, expect, it, vi } from "vitest";
import { LatestProgrammerValuesWriteQueue } from "./LatestProgrammerValuesWriteQueue";

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((complete, fail) => {
		resolve = complete;
		reject = fail;
	});
	return { promise, resolve, reject };
}

async function flush() {
	for (let turn = 0; turn < 5; turn++)
		await new Promise((resolve) => setTimeout(resolve, 0));
}

function settlementSpy<T>(promise: Promise<T>) {
	const spy = vi.fn();
	void promise.then(spy, spy);
	return spy;
}

describe("LatestProgrammerValuesWriteQueue gesture cancellation", () => {
	it("drops only unsent gesture tasks while the active task settles once", async () => {
		const queue = new LatestProgrammerValuesWriteQueue();
		const order: string[] = [];
		const activeResponse = deferred<string>();
		const active = queue.submitLatest(
			"fixture:intensity",
			"0.1",
			() => {
				order.push("a-1");
				return activeResponse.promise;
			},
			{ gesture: "gesture-a" },
		);
		const unsentLatest = queue.submitLatest(
			"fixture:intensity",
			"0.2",
			async () => order.push("a-2"),
			{ gesture: "gesture-a" },
		);
		const barrier = queue.submitBarrier(async () => {
			order.push("barrier");
			return "barrier";
		});
		const gestureB = queue.submitLatest(
			"fixture:pan",
			"0.3",
			async () => {
				order.push("b-1");
				return "b-1";
			},
			{ gesture: "gesture-b" },
		);
		const unsentBarrier = queue.submitBarrier(
			async () => order.push("a-step"),
			{ gesture: "gesture-a" },
		);
		await flush();
		const activeSettled = settlementSpy(active);

		expect(queue.cancelGesture("gesture-a")).toBe(2);
		await expect(unsentLatest).resolves.toBeNull();
		await expect(unsentBarrier).resolves.toBeNull();
		expect(activeSettled).not.toHaveBeenCalled();
		expect(order).toEqual(["a-1"]);

		activeResponse.resolve("a-1 accepted");
		await expect(active).resolves.toBe("a-1 accepted");
		await expect(barrier).resolves.toBe("barrier");
		await expect(gestureB).resolves.toBe("b-1");
		expect(activeSettled).toHaveBeenCalledOnce();
		expect(order).toEqual(["a-1", "barrier", "b-1"]);
	});

	it("keeps a late active rejection visible after repeated cancellation", async () => {
		const queue = new LatestProgrammerValuesWriteQueue();
		const activeResponse = deferred<string>();
		const active = queue.submitLatest(
			"fixture:intensity",
			"0.1",
			() => activeResponse.promise,
			{ gesture: "gesture-a" },
		);
		const unsent = queue.submitLatest(
			"fixture:intensity",
			"0.2",
			async () => "never",
			{ gesture: "gesture-a" },
		);
		await flush();
		const unsentSettled = settlementSpy(unsent);

		expect(queue.cancelGesture("gesture-a")).toBe(1);
		expect(queue.cancelGesture("gesture-a")).toBe(0);
		expect(queue.cancelGesture("")).toBe(0);
		const failure = new Error("transport failed");
		activeResponse.reject(failure);
		await expect(active).rejects.toBe(failure);
		expect(queue.cancelGesture("gesture-a")).toBe(0);
		await flush();
		expect(unsentSettled).toHaveBeenCalledOnce();
		expect(unsentSettled).toHaveBeenCalledWith(null);

		await expect(
			queue.submitLatest("fixture:intensity", "0.4", async () => "next", {
				gesture: "gesture-a",
			}),
		).resolves.toBe("next");
	});

	it("uses fresh gesture IDs and keeps coalescing across gestures bounded", async () => {
		const queue = new LatestProgrammerValuesWriteQueue();
		const activeResponse = deferred<string>();
		const run = vi.fn(async (value: string) => value);
		const active = queue.submitBarrier(() => activeResponse.promise);
		const oldTouch = queue.submitLatest(
			"fixture:pan",
			"0.1",
			() => run("old"),
			{ gesture: "touch-1" },
		);
		const freshTouch = queue.submitLatest(
			"fixture:pan",
			"0.2",
			() => run("fresh"),
			{ gesture: "touch-2" },
		);
		await expect(oldTouch).resolves.toBeNull();

		expect(queue.cancelGesture("touch-1")).toBe(0);
		activeResponse.resolve("barrier");
		await expect(active).resolves.toBe("barrier");
		await expect(freshTouch).resolves.toBe("fresh");
		expect(run).toHaveBeenCalledOnce();
	});

	it("never removes untagged tasks and keeps permanent stop", async () => {
		const queue = new LatestProgrammerValuesWriteQueue();
		const activeResponse = deferred<string>();
		const active = queue.submitBarrier(() => activeResponse.promise);
		const untagged = queue.submitLatest(
			"fixture:pan",
			"0.1",
			async () => "untagged",
		);
		const tagged = queue.submitLatest(
			"fixture:tilt",
			"0.2",
			async () => "tagged",
			{ gesture: "gesture-a" },
		);
		expect(queue.cancelGesture("gesture-b")).toBe(0);
		queue.stop();
		await expect(untagged).resolves.toBeNull();
		await expect(tagged).resolves.toBeNull();
		expect(queue.cancelGesture("gesture-a")).toBe(0);
		await expect(
			queue.submitLatest("fixture:pan", "0.3", async () => "late", {
				gesture: "gesture-c",
			}),
		).resolves.toBeNull();
		activeResponse.resolve("active");
		await expect(active).resolves.toBe("active");
	});
});
