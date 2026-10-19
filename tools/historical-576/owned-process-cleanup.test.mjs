import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import {
	stopOwnedGroup,
	stopOwned,
	applyCleanupResult,
} from "./owned-process-cleanup.mjs";
function fixture({
	closed = true,
	absentAt = Infinity,
	closeAt = Infinity,
	killDelay = null,
} = {}) {
	const child = new EventEmitter();
	Object.assign(child, {
		pid: 123,
		exitCode: closed ? 0 : null,
		signalCode: null,
	});
	let time = 0,
		absentTime = absentAt;
	const signals = [],
		waits = [],
		probes = [];
	const dependencies = {
		now: () => time,
		pause: async (ms) => {
			assert(ms > 0 && ms <= 50);
			waits.push(ms);
			time += ms;
			if (time >= closeAt && child.exitCode === null) {
				child.exitCode = 0;
				child.emit("close", 0);
			}
		},
		kill: (pid, signal) => {
			assert.equal(pid, -123, "only exact newly owned PGID");
			if (signal === 0) {
				probes.push(time);
				if (time >= absentTime) {
					const e = Error("absent");
					e.code = "ESRCH";
					throw e;
				}
				return;
			}
			signals.push([signal, time]);
			if (signal === "SIGKILL" && killDelay !== null)
				absentTime = time + killDelay;
		},
	};
	return { child, dependencies, signals, waits, probes, time: () => time };
}
test("closed leader does not skip TERM grace while descendants terminate", async () => {
	const f = fixture({ absentAt: 300 });
	assert.equal(await stopOwnedGroup(f.child, f.dependencies), 1);
	assert.equal(f.time(), 300);
	assert.deepEqual(f.signals, [["SIGTERM", 0]]);
	assert.equal(f.waits.length, 6);
	assert.equal(f.child.listenerCount("close"), 0);
});
test("KILL receives its own full grace even when leader was already reaped", async () => {
	const f = fixture({ killDelay: 200 });
	assert.equal(await stopOwnedGroup(f.child, f.dependencies), 1);
	assert.equal(f.time(), 2200);
	assert.deepEqual(f.signals, [
		["SIGTERM", 0],
		["SIGKILL", 2000],
	]);
	assert(f.probes.includes(2150));
});
test("group absence alone waits for direct leader reaping", async () => {
	const f = fixture({ closed: false, absentAt: 0, closeAt: 300 });
	assert.equal(await stopOwnedGroup(f.child, f.dependencies), 1);
	assert.equal(f.time(), 300);
	assert.equal(f.child.exitCode, 0);
	assert.deepEqual(f.signals, [["SIGTERM", 0]]);
});
test("persisting live or zombie group remains unverified at bounded deadline", async () => {
	for (const state of ["live", "zombie"]) {
		const f = fixture();
		f.child.testGroupState = state;
		assert.equal(await stopOwnedGroup(f.child, f.dependencies), 0);
		assert.equal(f.time(), 4000);
		assert.deepEqual(f.signals, [
			["SIGTERM", 0],
			["SIGKILL", 2000],
		]);
		assert(f.waits.length <= 80);
	}
});
test("permission and broken clock fail closed without other signal targets", async () => {
	const f = fixture();
	const denied = {
		...f.dependencies,
		kill: (pid) => {
			assert.equal(pid, -123);
			const e = Error("denied");
			e.code = "EPERM";
			throw e;
		},
	};
	await assert.rejects(stopOwnedGroup(f.child, denied), { code: "EPERM" });
	assert.equal(await stopOwned(f.child, denied), 0);
	assert.equal(f.child.listenerCount("close"), 0);
	const g = fixture();
	let reads = 0;
	assert.equal(
		await stopOwned(g.child, {
			...g.dependencies,
			now: () => (reads++ ? 0 : 10),
		}),
		0,
	);
	assert.equal(g.child.listenerCount("close"), 0);
	const h = fixture();
	assert.equal(
		await stopOwned(h.child, {
			...h.dependencies,
			now: () => 0,
			pause: async () => {},
		}),
		0,
	);
	assert(h.probes.length <= 128);
});
test("timer signal and finalizer coalesce the same child promise and signal wave", async () => {
	const f = fixture({ absentAt: 300 });
	const first = stopOwned(f.child, f.dependencies),
		second = stopOwned(f.child, {
			kill: () => assert.fail("duplicate cleanup"),
			pause: () => assert.fail("duplicate wait"),
			now: () => assert.fail("duplicate clock"),
		});
	assert.equal(first, second);
	assert.equal(await first, 1);
	assert.equal(stopOwned(f.child, f.dependencies), first);
	assert.deepEqual(f.signals, [["SIGTERM", 0]]);
});
test("cleanup failure preserves initial stage and never completes the report", () => {
	const primary = { failure_stage_code: 3, complete: 1, cleanup_verified: 1 };
	assert.equal(applyCleanupResult(primary, 0).failure_stage_code, 3);
	assert.equal(primary.complete, 0);
	assert.equal(primary.cleanup_verified, 0);
	const noPrimary = { failure_stage_code: 0, complete: 1, cleanup_verified: 1 };
	applyCleanupResult(noPrimary, 0);
	assert.equal(noPrimary.failure_stage_code, 12);
	assert.equal(noPrimary.complete, 0);
	const success = { failure_stage_code: 3, complete: 0, cleanup_verified: 0 };
	applyCleanupResult(success, 1);
	assert.deepEqual(success, {
		failure_stage_code: 3,
		complete: 0,
		cleanup_verified: 1,
	});
});
