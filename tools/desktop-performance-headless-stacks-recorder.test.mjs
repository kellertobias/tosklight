import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import {
	readFile,
	mkdtemp,
	rm,
	mkdir,
	writeFile,
	chmod,
	lstat,
	symlink,
	link,
	rename,
} from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import vm from "node:vm";
import * as recorder from "./desktop-performance-headless-stacks-recorder.mjs";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	HeadlessStackCapture,
	emptyReport,
	exportReport,
	verifyMonotonicClock,
	validateDisabledSamples,
	PerfScriptParser,
} from "./desktop-performance-headless-stacks.mjs";
const scratch = await mkdtemp(
	path.join(artifactPaths.tmp, "stack-recorder-tests-"),
);
after(() => rm(scratch, { recursive: true, force: true }));
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
	disableAck = "ack\n",
	neverExit = false,
	immediateDeadline = false,
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
	const commands = [];
	child.stdin.write = (command) => {
		commands.push(command);
		if (command === "disable\n") {
			child.stderr.emit("data", Buffer.from(disableAck));
			if (neverExit) return;
			queueMicrotask(() => {
				child.stderr.emit("data", Buffer.from(diagnostic));
				child.exitCode = code;
				child.signalCode = signal;
				child.emit("close", code, signal);
			});
			return;
		}
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
			setTimeout: immediateDeadline
				? (callback) => {
						queueMicrotask(callback);
						return 1;
					}
				: setTimeout,
			clearTimeout,
			privateText: async (filename) => sinks.get(filename)?.text ?? "",
		},
	);
	return { child, open, commands };
}
test("actual privileged disable/natural rejection retains stage, primitive exit and ACK evidence without changing failure", async () => {
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
		assert.equal(error.recordDiagnostics?.record_stop_acknowledged, null);
		assert.equal(error.recordDiagnostics?.record_disable_acknowledged, 1);
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
			assert.equal(error.recordDiagnostics.record_stop_acknowledged, null);
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
test("schema4 numeric export retains exact failure evidence and rejects oldschema, raw and hostile enum fields", async () => {
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
	assert.equal(report.schema_version, 4);
	const directory = await mkdtemp(path.join(scratch, "stack-failure-public-"));
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
		{ schema_version: 3 },
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
			assert.equal(error.recordDiagnostics.record_stop_acknowledged, null);
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

test("normal completion disables with ACK then waits natural zero, never sends STOP", async () => {
	const f = failedControl({ code: 0, diagnostic: "" });
	const recording = await f.open({
		root: "/private",
		name: "user-control",
		perf: "/installed/perf",
		binding: { pid: 10 },
	});
	const result = await recording.finish();
	assert.deepEqual(f.commands, ["ping\n", "enable\n", "disable\n"]);
	assert.equal(result.diagnostics.record_disable_requested, 1);
	assert.equal(result.diagnostics.record_disable_acknowledged, 1);
	assert.equal(result.diagnostics.record_stop_requested, 0);
	assert.equal(result.diagnostics.record_stop_acknowledged, null);
	assert.ok(result.activeSamplingWallNs >= 0);
});
test("missing or malformed disable ACK never accepts natural zero", async () => {
	for (const disableAck of ["", "ack extra\n"]) {
		const f = failedControl({ code: 0, disableAck });
		const recording = await f.open({
			root: "/private",
			name: "user-control",
			perf: "/installed/perf",
			binding: { pid: 10 },
		});
		await assert.rejects(recording.finish(), (error) => {
			assert.equal(error.recordDiagnostics.record_disable_acknowledged, 0);
			assert.equal(error.recordDiagnostics.record_stop_requested, 0);
			return true;
		});
	}
});

test("monotonic epoch/units require actual bounded Python value inside Node bracket", async () => {
	await verifyMonotonicClock("/private", async (...args) => {
		assert.equal(args[2], "/usr/bin/python3");
		assert.deepEqual(args[3], [
			"-c",
			"import time; print(time.clock_gettime_ns(time.CLOCK_MONOTONIC))",
		]);
		assert.equal(args[4], 4096);
		assert.equal(args[7], 2000);
		return String(process.hrtime.bigint()) + "\n";
	});
	for (const response of [
		"0\n",
		"99999999999999999999999\n",
		"private raw text",
	]) {
		await assert.rejects(
			verifyMonotonicClock("/private", async () => response),
		);
	}
});
test("decoded samples enforce conservative disable clock boundary and positive busy tail", () => {
	const parser = new PerfScriptParser(10);
	parser.feed(
		"10/10 1234.000000001: 20 cpu-clock:u:\n  a malloc (/private/server)\n\n",
	);
	parser.finish();
	const stopped = {
		disableAckNs: 1234000000002n,
		closedNs: 1234150000002n,
		diagnostics: { record_disable_acknowledged: 1 },
	};
	validateDisabledSamples(parser, stopped, 150000000n);
	assert.throws(() =>
		validateDisabledSamples(parser, {
			...stopped,
			disableAckNs: 1234000000000n,
		}),
	);
	assert.throws(() =>
		validateDisabledSamples(
			parser,
			{ ...stopped, closedNs: stopped.disableAckNs },
			150000000n,
		),
	);
	assert.throws(() =>
		validateDisabledSamples(new PerfScriptParser(10), stopped),
	);
	assert.throws(() =>
		validateDisabledSamples(parser, {
			...stopped,
			diagnostics: { record_disable_acknowledged: 0 },
		}),
	);
	assert.equal(
		Object.keys(parser.report).some((k) => /timestamp|SampleNs|ack_ns/.test(k)),
		false,
	);
});

test("normal completion never accepts early zero, natural signal, cap STOP or missing deadline exit", async () => {
	for (const scenario of [
		"early-zero",
		"signal",
		"cap",
		"capture-error",
		"deadline",
	]) {
		const f = failedControl({
			code: 0,
			capRaw: scenario === "cap",
			captureFailure: scenario === "capture-error",
			neverExit: scenario === "deadline",
			immediateDeadline: scenario === "deadline",
		});
		const recording = await f.open({
			root: "/private",
			name: "headless",
			perf: "/installed/perf",
			binding: { pid: 10 },
		});
		if (scenario === "early-zero" || scenario === "signal") {
			f.child.exitCode = scenario === "signal" ? null : 0;
			f.child.signalCode = scenario === "signal" ? "SIGTERM" : null;
			f.child.emit("close", f.child.exitCode, f.child.signalCode);
		}
		if (scenario === "cap" || scenario === "capture-error")
			f.child.stdout.emit("data", Buffer.from("data"));
		await assert.rejects(recording.finish(scenario !== "signal"), (error) => {
			if (scenario === "signal")
				assert.equal(error.recordDiagnostics.record_signal, 15);
			if (scenario === "cap")
				assert.equal(error.recordDiagnostics.record_capped, 1);
			if (scenario === "capture-error")
				assert.equal(error.recordDiagnostics.record_capture_error, 1);
			if (scenario === "deadline")
				assert.match(error.message, /lifetime bound/);
			return true;
		});
	}
});

// Real run/export boundaries: failure evidence must identify the attempted operation,
// without promoting a failed setup to recording success or exporting private errors.
test("pre-record setup stages preserve binding, identity, binary and thread failures", async (t) => {
	for (const scenario of [
		"binding",
		"identity",
		"read",
		"mismatch",
		"threads",
		"open",
	])
		await t.test(scenario, async () => {
			const original = new Error(`/private/secret pid=1234 ${scenario}`);
			const binding = {
				pid: 10,
				executable: "/private/server",
				ancestry: [{ pid: 9 }],
			};
			const cpu = {
				mode: scenario === "binding" ? "on" : "off",
				rootPid: 9,
				bindings: new Map([[10, binding]]),
				receipts: [{ owned: [binding] }],
			};
			let exported,
				opened = 0,
				measured = 0;
			const capture = new HeadlessStackCapture(
				{
					root: "/private",
					outputDirectory: "/public",
					preflight: { perf: "/usr/bin/perf", privileged: false },
					report: emptyReport(),
					binaryHash: createHash("sha256").update("binary").digest("hex"),
				},
				{
					verifyIdentity: async () => {
						if (scenario === "identity") throw original;
					},
					readFile: async () => {
						if (scenario === "read") throw original;
						return Buffer.from(scenario === "mismatch" ? "changed" : "binary");
					},
					threads: async () => {
						if (scenario === "threads") throw original;
						return [10];
					},
					writePrivate: async () => {},
					openRecorder: async () => {
						opened++;
						throw original;
					},
					exportReport: async (_directory, value) => {
						exported = structuredClone(value);
					},
				},
			);
			await assert.rejects(
				capture.run(cpu, async () => {
					measured++;
				}),
				(error) => {
					if (!["binding", "mismatch"].includes(scenario))
						assert.equal(error, original);
					return true;
				},
			);
			assert.equal(
				exported.pre_record_setup_stage,
				{ binding: 1, identity: 2, read: 3, mismatch: 3, threads: 4, open: 4 }[
					scenario
				],
			);
			assert.equal(
				exported.pre_record_binary_match,
				["threads", "open"].includes(scenario)
					? 1
					: scenario === "mismatch"
						? 0
						: null,
			);
			assert.equal(exported.recording_started, 0);
			assert.equal(exported.capture_complete, 0);
			assert.equal(opened, Number(scenario === "open"));
			assert.equal(measured, 0);
			assert.doesNotMatch(JSON.stringify(exported), /secret|pid=|\/private/u);
		});
});

test("schema4 setup evidence has a closed numeric vocabulary", async () => {
	for (const patch of [
		{ pre_record_setup_stage: null },
		{ pre_record_setup_stage: 5 },
		{ pre_record_setup_stage: -1 },
		{ pre_record_setup_stage: "hash" },
		{ pre_record_binary_match: 2 },
		{ pre_record_binary_match: "/private/binary" },
		{ schema_version: 3 },
		{ pre_record_binary_path: "/private/binary" },
	]) {
		await assert.rejects(exportReport(scratch, { ...emptyReport(), ...patch }));
	}
	for (const match of [null, 0, 1]) {
		const directory = await mkdtemp(path.join(scratch, "schema4-"));
		await exportReport(directory, {
			...emptyReport(),
			pre_record_setup_stage: 3,
			pre_record_binary_match: match,
		});
		const report = JSON.parse(
			await readFile(path.join(directory, "headless-stacks-numeric.json")),
		);
		assert.equal(report.schema_version, 4);
		assert.equal(report.pre_record_binary_match, match);
	}
});

const elf = (value) =>
	Buffer.concat([Buffer.from([0x7f, 69, 76, 70]), Buffer.from(value)]);
async function packageFixture(scenario = "valid") {
	const base = await mkdtemp(path.join(scratch, "package-"));
	const image = path.join(base, "candidate.AppImage");
	await writeFile(image, elf("AppImage"), { mode: 0o700 });
	const extract = async (
		root,
		name,
		executable,
		args,
		cap,
		output,
		input,
		timeout,
		cwd,
	) => {
		assert.equal(executable, image);
		assert.equal(name, "appimage-extract");
		assert.deepEqual(args, ["--appimage-extract"]);
		assert.equal(cap, 64 * 1024 * 1024);
		assert.equal(output, undefined);
		assert.equal(input, undefined);
		assert.equal(timeout, 30000);
		assert.equal(cwd, root);
		assert.equal((await lstat(root)).mode & 0o777, 0o700);
		if (scenario === "extract-failed") throw new Error("extract failed");
		const binary = path.join(root, "squashfs-root/usr/bin/light-headless");
		await mkdir(path.dirname(binary), { recursive: true });
		if (scenario !== "missing")
			await writeFile(
				binary,
				scenario === "not-elf"
					? Buffer.from("not ELF")
					: elf("packaged-sidecar"),
				{ mode: 0o755 },
			);
		if (scenario === "changed") await writeFile(image, elf("mutated image"));
		if (scenario === "replaced") {
			await rename(image, `${image}.old`);
			await writeFile(image, elf("AppImage"), { mode: 0o700 });
		}
		if (scenario === "duplicate") {
			await mkdir(path.join(root, "other"));
			await writeFile(path.join(root, "other/light-headless"), elf("other"));
		}
		if (scenario === "symlink") {
			await rename(binary, `${binary}.real`);
			await symlink(`${binary}.real`, binary);
		}
		if (scenario === "ancestor-symlink") {
			await rename(path.dirname(binary), path.join(root, "real-bin"));
			await symlink(path.join(root, "real-bin"), path.dirname(binary));
		}
		if (scenario === "hardlink") await link(binary, `${binary}.hardlink`);
		if (scenario === "writable") await chmod(binary, 0o777);
	};
	return { base, image, extract };
}

test("packaged sidecar pin binds extracted bytes rather than raw build bytes", async () => {
	const f = await packageFixture();
	const filename = await recorder.pinPackagedHeadless(
		f.image,
		f.base,
		f.extract,
	);
	const value = JSON.parse(await readFile(filename));
	assert.equal(value.schema, 1);
	assert.equal((await lstat(filename)).mode & 0o777, 0o600);
	assert.equal(value.appimage.path, f.image);
	assert.equal(
		value.packaged.sha256,
		createHash("sha256").update(elf("packaged-sidecar")).digest("hex"),
	);
	assert.notEqual(value.appimage.sha256, value.packaged.sha256);
	assert.equal(
		value.packaged.path,
		path.join(path.dirname(filename), "squashfs-root/usr/bin/light-headless"),
	);
});

test("packaged pin rejects changed, replaced, ambiguous and redirected sources", async () => {
	for (const scenario of [
		"changed",
		"replaced",
		"duplicate",
		"symlink",
		"ancestor-symlink",
		"hardlink",
		"writable",
		"not-elf",
		"missing",
		"extract-failed",
	]) {
		const f = await packageFixture(scenario);
		await assert.rejects(
			recorder.pinPackagedHeadless(f.image, f.base, f.extract),
			undefined,
			scenario,
		);
	}
	const f = await packageFixture();
	const redirect = `${f.image}.link`;
	await symlink(f.image, redirect);
	await assert.rejects(
		recorder.pinPackagedHeadless(redirect, f.base, f.extract),
		/redirect/u,
	);
});

test("bounded extraction command uses only its private owned working directory", async () => {
	const root = await mkdtemp(path.join(scratch, "command-cwd-"));
	await chmod(root, 0o700);
	const output = await recorder.boundedCommand(
		root,
		"cwd-proof",
		process.execPath,
		["-e", "process.stdout.write(process.cwd())"],
		4096,
		undefined,
		undefined,
		2000,
		root,
	);
	assert.equal(output, root);
	await assert.rejects(
		recorder.boundedCommand(
			root,
			"escape",
			process.execPath,
			[],
			4096,
			undefined,
			undefined,
			2000,
			scratch,
		),
		/escaped private root/u,
	);
});

test("workflow pins the packaged sidecar only for the explicit stack diagnostic", async () => {
	const workflow = await readFile(
		new URL(
			"../.github/workflows/release-output-diagnostics.yml",
			import.meta.url,
		),
		"utf8",
	);
	assert.match(
		workflow,
		/if \[\[ "\$HEADLESS_STACKS" == 1 \]\]; then export NO_STRIP=1; fi/u,
	);
	assert.match(workflow, /'--pin-packaged', str\(apps\[0\]\.resolve\(\)\)/u);
	assert.match(
		workflow,
		/paths = \[apps\[0\], root \/ 'light-headless', root \/ 'viz-renderer'\]/u,
	);
	assert.match(workflow, /paths\.append\(headless\)/u);
	assert.match(workflow, /DIAGNOSTIC_HEADLESS=\{headless\.resolve\(\)\}/u);
});
