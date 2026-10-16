import * as recorderModule from "./desktop-performance-headless-stacks-recorder.mjs";
import assert from "node:assert/strict";
import crypto from "node:crypto";
import { EventEmitter } from "node:events";
import {
	chmod,
	link,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	rm,
	symlink,
	unlink,
	writeFile,
} from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import vm from "node:vm";
import { StringDecoder } from "node:string_decoder";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	privateDirectory,
	privateJson,
} from "./desktop-performance-headless-stacks-recorder.mjs";
import {
	emptyReport,
	exportReport,
	functionId,
	HeadlessStackCapture,
	LIMITS,
	PerfScriptParser,
	PrivateSink,
	recordArguments,
	selectPreflightMode,
	validatedServerBinding,
	validateHeadlessStackOptions,
	verifyIdentity,
} from "./desktop-performance-headless-stacks.mjs";

await mkdir(artifactPaths.tmp, { recursive: true });
const scratch = await mkdtemp(
	path.join(artifactPaths.tmp, "headless-stacks-tests-"),
);
after(() => rm(scratch, { recursive: true, force: true }));
const root = path.resolve(
	path.dirname(new URL(import.meta.url).pathname),
	"..",
);
const runnerSource = await readFile(
	path.join(root, "tools/run-desktop-performance.mjs"),
	"utf8",
);
const recorderSource = await readFile(
	path.join(root, "tools/desktop-performance-headless-stacks-recorder.mjs"),
	"utf8",
);
const moduleSource = await readFile(
	path.join(root, "tools/desktop-performance-headless-stacks.mjs"),
	"utf8",
);
const sha = (value) => crypto.createHash("sha256").update(value).digest("hex");
function sample(pid = 10, tid = 11, period = 20000000, symbol = "malloc") {
	return ` ${pid}/${tid} 1234.000000001: ${period} cpu-clock:u:\n  7fff1234 ${symbol} (/private/secret/server)\n  7fff2222 light_dynamics::programming::preparation::Preparer<O>::prepare_owner_sample (/private/server)\n\n`;
}
function parser(text, limits = LIMITS) {
	const value = new PerfScriptParser(10, limits);
	value.feed(text);
	value.finish();
	return value;
}
function cpuFixture() {
	const binding = {
		pid: 10,
		parent_pid: 9,
		start_ticks: 50,
		executable: "/private/server",
		ancestry: [
			{ pid: 10, parent_pid: 9, start_ticks: 50 },
			{ pid: 9, parent_pid: 1, start_ticks: 40 },
		],
	};
	return {
		mode: "off",
		rootPid: 9,
		bindings: new Map([[10, binding]]),
		observerFailureReason: 0,
		receipts: Array.from({ length: 61 }, () => ({ owned: [binding] })),
	};
}
function captureFixture(changes = {}) {
	const calls = [],
		reports = [],
		cpu = cpuFixture();
	let stops = 0;
	const recorder = {
		raw: { filename: "/private/perf.data", bytes: 500 },
		capped: false,
		finish: async () => {
			if (!stops++) calls.push("stop");
			return { endedEarly: false };
		},
	};
	const capture = new HeadlessStackCapture(
		{
			root: "/private",
			outputDirectory: "/numeric",
			preflight: { perf: "/installed/perf", privileged: true },
			binaryHash: sha("binary"),
			report: emptyReport(),
		},
		{
			writePrivate: async () => {
				calls.push("private-receipt");
			},
			verifyIdentity: async () => {
				calls.push("identity");
			},
			threads: async () => [10, 11],
			readFile: async () => Buffer.from("binary"),
			openRecorder: async () => {
				calls.push("start");
				return recorder;
			},
			decode: async () => {
				calls.push("decode");
				return parser(sample());
			},
			exportReport: async (_directory, report) => {
				calls.push("export");
				reports.push(structuredClone(report));
			},
			...changes,
		},
	);
	return { capture, cpu, calls, reports, recorder };
}

test("option is absent by default and refuses ON, worker comparisons and non-absolute bindings", () => {
	const valid = {
		"headless-stacks": "on",
		"typed-cpu-diagnostics": "off",
		"headless-stack-preflight": "/private/ready.json",
		"headless-stack-binary": "/release/headless",
	};
	validateHeadlessStackOptions(valid, {});
	for (const change of [
		{ "headless-stacks": "true" },
		{ "typed-cpu-diagnostics": "on" },
		{ "headless-stack-preflight": "relative" },
		{ "headless-stack-binary": "relative" },
	])
		assert.throws(() =>
			validateHeadlessStackOptions({ ...valid, ...change }, {}),
		);
	assert.throws(() =>
		validateHeadlessStackOptions(valid, { LIGHT_OUTPUT_WORKERS: "4" }),
	);
	const start = runnerSource.indexOf("function parseArguments(");
	const end = runnerSource.indexOf("\nasync function runCase(", start);
	const parse = vm.runInNewContext(
		`${runnerSource.slice(start, end)}; parseArguments`,
		{ validateHeadlessStackOptions },
	);
	const defaults = [
		"--application",
		"/app",
		"--output-dir",
		"/out",
		"--demo-show",
		"/demo",
	];
	assert.equal(parse(defaults)["headless-stacks"], undefined);
	assert.throws(() =>
		parse([
			...defaults,
			"--headless-stacks",
			"on",
			"--typed-cpu-diagnostics",
			"on",
		]),
	);
});

