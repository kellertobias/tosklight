import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import { readFile, mkdtemp } from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import vm from "node:vm";
import * as recorder from "./desktop-performance-headless-stacks-recorder.mjs";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	HeadlessStackCapture,
	emptyReport,
	exportReport,
} from "./desktop-performance-headless-stacks.mjs";
const source = await readFile(
	new URL(
		"./desktop-performance-headless-stacks-recorder.mjs",
		import.meta.url,
	),
	"utf8",
);
const stackSource = await readFile(
	new URL("./desktop-performance-headless-stacks.mjs", import.meta.url),
	"utf8",
);
function failedControl({
	code = 255,
	signal = null,
	diagnostic = "no samples collected\n",
	spawnFailure = false,
	captureFailure = false,
	noPingAck = false,
	capRaw = false,
} = {}) {
	const child = new EventEmitter();
	Object.assign(child, {
		pid: 333,
		exitCode: null,
		signalCode: null,
		stdout: new EventEmitter(),
		stderr: new EventEmitter(),
		stdin: new EventEmitter(),
	});
	const sinks = new Map();
	class Sink {
		constructor(filename, _limit, onCap) {
			this.onCap = onCap;
			this.filename = filename;
			this.bytes = 0;
			this.text = "";
			sinks.set(filename, this);
		}
		write(chunk) {
			if (capRaw && this.filename.endsWith(".data")) this.onCap();
			if (captureFailure && this.filename.endsWith(".data"))
				throw new Error("private capture error");
			this.text += chunk.toString();
			this.bytes += chunk.length;
		}
		close() {}
		async verify() {}
	}
	child.stdin.write = (command) => {
		if (command !== "ping\n" || !noPingAck)
			child.stderr.emit("data", Buffer.from("ack\n"));
		if (command === "stop\n" && child.exitCode === null && !child.signalCode) {
			child.stderr.emit("data", Buffer.from(diagnostic));
			child.exitCode = code;
			child.signalCode = signal;
			child.emit("close", code, signal);
		}
	};
	const a = source.indexOf("export async function openRecorder(");
	const z = source.indexOf("\nexport async function boundedCommand(", a);
	const open = vm.runInNewContext(
		`${source.slice(a, z).replace("export async", "async")}; openRecorder`,
		{
			verifyIdentity: async () => {},
			privateDirectory: async () => {},
			PrivateSink: Sink,
			path,
			LIMITS: recorder.LIMITS,
			Buffer,
			process,
			recordArguments: recorder.recordArguments,
			recorderFailureDetails: recorder.recorderFailureDetails,
			spawn: () => {
				if (spawnFailure)
					queueMicrotask(() => child.emit("error", new Error("spawn failed")));
				return child;
			},
			until: async (predicate) => {
				await Promise.resolve();
				assert.ok(predicate());
			},
			recorderDescendants: async () => [{ pid: 333, start_ticks: 1 }],
			requireGone: async () => {},
			require_: (ok, message) => {
				if (!ok) throw new Error(message);
			},
			setTimeout,
			clearTimeout,
			privateText: async (filename) => sinks.get(filename)?.text ?? "",
		},
	);
	return { child, open };
}
test("actual privileged stop rejection retains stage, primitive exit and ACK evidence without changing failure", async () => {
	const f = failedControl();
	const recording = await f.open({
		root: "/private",
		name: "sudo-control",
		perf: "/installed/perf",
		privileged: true,
		binding: { pid: 10 },
	});
	await assert.rejects(recording.finish(), (error) => {
		assert.match(error.message, /record command failed/);
		assert.equal(error.recordDiagnostics?.record_stage, 2);
		assert.equal(error.recordDiagnostics?.record_exit_code, 255);
		assert.equal(error.recordDiagnostics?.record_signal, 0);
		assert.equal(error.recordDiagnostics?.record_privileged, 1);
		assert.equal(error.recordDiagnostics?.record_stop_acknowledged, 1);
		assert.equal(error.recordDiagnostics?.record_ack_count, 3);
		assert.equal(error.recordDiagnostics?.record_stderr_class, 1);
		return true;
	});
});
test("actual host preflight exports recorder evidence while preserving the original rejection", async () => {
	const failure = new Error("record command failed");
	failure.recordDiagnostics = {
		record_stage: 2,
		record_exit_code: 255,
		record_signal: 0,
		record_privileged: 1,
		record_spawn_error: 0,
		record_capture_error: 0,
		record_capped: 0,
		record_stop_requested: 1,
		record_stop_acknowledged: 1,
		record_ack_count: 3,
		record_ended_early: 0,
		record_stderr_class: 1,
	};
	let report;
	const a = stackSource.indexOf("export async function hostPreflight(");
	const z = stackSource.indexOf("\nexport async function exportReport(", a);
	const preflight = vm.runInNewContext(
		`${stackSource.slice(a, z).replace("export async", "async")}; hostPreflight`,
		{
			require_: (ok, message) => {
				if (!ok) throw new Error(message);
			},
			process: { platform: "linux" },
			mkdir: async () => {},
			mkdtemp: async () => "/private",
			chmod: async () => {},
			artifactPaths: { tmp: "/private" },
			path,
			emptyReport,
			verifiedTool: async () => "hash",
			boundedCommand: async () => "dwarf: on\n",
			selectPreflightMode: async () => {
				throw failure;
			},
			exportReport: async (_directory, value) => {
				report = value;
			},
		},
	);
	await assert.rejects(
		preflight("/installed/perf", "/public"),
		(error) => error === failure,
	);
	assert.equal(report.failure_code, 1);
	assert.equal(report.record_stage, 2);
	assert.equal(report.record_exit_code, 255);
	assert.equal(report.record_stderr_class, 1);
	assert.equal(report.privileged, 1);
	assert.equal(report.capture_complete, 0);
});

