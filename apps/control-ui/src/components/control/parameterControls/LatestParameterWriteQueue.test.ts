import { describe, expect, it, vi } from "vitest";
import { LatestParameterWriteQueue } from "./LatestParameterWriteQueue";

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((settle) => {
		resolve = settle;
	});
	return { promise, resolve };
}

describe("LatestParameterWriteQueue", () => {
	it("sends the active and latest continuous value without an unbounded FIFO", async () => {
		const first = deferred<string>();
		const calls: string[] = [];
		const queue = new LatestParameterWriteQueue();
		const active = queue.submitLatest("fixture:intensity", "0.1", async () => {
			calls.push("0.1");
			return first.promise;
		});
		const obsolete = queue.submitLatest(
			"fixture:intensity",
			"0.2",
			async () => {
				calls.push("0.2");
				return "obsolete";
			},
		);
		const latest = queue.submitLatest("fixture:intensity", "0.3", async () => {
			calls.push("0.3");
			return "latest";
		});

		expect(calls).toEqual(["0.1"]);
		expect(await obsolete).toBeNull();
		first.resolve("first");
		expect(await active).toBe("first");
		expect(await latest).toBe("latest");
		expect(calls).toEqual(["0.1", "0.3"]);
	});

	it("treats range or release work as a FIFO barrier", async () => {
		const first = deferred<void>();
		const calls: string[] = [];
		const queue = new LatestParameterWriteQueue();
		const active = queue.submitLatest("fixture:intensity", "0.1", async () => {
			calls.push("first");
			return first.promise;
		});
		const barrier = queue.submitBarrier(async () => {
			calls.push("release");
			return "released";
		});
		const after = queue.submitLatest("fixture:intensity", "0.2", async () => {
			calls.push("after");
			return "after";
		});
		first.resolve();
		await Promise.all([active, barrier, after]);
		expect(calls).toEqual(["first", "release", "after"]);
	});

	it("keeps only the latest pending write per continuous target", async () => {
		const first = deferred<void>();
		const calls: string[] = [];
		const queue = new LatestParameterWriteQueue();
		const active = queue.submitLatest("fixture:intensity", "0.1", async () => {
			calls.push("active");
			return first.promise;
		});
		const obsoletePan = queue.submitLatest("fixture:pan", "0.2", async () => {
			calls.push("obsolete-pan");
		});
		const tilt = queue.submitLatest("fixture:tilt", "0.3", async () => {
			calls.push("tilt");
		});
		const latestPan = queue.submitLatest("fixture:pan", "0.4", async () => {
			calls.push("latest-pan");
		});

		expect(await obsoletePan).toBeNull();
		first.resolve();
		await Promise.all([active, tilt, latestPan]);
		expect(calls).toEqual(["active", "tilt", "latest-pan"]);
	});

	it("collapses a duplicate pointer-up while the same value is active", async () => {
		const first = deferred<void>();
		const run = vi.fn(() => first.promise);
		const queue = new LatestParameterWriteQueue();
		const active = queue.submitLatest("group:pan", "0.5", run);
		expect(await queue.submitLatest("group:pan", "0.5", run)).toBeNull();
		first.resolve();
		await active;
		expect(run).toHaveBeenCalledOnce();
	});

	it("continues with the latest value after an active failure", async () => {
		const first = deferred<void>();
		const calls: string[] = [];
		const queue = new LatestParameterWriteQueue();
		const active = queue.submitLatest("fixture:pan", "0.1", async () => {
			calls.push("first");
			await first.promise;
			throw new Error("rollback");
		});
		const latest = queue.submitLatest("fixture:pan", "0.2", async () => {
			calls.push("latest");
			return "settled";
		});
		first.resolve();
		await expect(active).rejects.toThrow("rollback");
		expect(await latest).toBe("settled");
		expect(calls).toEqual(["first", "latest"]);
	});
});