test("record argv targets one exact TGID, no inheritance, fixed rate/user stacks and an internal lifetime bound", () => {
	const args = recordArguments(123, 75);
	assert.deepEqual(args.slice(-5), [
		"--pid",
		"123",
		"--",
		"/usr/bin/sleep",
		"75",
	]);
	assert.ok(args.includes("--no-inherit"));
	assert.ok(args.includes("--control=fd:0,2"));
	assert.deepEqual(args.slice(0, 8), [
		"record",
		"--event",
		"cpu-clock:u",
		"--freq",
		"49",
		"--strict-freq",
		"--call-graph",
		"dwarf,8192",
	]);
	for (const illegal of ["-a", "--all-cpus", "--uid", "--realtime", "--cpu"])
		assert.ok(!args.includes(illegal));
	for (const pid of [0, -1, "123", 1.5, NaN])
		assert.throws(() => recordArguments(pid, 75));
	assert.throws(() => recordArguments(1, 76));
});

test("user preflight is preferred; permission failure uses only the approved privileged probe; both failures stay unavailable", async () => {
	const calls = [];
	assert.equal(
		(
			await selectPreflightMode(async (privileged) => {
				calls.push(privileged);
				return "proof";
			})
		).privileged,
		false,
	);
	assert.deepEqual(calls, [false]);
	calls.length = 0;
	const result = await selectPreflightMode(async (privileged) => {
		calls.push(privileged);
		if (!privileged)
			throw Object.assign(new Error("EACCES"), { permissionDenied: true });
		return "proof";
	});
	assert.equal(result.privileged, true);
	assert.deepEqual(calls, [false, true]);
	await assert.rejects(
		selectPreflightMode(async () => {
			throw new Error("installed tool unavailable");
		}),
		/unavailable/,
	);
});

test("sample parser preserves denominators, weighted periods and nonadditive inclusive counts without public strings", () => {
	const value = parser(
		sample() + sample(10, 12, 300, "unknown_private_symbol") + sample(900, 901),
	);
	assert.equal(value.report.samples, 2);
	assert.equal(value.report.foreign_samples, 1);
	assert.equal(value.report.weighted_period, 20000300);
	assert.equal(value.report.unknown_self_samples, 1);
	assert.equal(value.report.functions[12].self_samples, 1);
	assert.equal(value.report.functions[0].self_samples, 1);
	assert.equal(value.report.functions[1].inclusive_samples, 2);
	const json = JSON.stringify(value.report);
	assert.doesNotMatch(
		json,
		/\/private\/|secret|7fff|malloc|unknown_private_symbol|901/u,
	);
});

test("unknown and absent stacks differ from valid zero counters, while malformed/lost metadata remains visible", () => {
	assert.equal(parser("").report.samples, 0);
	const unresolved = parser(sample(10, 11, 1, "[unknown]"));
	assert.equal(unresolved.report.unresolved_frames, 1);
	const empty = parser("10/11 1.000: 0 cpu-clock:u:\n\n");
	assert.equal(empty.report.samples, 1);
	assert.equal(empty.report.stackless_samples, 1);
	assert.equal(empty.report.weighted_period, 0);
	assert.equal(
		parser("PERF_RECORD_LOST id: 0, lost: 7\n").report.lost_events,
		7,
	);
	assert.equal(
		parser("PERF_RECORD_LOST unrecognized shape\n").report.lost_events,
		null,
	);
	assert.equal(
		parser("unexpected private metadata\n").report.malformed_lines,
		1,
	);
});

test("sample, frame, decoded byte, line and numeric-overflow caps fail without admitting excess", () => {
	assert.throws(
		() => parser(sample() + sample(), { ...LIMITS, samples: 1 }),
		/sample cap/,
	);
	assert.throws(() => parser(sample(), { ...LIMITS, frames: 1 }), /frame cap/);
	assert.throws(() => parser(sample(), { ...LIMITS, raw: 10 }), /byte cap/);
	assert.throws(
		() => parser("x".repeat(50), { ...LIMITS, line: 10 }),
		/line cap/,
	);
	assert.throws(
		() => parser(sample(10, 11, Number.MAX_SAFE_INTEGER) + sample()),
		/overflow/,
	);
	const fragmented = new PerfScriptParser(10);
	for (const char of sample()) fragmented.feed(char);
	assert.equal(fragmented.finish().samples, 1);
});