test("actual recorder retains direct SIGTERM versus sudo143 without treating either as success", async () => {
	for (const [code, signal, exitExpected, signalExpected] of [
		[null, "SIGTERM", null, 15],
		[143, null, 143, 0],
	]) {
		const f = failedControl({ code, signal });
		const recording = await f.open({
			root: "/private",
			name: "sudo-control",
			perf: "/installed/perf",
			privileged: true,
			binding: { pid: 10 },
		});
		await assert.rejects(recording.finish(), (error) => {
			assert.equal(error.recordDiagnostics.record_exit_code, exitExpected);
			assert.equal(error.recordDiagnostics.record_signal, signalExpected);
			assert.equal(error.recordDiagnostics.record_stop_acknowledged, 1);
			return true;
		});
	}
});
test("actual recorder exposes spawn/write failures as numeric flags, retaining generic rejection", async () => {
	for (const kind of ["spawnFailure", "captureFailure"]) {
		const f = failedControl({ code: 0, diagnostic: "", [kind]: true });
		const recording = await f.open({
			root: "/private",
			name: "headless",
			perf: "/installed/perf",
			privileged: false,
			binding: { pid: 10 },
		});
		if (kind === "captureFailure")
			f.child.stdout.emit("data", Buffer.from("sample"));
		await assert.rejects(recording.finish(), (error) => {
			assert.equal(error.recordDiagnostics.record_stage, 3);
			assert.equal(
				error.recordDiagnostics.record_spawn_error,
				Number(kind === "spawnFailure"),
			);
			assert.equal(
				error.recordDiagnostics.record_capture_error,
				Number(kind === "captureFailure"),
			);
			assert.equal(error.recordDiagnostics.record_exit_code, 0);
			return true;
		});
	}
});
test("stderr classifier is closed and never publishes diagnostic text, paths, symbols or process IDs", () => {
	const messages = [
		["ack\n", 0],
		["no samples collected", 1],
		["Permission denied /private/key", 2],
		["Input/output error pid123", 3],
		["unsupported: not supported", 4],
		["fatal: private_symbol", 5],
		["unrecognized private content /secret", 6],
		[null, null],
	];
	for (const [stderr, expected] of messages) {
		const value = recorder.recorderFailureDetails({
			name: "user-bound",
			privileged: false,
			result: null,
			ack: 2,
			stderr,
		});
		assert.equal(value.record_stage, 1);
		assert.equal(value.record_stderr_class, expected);
		assert.equal(value.record_exit_code, null);
		assert.equal(value.record_signal, null);
		assert.equal(value.record_stop_acknowledged, null);
		assert.equal(
			Object.values(value).every(
				(entry) => entry === null || typeof entry === "number",
			),
			true,
		);
		assert.doesNotMatch(JSON.stringify(value), /private|secret|symbol|pid123/);
	}
});
test("schema2 numeric export retains exact failure evidence and rejects oldschema, raw and hostile enum fields", async () => {
	const details = recorder.recorderFailureDetails({
		name: "sudo-control",
		privileged: true,
		result: { code: 143, signal: null },
		ack: 3,
		capped: true,
		stopRequested: true,
		stopAcknowledged: true,
		endedEarly: false,
		stderr: "No samples collected",
	});
	const report = {
		...emptyReport(),
		...details,
		failure_code: 1,
		privileged: 1,
	};
	assert.equal(report.schema_version, 2);
	const directory = await mkdtemp(
		path.join(artifactPaths.tmp, "stack-failure-public-"),
	);
	await exportReport(directory, report);
	assert.deepEqual(
		JSON.parse(
			await readFile(
				path.join(directory, "headless-stacks-numeric.json"),
				"utf8",
			),
		),
		report,
	);
	for (const patch of [
		{ schema_version: 1 },
		{ record_stage: 4 },
		{ record_signal: 14 },
		{ record_exit_code: -1 },
		{ record_exit_code: 256 },
		{ record_ack_count: 16385 },
		{ record_stderr_class: 7 },
		{ record_privileged: 2 },
		{ record_stop_requested: 2 },
		{ record_capture_error: "/private/error" },
		{ record_pid: 123 },
		{ record_stderr: "raw" },
	]) {
		await assert.rejects(exportReport(directory, { ...report, ...patch }));
	}
});

