import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { benchmarkListenerPids, startOwnedBenchmarkServer, stopOwnedBenchmarkChild } from "./owned-benchmark-server.mjs";

function fixture(overrides = {}) {
	const child = Object.assign(new EventEmitter(), { pid: 12345, exitCode: null, signalCode: null });
	const calls = [];
	let clock = 1000;
	const binary = { executable: "/owned/headless", sha256: "frozen-binary" };
	const options = { port: 19500, executable: binary.executable, timeoutMillis: 500,
		spawnServer: async () => { calls.push("spawn"); return child; } };
	const dependencies = {
		freePort: async () => { calls.push("free-port"); },
		identify: async () => ({ ...binary }),
		listeners: async () => { calls.push("listener"); return [child.pid]; },
		readiness: async () => { calls.push("readiness"); return true; },
		stop: async (owned) => { assert.equal(owned, child); calls.push("stop-owned"); child.exitCode = 0; },
		now: () => clock,
		sleep: async (millis) => { clock += millis; },
		...overrides,
	};
	return { child, calls, options, dependencies, binary };
}

test("slow original startup must complete healthy and owned before it is returned for app launch", async () => {
	let polls = 0;
	const f = fixture({ readiness: async () => ++polls >= 4 });
	const result = await startOwnedBenchmarkServer(f.options, f.dependencies);
	assert.equal(polls, 4);
	assert.deepEqual(result.identity.begin, { pid: 12345, listenerPid: 12345, ...f.binary, verifiedAtEpochMillis: 1300 });
	assert.deepEqual(f.calls, ["free-port", "spawn", "listener"]);
	assert.equal((await result.verify()).listenerPid, 12345);
});

test("occupied preflight port fails without spawning or accepting existing readiness", async () => {
	const f = fixture({ freePort: async () => { throw new Error("occupied"); } });
	await assert.rejects(startOwnedBenchmarkServer(f.options, f.dependencies), /occupied/);
	assert.deepEqual(f.calls, []);
});

test("foreign or ambiguous listener readiness fails and cleans only the owned child", async () => {
	for (const pids of [[999], [12345, 999], []]) {
		const f = fixture({ listeners: async () => pids });
		await assert.rejects(startOwnedBenchmarkServer(f.options, f.dependencies), /ownership mismatch/);
		assert.equal(f.calls.at(-1), "stop-owned");
	}
});

test("original child exit racing with a healthy response cannot pass", async () => {
	const f = fixture();
	f.dependencies.readiness = async () => { f.child.exitCode = 1; return true; };
	await assert.rejects(startOwnedBenchmarkServer(f.options, f.dependencies), /Original.*exited/);
	assert.equal(f.calls.at(-1), "stop-owned");
});

test("child exit during listener inspection and spawn failure both fail closed", async () => {
	for (const failure of ["exit", "spawn"]) {
		const f = fixture();
		f.dependencies.listeners = async () => {
			if (failure === "exit") f.child.signalCode = "SIGTERM";
			else f.child.emit("error", new Error("spawn rejected"));
			return [f.child.pid];
		};
		await assert.rejects(startOwnedBenchmarkServer(f.options, f.dependencies), /Original.*(exited|failed)/);
		assert.equal(f.calls.at(-1), "stop-owned");
	}
});

test("readiness timeout or unavailable ownership tooling cleans the original child", async () => {
	for (const overrides of [
		{ readiness: async () => false },
		{ listeners: async () => { throw new Error("lsof unavailable"); } },
	]) {
		const f = fixture(overrides);
		await assert.rejects(startOwnedBenchmarkServer(f.options, f.dependencies), /did not become ready|lsof unavailable/);
		assert.equal(f.calls.at(-1), "stop-owned");
	}
});

test("end verification rejects listener replacement, child exit, or changed executable", async () => {
	for (const failure of ["listener", "exit", "binary"]) {
		let changed = false;
		const f = fixture({ listeners: async () => changed && failure === "listener" ? [999] : [12345],
			identify: async () => ({ executable: "/owned/headless", sha256: changed && failure === "binary" ? "replacement" : "frozen-binary" }) });
		const result = await startOwnedBenchmarkServer(f.options, f.dependencies);
		changed = true;
		if (failure === "exit") f.child.exitCode = 0;
		await assert.rejects(result.verify(), /ownership mismatch|Original.*exited|executable changed/);
	}
});

test("native listener tools preserve exact PID attribution on all supported hosts", async () => {
	for (const [platform, stdout, expectedCommand] of [
		["darwin", "12345\n12345\n", "/usr/sbin/lsof"],
		["linux", 'LISTEN 0 128 127.0.0.1:19500 0.0.0.0:* users:(("headless",pid=12345,fd=9))', "ss"],
		["win32", "12345\r\n", "powershell.exe"],
	]) {
		assert.deepEqual(await benchmarkListenerPids(19500, { platform, run: async (command, args, options) => {
			assert.equal(command, expectedCommand);
			assert.ok(args.some((arg) => arg.includes("19500")));
			assert.equal(options.timeout, 2000);
			return { stdout };
		} }), [12345]);
	}
	await assert.rejects(benchmarkListenerPids(19500, { platform: "linux", run: async () => ({ stdout: "LISTEN unknown-owner" }) }), /did not expose its PID/);
});

test("cleanup signals only the supplied child and does nothing after exit", async () => {
	const f = fixture();
	const signals = [];
	f.child.kill = (signal) => {
		signals.push(signal);
		f.child.signalCode = signal;
		f.child.emit("exit", null, signal);
	};
	await stopOwnedBenchmarkChild(f.child);
	await stopOwnedBenchmarkChild(f.child);
	assert.deepEqual(signals, ["SIGTERM"]);
});