test("raw writer enforces a hard byte cap, exclusive owned mode and no symlink/replacement/hardlink adoption", async () => {
	const directory = await mkdtemp(path.join(scratch, "raw-"));
	const filename = path.join(directory, "capture");
	let overflows = 0;
	const sink = new PrivateSink(filename, 8, () => overflows++);
	sink.write(Buffer.from("123456"));
	sink.write(Buffer.from("7890123"));
	sink.write(Buffer.from("extra"));
	sink.close();
	assert.equal(await readFile(filename, "utf8"), "12345678");
	assert.equal(overflows, 1);
	assert.equal((await lstat(filename)).mode & 0o777, 0o600);
	await sink.verify();
	assert.throws(() => new PrivateSink(filename, 8));
	const alias = path.join(directory, "alias");
	await symlink(filename, alias);
	assert.throws(() => new PrivateSink(alias, 8));
	await unlink(filename);
	await writeFile(filename, "replacement", { mode: 0o600 });
	await assert.rejects(sink.verify(), /identity changed/);
});

test("private writer rejects permissive mode and does not treat a modified file as verified", async () => {
	const filename = path.join(scratch, "mode");
	const sink = new PrivateSink(filename, 8);
	sink.write("abc");
	sink.close();
	await chmod(filename, 0o644);
	await assert.rejects(sink.verify());
});

test("only the current exact observer binding/receipt is adopted", () => {
	const cpu = cpuFixture();
	assert.equal(validatedServerBinding(cpu), cpu.bindings.get(10));
	for (const change of [
		{ mode: "on" },
		{ receipts: [] },
		{ rootPid: 500 },
		{ observerFailureReason: 5 },
		{ bindings: new Map() },
		{
			bindings: new Map([
				[10, cpu.bindings.get(10)],
				[11, cpu.bindings.get(10)],
			]),
		},
		{ receipts: [{ owned: [{ ...cpu.bindings.get(10), start_ticks: 51 }] }] },
	])
		assert.throws(() => validatedServerBinding({ ...cpu, ...change }));
});

test("fresh owned birth/executable/parent chain checks reject PID reuse, exec swap and hostile ancestry", async () => {
	const record = cpuFixture().bindings.get(10);
	const inspect = async (pid) =>
		pid === 10 ? { ...record } : { ...record.ancestry[1], executable: "/app" };
	await verifyIdentity(record, { inspect });
	for (const change of [
		{ start_ticks: 51 },
		{ parent_pid: 500 },
		{ executable: "/other" },
	])
		await assert.rejects(
			verifyIdentity(record, {
				inspect: async (pid) =>
					pid === 10 ? { ...record, ...change } : inspect(pid),
			}),
		);
	await assert.rejects(
		verifyIdentity(
			{ ...record, ancestry: [record.ancestry[0], record.ancestry[0]] },
			{ inspect },
		),
	);
	await assert.rejects(
		verifyIdentity({ ...record, ancestry: [] }, { inspect }),
	);
});

test("recorder stops and drains before parsing/export; valid numeric report completes only admitted existing-thread coverage", async () => {
	const f = captureFixture();
	assert.equal(
		await f.capture.run(f.cpu, async () => {
			f.calls.push("measure");
			return 123;
		}),
		123,
	);
	assert.ok(f.calls.indexOf("start") < f.calls.indexOf("measure"));
	assert.ok(f.calls.indexOf("measure") < f.calls.indexOf("stop"));
	assert.ok(f.calls.indexOf("stop") < f.calls.indexOf("decode"));
	assert.ok(f.calls.indexOf("decode") < f.calls.indexOf("export"));
	assert.equal(f.reports[0].capture_complete, 1);
	assert.equal(f.reports[0].future_thread_coverage, 0);
});

test("original benchmark failure survives recorder, decode and export failures, with stop attempted before return", async () => {
	for (const failure of ["none", "stop", "decode", "export"]) {
		const original = new Error("original throughput failure");
		const calls = [];
		const overrides =
			failure === "decode"
				? {
						decode: async () => {
							throw new Error("decode byte cap");
						},
					}
				: failure === "export"
					? {
							exportReport: async () => {
								throw new Error("ENOSPC");
							},
						}
					: {};
		const f = captureFixture(overrides);
		if (failure === "stop")
			f.recorder.finish = async () => {
				calls.push("stop");
				throw new Error("stop failed");
			};
		await assert.rejects(
			f.capture.run(f.cpu, async () => {
				throw original;
			}),
			(error) => error === original,
		);
		assert.ok(
			failure === "stop" ? calls.includes("stop") : f.calls.includes("stop"),
		);
		if (failure !== "export") assert.equal(f.reports[0].workload_failed, 1);
	}
});