test("startup ACK failure keeps bounded recorder evidence without replacing original startup error", async () => {
	const f = failedControl({ noPingAck: true });
	await assert.rejects(
		f.open({
			root: "/private",
			name: "user-bound",
			perf: "/installed/perf",
			privileged: false,
			binding: { pid: 10 },
		}),
		(error) => {
			assert.notEqual(error.message, "record command failed");
			assert.equal(error.recordDiagnostics.record_stage, 1);
			assert.equal(error.recordDiagnostics.record_exit_code, 255);
			assert.equal(error.recordDiagnostics.record_ack_count, 1);
			assert.equal(error.recordDiagnostics.record_stop_acknowledged, 1);
			return true;
		},
	);
});

test("natural lifetime rejection never invents a requested STOP or acknowledgment", async () => {
	const f = failedControl();
	const recording = await f.open({
		root: "/private",
		name: "user-bound",
		perf: "/installed/perf",
		privileged: false,
		binding: { pid: 10 },
	});
	f.child.exitCode = 255;
	f.child.emit("close", 255, null);
	await assert.rejects(recording.finish(false), (error) => {
		assert.equal(error.recordDiagnostics.record_stop_requested, 0);
		assert.equal(error.recordDiagnostics.record_stop_acknowledged, null);
		assert.equal(error.recordDiagnostics.record_ack_count, 2);
		return true;
	});
});

test("already closed recorder reports no attempted STOP despite finish default", async () => {
	const f = failedControl();
	const recording = await f.open({
		root: "/private",
		name: "user-control",
		perf: "/installed/perf",
		binding: { pid: 10 },
	});
	f.child.exitCode = 255;
	f.child.emit("close", 255, null);
	await assert.rejects(recording.finish(), (error) => {
		assert.equal(error.recordDiagnostics.record_stop_requested, 0);
		assert.equal(error.recordDiagnostics.record_stop_acknowledged, null);
		return true;
	});
});
test("cap and capture-error STOP remain observed when finalization uses natural mode", async () => {
	for (const option of ["capRaw", "captureFailure"]) {
		const f = failedControl({ [option]: true });
		const recording = await f.open({
			root: "/private",
			name: "user-bound",
			perf: "/installed/perf",
			binding: { pid: 10 },
		});
		f.child.stdout.emit("data", Buffer.from("data"));
		await assert.rejects(recording.finish(false), (error) => {
			assert.equal(error.recordDiagnostics.record_stop_requested, 1);
			assert.equal(error.recordDiagnostics.record_stop_acknowledged, null);
			assert.equal(
				error.recordDiagnostics[
					option === "capRaw" ? "record_capped" : "record_capture_error"
				],
				1,
			);
			return true;
		});
	}
});

test("actual measurement boundary exports recorder failure while retaining original workload error", async () => {
	const original = new Error("original workload error");
	const failure = new Error("record command failed");
	failure.recordDiagnostics = recorder.recorderFailureDetails({
		name: "headless",
		privileged: true,
		result: { code: 143, signal: null },
		stopRequested: true,
		stopAcknowledged: true,
		ack: 3,
		stderr: "no samples",
	});
	const binding = {
		pid: 10,
		executable: "/private/server",
		ancestry: [{ pid: 9 }],
	};
	const cpu = {
		mode: "off",
		rootPid: 9,
		bindings: new Map([[10, binding]]),
		receipts: [{ owned: [binding] }],
	};
	let report;
	const capture = new HeadlessStackCapture(
		{
			root: "/private",
			outputDirectory: "/public",
			preflight: { perf: "/installed/perf", privileged: true },
			binaryHash: createHash("sha256").update("binary").digest("hex"),
			report: emptyReport(),
		},
		{
			verifyIdentity: async () => {},
			threads: async () => [10],
			readFile: async () => Buffer.from("binary"),
			writePrivate: async () => {},
			openRecorder: async () => ({
				raw: { bytes: 4 },
				finish: async () => {
					throw failure;
				},
			}),
			exportReport: async (_directory, value) => {
				report = structuredClone(value);
			},
		},
	);
	await assert.rejects(
		capture.run(cpu, async () => {
			throw original;
		}),
		(error) => error === original,
	);
	assert.equal(report.record_stage, 3);
	assert.equal(report.record_exit_code, 143);
	assert.equal(report.record_stop_requested, 1);
	assert.equal(report.record_stop_acknowledged, 1);
	assert.equal(report.workload_failed, 1);
	assert.equal(report.capture_complete, 0);
});