test("unavailable attach, pinned executable mismatch and ownership loss cannot produce a complete report", async () => {
	for (const changes of [
		{
			openRecorder: async () => {
				throw new Error("permission denied");
			},
		},
		{ readFile: async () => Buffer.from("different binary") },
		{
			verifyIdentity: async () => {
				throw new Error("reused PID");
			},
		},
	]) {
		const f = captureFixture(changes);
		let measured = false;
		await assert.rejects(
			f.capture.run(f.cpu, async () => {
				measured = true;
			}),
		);
		assert.equal(measured, false);
		assert.equal(f.reports[0].capture_complete, 0);
	}
});

test("late/missing threads, short receipts, raw cap, loss, unknown/unresolved or empty evidence remain partial", async () => {
	const variants = [
		{ threads: async () => [10, 11, 12] },
		{ decode: async () => parser("") },
		{ decode: async () => parser(sample(10, 11, 1, "unknown_function")) },
		{ decode: async () => parser(sample(10, 11, 1, "[unknown]")) },
		{
			decode: async () =>
				parser(sample() + "PERF_RECORD_LOST id: 0, lost: 2\n"),
		},
	];
	for (let index = 0; index < variants.length; index++) {
		let reads = 0;
		const changes = { ...variants[index] };
		if (index === 0)
			changes.threads = async () => (++reads === 1 ? [10, 11] : [10, 11, 12]);
		const f = captureFixture(changes);
		await assert.rejects(f.capture.run(f.cpu, async () => {}));
		assert.equal(f.reports[0].capture_complete, 0);
	}
	for (const kind of ["missing", "receipts", "cap", "early"]) {
		let reads = 0;
		const f = captureFixture(
			kind === "missing"
				? { threads: async () => (++reads === 1 ? [10, 11] : [10]) }
				: {},
		);
		if (kind === "receipts") f.cpu.receipts.pop();
		if (kind === "cap") f.recorder.capped = true;
		if (kind === "early")
			f.recorder.finish = async () => ({ endedEarly: true });
		await assert.rejects(f.capture.run(f.cpu, async () => {}));
		assert.equal(f.reports[0].capture_complete, 0);
	}
});

test("numeric output roundtrip rejects unknown fields, raw strings, duplicated IDs, excess counts and malformed denominators", async () => {
	const report = parser(sample()).report;
	const directory = await mkdtemp(path.join(scratch, "public-"));
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
	const invalid = [
		{ ...report, private_path: 1 },
		{ ...report, failure_code: "/private/token" },
		{ ...report, samples: LIMITS.samples + 1 },
		{ ...report, raw_bytes: LIMITS.raw + 1 },
		{ ...report, capture_complete: 2 },
		{ ...report, schema_version: 1 },
		{ ...report, multi_frame_samples: report.samples + 1 },
		{ ...report, weighted_period: 0 },
		{
			...report,
			functions: report.functions.map((row, id) =>
				id === 1 ? { ...row, id: 0 } : row,
			),
		},
		{
			...report,
			functions: report.functions.map((row, id) =>
				id === 1 ? { ...row, raw_symbol: 1 } : row,
			),
		},
	];
	for (const value of invalid)
		await assert.rejects(exportReport(directory, value));
	await assert.rejects(exportReport(directory, report), /EEXIST/);
});

test("production helper program parses /proc-format thread ID fixtures and creates the late thread only after control", () => {
	const start = moduleSource.indexOf("const HELPER_WORKER = ");
	const end = moduleSource.indexOf("\nasync function message(", start);
	const code = vm.runInNewContext(`${moduleSource.slice(start, end)}; HELPER`);
	const workers = [],
		sent = [],
		intervals = [];
	class Worker {
		constructor(value) {
			workers.push(value);
		}
		once(_event, callback) {
			callback(101);
		}
	}
	let late;
	vm.runInNewContext(code, {
		require: (name) =>
			name === "node:worker_threads"
				? { Worker }
				: { readFileSync: () => "Pid:\t100\n" },
		setInterval: (callback) => intervals.push(callback),
		process: {
			send: (value) => sent.push(value),
			on: (_name, cb) => {
				late = cb;
			},
		},
	});
	assert.equal(workers.length, 1);
	assert.equal(sent[0].tid, 101);
	assert.equal(sent[0].main, 100);
	late();
	assert.equal(workers.length, 2);
	assert.equal(sent[1].late, true);
	let workerTid;
	vm.runInNewContext(workers[0], {
		require: (name) =>
			name === "node:worker_threads"
				? {
						parentPort: {
							postMessage: (id) => {
								workerTid = id;
							},
						},
					}
				: { readFileSync: () => "Pid:\t101\n" },
		setInterval: () => {},
	});
	assert.equal(workerTid, 101);
});

test("workflow remains opt-in OFF-only; raw paths are absent from artifact allowlist and preflight precedes build", async () => {
	const workflow = await readFile(
		path.join(root, ".github/workflows/release-output-diagnostics.yml"),
		"utf8",
	);
	assert.match(workflow, /headless_stacks:[\s\S]*?default: false/u);
	assert.ok(
		workflow.indexOf("Preflight opt-in") <
			workflow.indexOf("Build pinned canonical"),
	);
	assert.match(
		workflow,
		/if \[\[ "\$HEADLESS_STACKS" == 1 \]\]; then observers=off; fi/u,
	);
	const upload = workflow.slice(workflow.indexOf("      - name: Publish only"));
	assert.match(upload, /off\/headless-stacks-numeric\.json/u);
	assert.doesNotMatch(
		upload,
		/\.data|\.stderr|\.stdout|private|headless-stacks-\*/u,
	);
});

function recorderHarness({
	acknowledgePing = true,
	failIdentity = false,
	failWrite = false,
} = {}) {
	const calls = [],
		sinks = [];
	const child = new EventEmitter();
	Object.assign(child, {
		pid: 333,
		exitCode: null,
		signalCode: null,
		stdout: new EventEmitter(),
		stderr: new EventEmitter(),
		stdin: new EventEmitter(),
	});
	child.stdin.write = (command) => {
		calls.push(command.trim());
		if (command === "ping\n" && !acknowledgePing)
			child.stderr.emit("data", Buffer.from("ack extra\nnotack\n"));
		else child.stderr.emit("data", Buffer.from("ack\n"));
		if (command === "stop\n" && child.exitCode === null) {
			child.exitCode = 0;
			calls.push("child-exit");
			child.emit("close", 0, null);
		}
	};
	class Sink {
		constructor(filename, cap, overflow) {
			Object.assign(this, { filename, cap, overflow, bytes: 0 });
			sinks.push(this);
		}
		write(chunk) {
			if (failWrite && this.filename.endsWith(".data"))
				throw new Error("ENOSPC");
			this.bytes += chunk.length;
		}
		close() {
			calls.push("file-close");
		}
		async verify() {
			calls.push("file-verify");
		}
	}
	const start = recorderSource.indexOf("export async function openRecorder(");
	const end = recorderSource.indexOf(
		"\nexport async function boundedCommand(",
		start,
	);
	const open = vm.runInNewContext(
		`${recorderSource.slice(start, end).replace("export async", "async")}; openRecorder`,
		{
			verifyIdentity: async () => {
				calls.push("binding-verify");
			},
			privateDirectory: async () => {},
			PrivateSink: Sink,
			path,
			LIMITS,
			Buffer,
			process,
			recordArguments,
			recorderFailureDetails: recorderModule.recorderFailureDetails,
			spawn: (command, args) => {
				calls.push({ command, args });
				return child;
			},
			until: async (check) => {
				if (!check()) throw new Error("control acknowledgment timeout");
			},
			recorderDescendants: async (_child, _perf, privileged) => {
				calls.push(`descendant-${privileged}`);
				if (failIdentity) throw new Error("recorder birth changed");
				return [{ pid: 334, start_ticks: 55 }];
			},
			requireGone: async () => {
				assert.equal(child.exitCode, 0);
				calls.push("recorder-gone");
			},
			require_: (value, reason) => {
				if (!value) throw new Error(reason);
			},
			setTimeout,
			clearTimeout,
			readFile: async () => "",
			privateText: async () => "",
		},
	);
	return { calls, child, sinks, open };
}

test("actual recorder control path binds exact privileged argv, accepts only strict acks and proves recorder gone before completion", async () => {
	const f = recorderHarness();
	const recorder = await f.open({
		root: "/private",
		name: "test",
		perf: "/installed/perf",
		privileged: true,
		binding: { pid: 10 },
		seconds: 3,
	});
	const invocation = f.calls.find((item) => typeof item === "object");
	assert.equal(invocation.command, "/usr/bin/sudo");
	assert.deepEqual(Array.from(invocation.args.slice(0, 3)), [
		"-n",
		"--",
		"/installed/perf",
	]);
	assert.ok(invocation.args.includes("10"));
	assert.deepEqual(
		f.calls.filter(
			(item) => typeof item === "string" && ["ping", "enable"].includes(item),
		),
		["ping", "enable"],
	);
	const stopped = await recorder.finish();
	await recorder.finish();
	assert.equal(stopped.ack, 3);
	assert.equal(f.calls.filter((item) => item === "stop").length, 1);
	assert.ok(f.calls.indexOf("child-exit") < f.calls.indexOf("recorder-gone"));
	assert.ok(f.calls.includes("descendant-true"));
});

test("actual recorder startup/control/identity/write failures stop the owned child without adopting another process", async () => {
	for (const options of [
		{ acknowledgePing: false },
		{ failIdentity: true },
		{ failWrite: true },
	]) {
		const f = recorderHarness(options);
		if (options.failWrite) {
			const recorder = await f.open({
				root: "/private",
				name: "test",
				perf: "/installed/perf",
				privileged: false,
				binding: { pid: 10 },
				seconds: 3,
			});
			f.child.stdout.emit("data", Buffer.from("sample"));
			await assert.rejects(recorder.finish(), /record command failed/);
		} else {
			await assert.rejects(
				f.open({
					root: "/private",
					name: "test",
					perf: "/installed/perf",
					privileged: false,
					binding: { pid: 10 },
					seconds: 3,
				}),
			);
		}
		assert.equal(f.child.exitCode, 0);
		assert.ok(f.calls.includes("stop"));
		assert.ok(f.calls.includes("recorder-gone"));
	}
});

test("production 60-poll window enters stack scope after the opening ownership observation and stops before aggregation", async () => {
	const start = runnerSource.indexOf("async function measureWindow(");
	const end = runnerSource.indexOf("\nfunction apiClient(", start);
	let clock = 0,
		requests = 0,
		observations = 0;
	const calls = [],
		samples = [];
	const measure = vm.runInNewContext(
		`${runnerSource.slice(start, end)}; measureWindow`,
		{
			DURATION_SECONDS: 60,
			performance: { now: () => clock },
			Date,
			setTimeout: (callback) => {
				clock += 1000;
				callback();
			},
			readLinuxProcessTree: async () => [],
			processTreeResourceDelta: () => ({ cpuPercent: 0, residentBytes: 0 }),
			LINUX_PROCESS_OPTIONS: {},
			average: (values) => {
				calls.push("aggregate");
				return values.reduce((sum, value) => sum + value, 0) / values.length;
			},
			percentile: (values) => values[0],
		},
	);
	const cpu = {
		observe: async () => {
			observations++;
			calls.push("observe");
		},
		requestStart: () => ({}),
		requestEnd: () => {},
		headlessStacks: {
			run: async (_cpu, window) => {
				assert.equal(observations, 1);
				calls.push("stack-start");
				try {
					return await window();
				} finally {
					calls.push("stack-stop");
				}
			},
		},
	};
	const result = await measure(
		{
			request: async () => ({
				output: {
					frames_sent: 40 * requests++,
					send_errors: 0,
					frame_rate_band_counts: [0],
				},
			}),
		},
		9,
		"unrestricted",
		samples,
		cpu,
	);
	assert.equal(requests, 61);
	assert.equal(observations, 61);
	assert.equal(samples.length, 61);
	assert.equal(result.average_completed_hz, 40);
	assert.ok(calls.indexOf("stack-stop") < calls.indexOf("aggregate"));
});

test("private root/receipt guards reject redirects, oversized receipts and hardlink mutations", async () => {
	const directory = await mkdtemp(path.join(scratch, "guards-"));
	await privateDirectory(directory);
	const alias = path.join(scratch, "root-link");
	await symlink(directory, alias);
	await assert.rejects(privateDirectory(alias));
	await chmod(directory, 0o755);
	await assert.rejects(privateDirectory(directory));
	await chmod(directory, 0o700);
	const receipt = path.join(directory, "receipt.json");
	await writeFile(receipt, JSON.stringify({ valid: 1 }), { mode: 0o600 });
	assert.deepEqual(await privateJson(receipt), { valid: 1 });
	await link(receipt, path.join(directory, "hardlink"));
	await assert.rejects(privateJson(receipt));
	const oversized = path.join(directory, "oversized");
	await writeFile(oversized, "x".repeat(65537), { mode: 0o600 });
	await assert.rejects(privateJson(oversized));
	const filename = path.join(directory, "hardlinked-raw");
	const sink = new PrivateSink(filename, 8);
	sink.write("abc");
	sink.close();
	await link(filename, path.join(directory, "raw-alias"));
	await assert.rejects(sink.verify());
});

function descendantHarness({
	changedBirth = false,
	changedExecutable = false,
	wrongParent = false,
} = {}) {
	const records = new Map([
		[9, { pid: 9, parentPid: 1, startTicks: 90 }],
		[333, { pid: 333, parentPid: wrongParent ? 500 : 9, startTicks: 100 }],
		[334, { pid: 334, parentPid: 333, startTicks: 101 }],
	]);
	const reads = [];
	const start = recorderSource.indexOf("async function recorderDescendants(");
	const end = recorderSource.indexOf("\nexport async function threads(", start);
	let perfReads = 0;
	const discover = vm.runInNewContext(
		`${recorderSource.slice(start, end)}; recorderDescendants`,
		{
			process: { pid: 9 },
			processIdentity: async () => ({
				pid: 9,
				parent_pid: 1,
				start_ticks: 90,
				executable: "/node",
			}),
			parseLinuxProcessStat: JSON.parse,
			readFile: async (filename) =>
				filename.endsWith("children")
					? "334"
					: JSON.stringify(records.get(Number(filename.split("/")[2]))),
			readlink: async () => {
				throw Object.assign(new Error("root process"), { code: "EACCES" });
			},
			execFile: (command, args, _options, callback) => {
				reads.push({ command, args: Array.from(args) });
				const pid = Number(args.at(-1).split("/")[2]);
				assert.equal(command, "/usr/bin/sudo");
				assert.deepEqual(Array.from(args.slice(0, 5)), [
					"-n",
					"--",
					"/usr/bin/readlink",
					"--",
					`/proc/${pid}/exe`,
				]);
				if (pid === 334 && changedBirth) records.get(334).startTicks++;
				const executable =
					pid === 333
						? "/usr/bin/sudo"
						: changedExecutable && ++perfReads > 1
							? "/other"
							: "/installed/perf";
				callback(null, `${executable}\n`);
			},
			require_: (value, message) => {
				if (!value) throw new Error(message);
			},
		},
	);
	return { discover, reads };
}

test("narrow privileged readlink follows only the owned recorder chain and repeats birth/executable checks", async () => {
	const f = descendantHarness();
	const records = await f.discover({ pid: 333 }, "/installed/perf", true);
	assert.equal(records.length, 1);
	assert.equal(records[0].pid, 334);
	assert.equal(f.reads.length, 4);
	for (const options of [
		{ changedBirth: true },
		{ changedExecutable: true },
		{ wrongParent: true },
	]) {
		const hostile = descendantHarness(options);
		await assert.rejects(
			hostile.discover({ pid: 333 }, "/installed/perf", true),
		);
		if (options.wrongParent) assert.equal(hostile.reads.length, 0);
	}
	const user = descendantHarness();
	await assert.rejects(user.discover({ pid: 333 }, "/installed/perf", false));
	assert.equal(user.reads.length, 0);
});

test("actual decoder uses a bound descriptor and preserves UTF-8 across output chunks", async () => {
	const start = moduleSource.indexOf("async function decode(");
	const end = moduleSource.indexOf("\nconst HELPER_WORKER", start);
	class ObservedParser extends PerfScriptParser {
		constructor(pid) {
			super(pid);
			this.decoded = "";
		}
		feed(chunk) {
			this.decoded += chunk;
			super.feed(chunk);
		}
	}
	let decoderStderr = "";
	const decode = vm.runInNewContext(
		`${moduleSource.slice(start, end)}; decode`,
		{
			PerfScriptParser: ObservedParser,
			StringDecoder,
			path,
			privateText: async () => decoderStderr,
			LIMITS,
			boundedCommand: async (
				_root,
				_name,
				_perf,
				args,
				_cap,
				onOutput,
				filename,
			) => {
				assert.deepEqual(Array.from(args.slice(0, 3)), [
					"script",
					"--input",
					"/proc/self/fd/3",
				]);
				assert.equal(filename, "/private/headless.data");
				const bytes = Buffer.from(sample(10, 11, 1, "function_ç"));
				const split = bytes.indexOf(Buffer.from("ç")) + 1;
				onOutput(bytes.subarray(0, split));
				onOutput(bytes.subarray(split));
			},
		},
	);
	const result = await decode(
		"/private",
		"script",
		"/installed/perf",
		"/private/headless.data",
		10,
	);
	assert.equal(result.report.samples, 1);
	assert.equal(result.report.multi_frame_samples, 1);
	assert.match(result.decoded, /function_ç/u);
	assert.doesNotMatch(result.decoded, /�/u);
	decoderStderr = "perf warning: records may be lost";
	const warned = await decode(
		"/private",
		"script",
		"/installed/perf",
		"/private/headless.data",
		10,
	);
	assert.equal(warned.report.decode_warnings, 1);
	assert.equal(warned.report.lost_events, null);
});

test("real capture creation fails before workload for missing CFI/symbols or stale/hostile preflight receipts", async () => {
	const start = moduleSource.indexOf(
		"export async function createHeadlessStackCapture(",
	);
	const end = moduleSource.indexOf(
		"\nexport class HeadlessStackCapture",
		start,
	);
	const options = {
		"headless-stacks": "on",
		"typed-cpu-diagnostics": "off",
		"headless-stack-preflight": "/private/ready.json",
		"headless-stack-binary": "/release/headless",
	};
	for (const variant of [
		"valid",
		"CFI",
		"symbols",
		"empty-proof",
		"stale",
		"extra",
	]) {
		const reports = [],
			commands = [];
		const receipt = {
			schema: 1,
			created_ms: Date.now(),
			uid: process.getuid(),
			boot_id: "boot",
			perf: "/installed/perf",
			perf_hash: "hash",
			privileged: true,
			proof: {
				existing_threads_verified: 1,
				late_threads_excluded: 1,
				unwind_verified: 1,
				lifetime_bound_verified: 1,
				control_stop_verified: 1,
			},
		};
		if (variant === "empty-proof") receipt.proof = {};
		if (variant === "stale") receipt.created_ms -= 3600001;
		if (variant === "extra") receipt.private_unknown = 1;
		const create = vm.runInNewContext(
			`${moduleSource.slice(start, end).replace("export async", "async")}; createHeadlessStackCapture`,
			{
				validateHeadlessStackOptions,
				mkdtemp: async () => "/private/owned",
				chmod: async () => {},
				path,
				emptyReport,
				privateDirectory: async () => {},
				privateJson: async () => receipt,
				process,
				Date,
				readFile: async (name) =>
					name.startsWith("/proc/") ? "boot" : Buffer.from("binary"),
				verifiedTool: async () => "hash",
				hash: sha,
				HeadlessStackCapture,
				require_: (value, message) => {
					if (!value) throw new Error(message);
				},
				boundedCommand: async (_root, name) => {
					commands.push(name);
					return name === "binary-sections"
						? variant === "CFI"
							? ".symtab"
							: ".symtab .eh_frame"
						: variant === "symbols"
							? "0000 T malloc"
							: "0000 T light_dynamics::programming::preparation::Preparer<O>::prepare_owner_sample";
				},
				functionId,
				exportReport: async (_directory, report) => reports.push(report),
			},
		);
		if (variant === "valid")
			assert.equal(
				(
					await create(
						{ root: "/private", outputDirectory: "/public" },
						options,
					)
				).report.symbols_available,
				1,
			);
		else {
			await assert.rejects(
				create({ root: "/private", outputDirectory: "/public" }, options),
			);
			assert.equal(reports[0].failure_code, 2);
			assert.equal(reports[0].recording_started, 0);
			if (["empty-proof", "stale", "extra"].includes(variant))
				assert.equal(commands.length, 0);
		}
	}
});

test("actual helper preflight cleans its owned helper even when memoized recorder finish rejects", async () => {
	const start = moduleSource.indexOf("async function helperPreflight(");
	const end = moduleSource.indexOf("\nasync function verifiedTool", start);
	for (const controlFails of [false, true]) {
		const failure = new Error(
			controlFails ? "control failure" : "bound failure",
		);
		const calls = [];
		const child = {
			exitCode: null,
			signalCode: null,
			send: () => {},
			kill: (signal) => calls.push(signal),
		};
		let opened = 0;
		const preflight = vm.runInNewContext(
			`${moduleSource.slice(start, end)}; helperPreflight`,
			{
				ownedHelper: async () => ({
					child,
					binding: { pid: 10 },
					initial: { tid: 11, main: 10 },
					closed: Promise.resolve(),
				}),
				openRecorder: async () => {
					const index = opened++;
					return {
						raw: { filename: "/private/raw" },
						recorderBirths: [],
						finish: async () => {
							calls.push(`finish${index}`);
							if (!controlFails || index === 1) throw failure;
							return { ack: 3, capped: false };
						},
					};
				},
				message: async () => ({ tid: 12 }),
				decode: async () => ({
					report: {
						samples: 2,
						multi_frame_samples: 2,
						malformed_lines: 0,
						decode_warnings: 0,
					},
					resolved: 2,
					tids: new Set([10, 11]),
				}),
				require_: (ok, message) => {
					if (!ok) throw new Error(message);
				},
				verifyIdentity: async () => calls.push("identity"),
				requireGone: async () => calls.push("gone"),
				writeFile: async () => calls.push("proof"),
				path,
			},
		);
		await assert.rejects(
			preflight("/private", "/installed/perf", false),
			(error) => error === failure,
		);
		assert.deepEqual(calls.slice(-4), ["identity", "SIGTERM", "gone", "proof"]);
		assert.equal(opened, controlFails ? 2 : 1);
	}
});
