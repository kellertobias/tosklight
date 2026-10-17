import { spawn } from "node:child_process";
import crypto from "node:crypto";
import {
	chmod,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	realpath,
	writeFile,
} from "node:fs/promises";
import path from "node:path";
import { StringDecoder } from "node:string_decoder";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import { verifyRuntimeSymbols } from "./desktop-performance-runtime-symbols.mjs";
import { assertNumericOnly } from "./desktop-performance-cpu-stages.mjs";
import {
	LIMITS,
	boundedCommand,
	openRecorder,
	pinPackagedHeadless,
	privateDirectory,
	privateJson,
	privateText,
	processIdentity,
	requireGone,
	threads,
	verifyIdentity,
} from "./desktop-performance-headless-stacks-recorder.mjs";
export {
	LIMITS,
	PrivateSink,
	recordArguments,
	verifyIdentity,
} from "./desktop-performance-headless-stacks-recorder.mjs";

import {
	FUNCTION_MAPPING_VERSION,
	functionRows,
	functionId,
	emptyCoverage,
	observeCoverage,
	validateCoverage,
} from "./desktop-performance-headless-stack-coverage.mjs";
export { functionId } from "./desktop-performance-headless-stack-coverage.mjs";

const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const require_ = (condition, message) => {
	if (!condition) throw new Error(`Headless stacks unavailable: ${message}`);
};
const uint = (value) => {
	require_(Number.isSafeInteger(value) && value >= 0, "numeric overflow");
	return value;
};
const add = (a, b) => uint(a + b);

export function emptyReport() {
	return {
		schema_version: 5,
		function_mapping_version: FUNCTION_MAPPING_VERSION,
		requested_frequency_hz: 49,
		preflight_available: 0,
		symbols_available: 0,
		unwind_available: 0,
		recording_started: 0,
		capture_complete: 0,
		failure_code: 0,
		pre_record_setup_stage: 0,
		pre_record_binary_match: null,
		record_stage: 0,
		record_exit_code: null,
		record_signal: null,
		record_privileged: null,
		record_spawn_error: null,
		record_capture_error: null,
		record_capped: null,
		record_stop_requested: null,
		record_stop_acknowledged: null,
		record_disable_requested: null,
		record_disable_acknowledged: null,
		record_ack_count: null,
		record_ended_early: null,
		record_stderr_class: null,
		cap_code: 0,
		existing_threads_only: 1,
		future_thread_coverage: 0,
		transient_thread_coverage: 0,
		recorder_ended_early: 0,
		workload_failed: 0,
		per_frame_join_available: 0,
		sampling_overhead_present: 1,
		privileged: 0,
		recording_wall_ns: null,
		active_sampling_wall_ns: null,
		recording_wall_includes_disabled_tail: 1,
		disable_boundary_verified: 0,
		initial_threads: null,
		final_threads: null,
		late_threads: null,
		missing_threads: null,
		samples: 0,
		foreign_samples: 0,
		unknown_self_samples: 0,
		stackless_samples: 0,
		multi_frame_samples: 0,
		unresolved_frames: 0,
		depth_capped_samples: 0,
		malformed_lines: 0,
		decode_warnings: 0,
		lost_events: 0,
		weighted_period: 0,
		raw_bytes: 0,
		functions: functionRows(),
		coverage: emptyCoverage(),
	};
}
export function validateHeadlessStackOptions(
	values,
	environment = process.env,
) {
	require_(
		values["headless-stacks"] === "on",
		"--headless-stacks must be on when supplied",
	);
	require_(
		values["typed-cpu-diagnostics"] === "off",
		"requires exactly the existing OFF576 cohort",
	);
	require_(
		!environment.LIGHT_OUTPUT_WORKERS,
		"worker comparisons are not supported",
	);
	for (const option of ["headless-stack-preflight", "headless-stack-binary"])
		require_(
			path.isAbsolute(values[option] ?? ""),
			`missing absolute --${option}`,
		);
}

export class PerfScriptParser {
	constructor(tgid, limits = LIMITS, ownedExecutable = null) {
		require_(
			ownedExecutable === null ||
				(typeof ownedExecutable === "string" &&
					path.isAbsolute(ownedExecutable)),
			"owned executable path",
		);
		this.ownedExecutable = ownedExecutable;
		this.tgid = tgid;
		this.limits = limits;
		this.report = emptyReport();
		this.buffer = "";
		this.bytes = 0;
		this.current = null;
		this.tids = new Set();
		this.resolved = 0;
		this.minimumSampleNs = null;
		this.maximumSampleNs = null;
	}
	feed(chunk) {
		this.bytes += Buffer.byteLength(chunk);
		require_(this.bytes <= this.limits.raw, "decoded byte cap");
		this.buffer += chunk;
		let newline;
		while ((newline = this.buffer.indexOf("\n")) >= 0) {
			this.line(this.buffer.slice(0, newline));
			this.buffer = this.buffer.slice(newline + 1);
		}
		require_(this.buffer.length <= this.limits.line, "line cap");
	}
	line(line) {
		require_(line.length <= this.limits.line, "line cap");
		if (!line.trim()) {
			this.flush();
			return;
		}
		if (line.startsWith("#")) return;
		if (line.includes("PERF_RECORD_LOST")) {
			this.flush();
			const match = /\blost:\s*(\d+)/u.exec(line);
			if (match && this.report.lost_events !== null)
				this.report.lost_events = add(
					this.report.lost_events,
					uint(Number(match[1])),
				);
			else {
				this.report.lost_events = null;
				this.report.malformed_lines++;
			}
			return;
		}
		const header =
			/^\s*(\d+)\/(\d+)\s+(\d+\.\d+):\s+(\d+)\s+cpu-clock(?::u)?:\s*(.*)$/u.exec(
				line,
			);
		if (header) {
			this.flush();
			require_(
				this.report.samples + this.report.foreign_samples < this.limits.samples,
				"sample cap",
			);
			const [seconds, fraction] = header[3].split(".");
			require_(fraction.length <= 9, "sample timestamp precision");
			this.current = {
				timeNs: BigInt(seconds) * 1000000000n + BigInt(fraction.padEnd(9, "0")),
				pid: uint(Number(header[1])),
				tid: uint(Number(header[2])),
				period: uint(Number(header[4])),
				frames: [],
			};
			if (header[5]) this.frame(header[5]);
			return;
		}
		if (this.current) this.frame(line);
		else this.report.malformed_lines++;
	}
	frame(line) {
		const frame = /^\s*(?:0x)?[0-9a-f]+\s+(.+)\s+\((.*)\)\s*$/iu.exec(line);
		if (!frame) {
			this.report.malformed_lines++;
			return;
		}
		require_(this.current.frames.length < this.limits.frames, "frame cap");
		this.current.frames.push({ symbol: frame[1].trim(), dso: frame[2].trim() });
	}
	flush() {
		const sample = this.current;
		this.current = null;
		if (!sample) return;
		if (sample.pid !== this.tgid) {
			this.report.foreign_samples++;
			return;
		}
		this.minimumSampleNs =
			this.minimumSampleNs === null || sample.timeNs < this.minimumSampleNs
				? sample.timeNs
				: this.minimumSampleNs;
		this.maximumSampleNs =
			this.maximumSampleNs === null || sample.timeNs > this.maximumSampleNs
				? sample.timeNs
				: this.maximumSampleNs;
		this.report.samples++;
		this.tids.add(sample.tid);
		this.report.weighted_period = add(
			this.report.weighted_period,
			sample.period,
		);
		if (!sample.frames.length) this.report.stackless_samples++;
		if (sample.frames.length >= 2) this.report.multi_frame_samples++;
		if (sample.frames.length === this.limits.frames)
			this.report.depth_capped_samples++;
		this.resolved = add(
			this.resolved,
			observeCoverage(
				this.report,
				sample.frames,
				sample.period,
				this.ownedExecutable,
			),
		);
	}
	finish() {
		if (this.buffer) this.line(this.buffer);
		this.buffer = "";
		this.flush();
		return this.report;
	}
}

export function validatedServerBinding(cpu) {
	require_(
		cpu?.mode === "off" &&
			cpu.bindings?.size === 1 &&
			cpu.receipts?.length > 0 &&
			!cpu.observerFailureReason,
		"validated OFF server unavailable",
	);
	const binding = [...cpu.bindings.values()][0];
	require_(
		cpu.receipts
			.at(-1)
			.owned.some((entry) => JSON.stringify(entry) === JSON.stringify(binding)),
		"receipt/binding mismatch",
	);
	require_(
		binding.ancestry.at(-1)?.pid === cpu.rootPid,
		"binding is outside owned application",
	);
	return binding;
}

async function decode(
	root,
	name,
	perf,
	filename,
	tgid,
	ownedExecutable = null,
) {
	const parser = new PerfScriptParser(tgid, LIMITS, ownedExecutable);
	const decoder = new StringDecoder("utf8");
	// Raw file is caller-owned/private and inode-validated immediately before and after decoding.
	await boundedCommand(
		root,
		name,
		perf,
		[
			"script",
			"--input",
			"/proc/self/fd/3",
			"--ns",
			"--show-lost-events",
			"--max-stack",
			"128",
			"--no-inline",
			"--fields",
			"pid,tid,time,period,event,ip,sym,dso",
		],
		LIMITS.raw,
		(chunk) => parser.feed(decoder.write(chunk)),
		filename,
	);
	if ((await privateText(path.join(root, `${name}.stderr`))).trim()) {
		parser.report.decode_warnings = 1;
		parser.report.lost_events = null;
	}
	parser.feed(decoder.end());
	parser.finish();
	return parser;
}
const HELPER_WORKER = String.raw`
const { parentPort } = require("node:worker_threads");
const fs = require("node:fs");
const tid = Number(/^Pid:\s+(\d+)/m.exec(fs.readFileSync("/proc/thread-self/status", "utf8"))[1]);
parentPort.postMessage(tid);
setInterval(() => {
  const end = Date.now() + 12;
  while (Date.now() < end) Math.sqrt(Date.now());
}, 20);
`;
const HELPER = String.raw`
const { Worker } = require("node:worker_threads");
const fs = require("node:fs");
const tid = () => Number(/^Pid:\s+(\d+)/m.exec(fs.readFileSync("/proc/thread-self/status", "utf8"))[1]);
const start = (late) => {
  const worker = new Worker(${JSON.stringify(HELPER_WORKER)}, { eval: true });
  worker.once("message", (id) => process.send({ late, tid: id, main: tid() }));
};
start(false);
setInterval(() => {
  const end = Date.now() + 12;
  while (Date.now() < end) Math.sqrt(Date.now());
}, 20);
process.on("message", () => start(true));
`;
async function message(child) {
	return new Promise((resolve, reject) => {
		const timer = setTimeout(() => {
			child.removeListener("message", received);
			reject(new Error("helper readiness timeout"));
		}, 3000);
		const received = (value) => {
			clearTimeout(timer);
			resolve(value);
		};
		child.once("message", received);
	});
}
async function ownedHelper() {
	const child = spawn(process.execPath, ["-e", HELPER], {
		stdio: ["ignore", "ignore", "ignore", "ipc"],
	});
	const closed = new Promise((resolve) => child.once("close", resolve));
	child.on("error", () => {});
	let binding;
	try {
		const initial = await message(child);
		const identity = await processIdentity(child.pid),
			parent = await processIdentity(process.pid);
		require_(
			identity.parent_pid === process.pid &&
				identity.executable === (await realpath(process.execPath)),
			"helper ownership",
		);
		binding = { ...identity, ancestry: [identity, parent] };
		return { child, initial, binding, closed };
	} catch (error) {
		if (binding) await verifyIdentity(binding);
		else if (child.pid && child.exitCode === null) {
			const identity = await processIdentity(child.pid);
			require_(
				identity.parent_pid === process.pid &&
					identity.executable === (await realpath(process.execPath)),
				"failed helper ownership",
			);
		}
		if (child.pid && child.exitCode === null && !child.signalCode)
			child.kill("SIGTERM");
		await closed;
		throw error;
	}
}
export async function verifyMonotonicClock(
	root,
	command = boundedCommand,
	name = "monotonic-clock",
) {
	const before = process.hrtime.bigint();
	const text = await command(
		root,
		name,
		"/usr/bin/python3",
		["-c", "import time; print(time.clock_gettime_ns(time.CLOCK_MONOTONIC))"],
		4096,
		undefined,
		undefined,
		2000,
	);
	const after = process.hrtime.bigint();
	require_(/^\d+\n$/u.test(text), "monotonic clock response");
	const python = BigInt(text.trim());
	require_(
		python >= before && python <= after,
		"monotonic clock epoch or units differ",
	);
	return {
		before_ns: String(before),
		python_ns: String(python),
		after_ns: String(after),
	};
}
export function validateDisabledSamples(parsed, stopped, minimumTailNs = 0n) {
	require_(
		stopped.diagnostics?.record_disable_acknowledged === 1 &&
			typeof stopped.disableAckNs === "bigint" &&
			typeof stopped.closedNs === "bigint",
		"disable clock receipt unavailable",
	);
	require_(
		parsed.report.samples > 0 &&
			!parsed.report.malformed_lines &&
			!parsed.report.decode_warnings &&
			!parsed.report.foreign_samples &&
			parsed.report.lost_events === 0 &&
			parsed.minimumSampleNs !== null &&
			parsed.maximumSampleNs !== null &&
			parsed.maximumSampleNs <= stopped.disableAckNs,
		"samples after disable acknowledgment or absent",
	);
	require_(
		stopped.closedNs - stopped.disableAckNs >= minimumTailNs,
		"disabled observation tail too short",
	);
}

async function helperPreflight(root, perf, privileged) {
	const clock = await verifyMonotonicClock(
		root,
		boundedCommand,
		privileged ? "sudo-monotonic-clock" : "user-monotonic-clock",
	);
	const helper = await ownedHelper();
	let recording, control, evidence, failure;
	try {
		recording = await openRecorder({
			root,
			name: privileged ? "sudo-bound" : "user-bound",
			perf,
			privileged,
			binding: helper.binding,
			seconds: 1.5,
		});
		const late = message(helper.child);
		helper.child.send("late");
		const created = await late;
		await recording.finish(false); // Prove the independent perf workload timer, without stop.
		const parsed = await decode(
			root,
			privileged ? "sudo-bound-script" : "user-bound-script",
			perf,
			recording.raw.filename,
			helper.binding.pid,
		);
		require_(
			parsed.report.samples > 0 &&
				parsed.report.multi_frame_samples > 0 &&
				parsed.resolved > 0 &&
				!parsed.report.malformed_lines &&
				!parsed.report.decode_warnings &&
				parsed.tids.has(helper.initial.tid) &&
				parsed.tids.has(helper.initial.main) &&
				!parsed.tids.has(created.tid),
			"existing-thread/no-inherit/unwind preflight",
		);
		control = await openRecorder({
			root,
			name: privileged ? "sudo-control" : "user-control",
			perf,
			privileged,
			binding: helper.binding,
			seconds: 3,
		});
		await new Promise((resolve) => setTimeout(resolve, 150));
		const stopped = await control.finish();
		const controlParsed = await decode(
			root,
			privileged ? "sudo-control-script" : "user-control-script",
			perf,
			control.raw.filename,
			helper.binding.pid,
		);
		validateDisabledSamples(controlParsed, stopped, 150000000n);
		require_(
			stopped.ack >= 3 &&
				!stopped.capped &&
				stopped.diagnostics?.record_disable_acknowledged === 1,
			"enable/disable acknowledgment preflight",
		);
		evidence = {
			clock,
			disable_ack_ns: String(stopped.disableAckNs),
			closed_ns: String(stopped.closedNs),
			control_sample_max_ns: String(controlParsed.maximumSampleNs),
			helper: helper.binding,
			initial: helper.initial,
			late: created,
			timed_recorder_births: recording.recorderBirths,
			control_recorder_births: control.recorderBirths,
			admitted_tids: [...parsed.tids],
			samples: parsed.report.samples,
			resolved_frames: parsed.resolved,
			control_acks: stopped.ack,
		};
		return {
			existing_threads_verified: 1,
			late_threads_excluded: 1,
			unwind_verified: 1,
			lifetime_bound_verified: 1,
			control_disable_verified: 1,
			monotonic_clock_verified: 1,
		};
	} catch (error) {
		failure = error;
		throw error;
	} finally {
		const stopped = await Promise.allSettled([
			control?.finish(),
			recording?.finish(),
		]);
		await verifyIdentity(helper.binding);
		if (helper.child.exitCode === null && !helper.child.signalCode)
			helper.child.kill("SIGTERM");
		await helper.closed;
		await requireGone([helper.binding]);
		await writeFile(
			path.join(root, privileged ? "sudo-proof.json" : "user-proof.json"),
			JSON.stringify({ ...evidence, helper_gone: 1 }),
			{ flag: "wx", mode: 0o600 },
		);
		if (!failure) {
			const rejected = stopped.find((result) => result.status === "rejected");
			if (rejected) throw rejected.reason;
		}
	}
}
async function verifiedTool(filename) {
	require_(
		path.isAbsolute(filename) && (await realpath(filename)) === filename,
		"absolute installed perf required",
	);
	const metadata = await lstat(filename),
		bytes = await readFile(filename);
	require_(
		metadata.isFile() &&
			!metadata.isSymbolicLink() &&
			metadata.uid === 0 &&
			!(metadata.mode & 0o022) &&
			bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46])),
		"installed perf must be immutable root-owned ELF",
	);
	return hash(bytes);
}
export async function selectPreflightMode(probe) {
	try {
		return { privileged: false, proof: await probe(false) };
	} catch (error) {
		if (!error.permissionDenied) throw error;
		return { privileged: true, proof: await probe(true) };
	}
}
export async function hostPreflight(perf, outputDirectory) {
	require_(process.platform === "linux", "Linux required");
	await mkdir(artifactPaths.tmp, { recursive: true });
	const root = await mkdtemp(
		path.join(artifactPaths.tmp, "headless-stacks-private-"),
	);
	await chmod(root, 0o700);
	const report = emptyReport();
	try {
		const perfHash = await verifiedTool(perf);
		const version = await boundedCommand(
			root,
			"perf-version",
			perf,
			["version", "--build-options"],
			65536,
		);
		require_(
			/(?:libunwind|dwarf)[^\n]*\bon\b/iu.test(version),
			"installed DWARF unwinder unavailable",
		);
		const { privileged, proof } = await selectPreflightMode((mode) =>
			helperPreflight(root, perf, mode),
		);
		const filename = path.join(root, "host-ready.json");
		await writeFile(
			filename,
			JSON.stringify({
				schema: 1,
				created_ms: Date.now(),
				uid: process.getuid(),
				boot_id: (
					await readFile("/proc/sys/kernel/random/boot_id", "utf8")
				).trim(),
				perf,
				perf_hash: perfHash,
				privileged,
				proof,
			}),
			{ flag: "wx", mode: 0o600 },
		);
		return filename;
	} catch (error) {
		for (const key of Object.keys(report))
			if (
				key.startsWith("record_") &&
				Object.hasOwn(error.recordDiagnostics ?? {}, key)
			)
				report[key] = error.recordDiagnostics[key];
		if (report.record_privileged !== null)
			report.privileged = report.record_privileged;
		report.failure_code = 1;
		await exportReport(outputDirectory, report);
		throw error;
	}
}
// A lifecycle/coverage probe only: no desktop workload, cadence, resource or release verdict.
// Dependency injection is for offline tests; production uses only freshly owned helper children.
export async function runRecorderLifetimeProbe(
	perf,
	outputDirectory,
	injected = {},
) {
	const io = {
		platform: process.platform,
		temporary: artifactPaths.tmp,
		mkdir,
		mkdtemp,
		chmod,
		privateDirectory,
		hostPreflight,
		privateJson,
		ownedHelper,
		openRecorder,
		decode,
		verifyIdentity,
		requireGone,
		writeFile,
		now: () => process.hrtime.bigint(),
		wait: (milliseconds) =>
			new Promise((resolve) => setTimeout(resolve, milliseconds)),
		waitClosed: async (closed, milliseconds) => {
			let timer;
			try {
				return await Promise.race([
					closed.then(() => true),
					new Promise((resolve) => {
						timer = setTimeout(() => resolve(false), milliseconds);
					}),
				]);
			} finally {
				clearTimeout(timer);
			}
		},
		...injected,
	};
	require_(io.platform === "linux", "Linux required");
	require_(
		path.isAbsolute(perf) && path.isAbsolute(outputDirectory),
		"absolute probe paths required",
	);
	let root;
	const report = {
		schema: 1,
		passed: 0,
		setup_stage: 0,
		failure_code: 0,
		requested_active_seconds: 85,
		recorder_lifetime_seconds: 120,
		active_wait_ns: null,
		active_sampling_wall_ns: null,
		disabled_tail_ns: null,
		preflight_verified: 0,
		disable_acknowledged: 0,
		natural_exit_verified: 0,
		samples: 0,
		resolved_frames: 0,
		unresolved_frames: null,
		helper_gone: 0,
		recorder_gone: 0,
		private_proof_retained: 0,
		performance_acceptance: 0,
	};
	let helper, recording, preflightPath, stopped, parsed, failure;
	try {
		await io.mkdir(io.temporary, { recursive: true });
		root = await io.mkdtemp(
			path.join(io.temporary, "headless-lifetime-private-"),
		);
		await io.chmod(root, 0o700);
		await io.privateDirectory(root);
		report.setup_stage = 1;
		preflightPath = await io.hostPreflight(perf, outputDirectory);
		const preflight = await io.privateJson(preflightPath);
		require_(
			preflight.perf === perf && typeof preflight.privileged === "boolean",
			"lifetime preflight binding",
		);
		report.preflight_verified = 1;
		report.setup_stage = 2;
		helper = await io.ownedHelper();
		await io.verifyIdentity(helper.binding);
		report.setup_stage = 3;
		recording = await io.openRecorder({
			root,
			name: "lifetime",
			perf,
			privileged: preflight.privileged,
			binding: helper.binding,
			seconds: 120,
		});
		report.setup_stage = 4;
		const start = io.now();
		await io.wait(85_000);
		report.active_wait_ns = uint(Number(io.now() - start));
		require_(
			report.active_wait_ns >= 85_000_000_000,
			"lifetime active wait too short",
		);
		await io.verifyIdentity(helper.binding);
		report.setup_stage = 5;
		stopped = await recording.finish();
		await io.verifyIdentity(helper.binding);
		require_(
			stopped.diagnostics?.record_exit_code === 0 &&
				stopped.diagnostics?.record_signal === 0 &&
				stopped.diagnostics?.record_ended_early === 0 &&
				stopped.diagnostics?.record_disable_acknowledged === 1 &&
				stopped.diagnostics?.record_stop_requested === 0 &&
				!stopped.capped,
			"lifetime natural completion",
		);
		report.disable_acknowledged = 1;
		report.natural_exit_verified = 1;
		report.active_sampling_wall_ns = uint(stopped.activeSamplingWallNs);
		report.disabled_tail_ns = uint(
			Number(stopped.closedNs - stopped.disableAckNs),
		);
		report.setup_stage = 6;
		parsed = await io.decode(
			root,
			"lifetime-script",
			perf,
			recording.raw.filename,
			helper.binding.pid,
		);
		validateDisabledSamples(parsed, stopped, 1_000_000_000n);
		require_(
			parsed.report.multi_frame_samples > 0 && parsed.resolved > 0,
			"lifetime positive decode",
		);
		report.samples = uint(parsed.report.samples);
		report.resolved_frames = uint(parsed.resolved);
		report.unresolved_frames = uint(parsed.report.unresolved_frames);
	} catch (error) {
		failure = error;
		report.failure_code = report.setup_stage || 8;
	} finally {
		// finish is memoized. Keep its original failure; absence must be independently proven.
		if (recording) {
			try {
				await recording.finish();
			} catch (error) {
				failure ??= error;
				report.failure_code ||= 7;
			}
			try {
				require_(
					recording.recorderBirths?.length > 0,
					"missing lifetime recorder births",
				);
				await io.requireGone(recording.recorderBirths);
				report.recorder_gone = 1;
			} catch (error) {
				failure ??= error;
				report.failure_code ||= 7;
			}
		}
		if (helper) {
			try {
				if (helper.child.exitCode === null && !helper.child.signalCode) {
					await io.verifyIdentity(helper.binding);
					helper.child.kill("SIGTERM");
				}
				if (!(await io.waitClosed(helper.closed, 2000))) {
					await io.verifyIdentity(helper.binding);
					helper.child.kill("SIGKILL");
					require_(
						await io.waitClosed(helper.closed, 2000),
						"lifetime helper remains alive",
					);
				}
				await io.requireGone([helper.binding]);
				report.helper_gone = 1;
			} catch (error) {
				failure ??= error;
				report.failure_code ||= 7;
			}
		}
		try {
			require_(typeof root === "string", "private lifetime root unavailable");
			const proofReport = {
				...report,
				private_proof_retained: 1,
				passed: Number(
					!failure && report.helper_gone === 1 && report.recorder_gone === 1,
				),
			};
			await io.writeFile(
				path.join(root, "lifetime-proof.json"),
				JSON.stringify({
					preflight_path: preflightPath,
					helper: helper?.binding,
					recorder_births: recording?.recorderBirths,
					disable_ack_ns: stopped ? String(stopped.disableAckNs) : null,
					closed_ns: stopped ? String(stopped.closedNs) : null,
					error: failure ? String(failure.message).slice(0, 2048) : null,
					report: proofReport,
				}),
				{ flag: "wx", mode: 0o600 },
			);
			report.private_proof_retained = 1;
		} catch {
			failure ??= new Error("private lifetime proof unavailable");
			report.passed = 0;
			report.private_proof_retained = 0;
			report.failure_code = 8;
		}
	}
	report.passed = Number(
		!failure &&
			report.helper_gone === 1 &&
			report.recorder_gone === 1 &&
			report.private_proof_retained === 1,
	);
	assertNumericOnly(report);
	return report;
}

export async function exportReport(outputDirectory, report) {
	assertNumericOnly(report);
	const baseline = emptyReport();
	require_(
		report.schema_version === 5 &&
			report.function_mapping_version === FUNCTION_MAPPING_VERSION &&
			report.requested_frequency_hz === 49,
		"unknown public schema",
	);
	require_(
		Number.isSafeInteger(report.pre_record_setup_stage) &&
			report.pre_record_setup_stage >= 0 &&
			report.pre_record_setup_stage <= 4,
		"public setup stage bound",
	);
	for (const key of [
		"pre_record_binary_match",
		"record_privileged",
		"record_spawn_error",
		"record_capture_error",
		"record_capped",
		"record_disable_requested",
		"record_disable_acknowledged",
		"record_stop_requested",
		"record_stop_acknowledged",
		"record_ended_early",
	])
		require_(
			report[key] === null || report[key] === 0 || report[key] === 1,
			"public recorder flag bound",
		);
	require_(
		report.record_signal === null ||
			[0, 2, 6, 9, 11, 15, 255].includes(report.record_signal),
		"public recorder signal bound",
	);
	for (const [key, maximum] of [
		["record_stage", 3],
		["record_exit_code", 255],
		["record_ack_count", LIMITS.stderr / 4],
		["record_stderr_class", 6],
	])
		require_(
			report[key] === null ||
				(Number.isSafeInteger(report[key]) &&
					report[key] >= 0 &&
					report[key] <= maximum),
			"public recorder enum bound",
		);
	for (const key of [
		"preflight_available",
		"symbols_available",
		"unwind_available",
		"recording_started",
		"capture_complete",
		"existing_threads_only",
		"future_thread_coverage",
		"transient_thread_coverage",
		"recorder_ended_early",
		"workload_failed",
		"per_frame_join_available",
		"recording_wall_includes_disabled_tail",
		"disable_boundary_verified",
		"sampling_overhead_present",
		"privileged",
	])
		require_(report[key] === 0 || report[key] === 1, "public flag bound");
	require_(
		report.stackless_samples <= report.samples &&
			report.multi_frame_samples <= report.samples &&
			report.unknown_self_samples <= report.samples &&
			report.depth_capped_samples <= report.samples &&
			report.unresolved_frames <= report.samples * LIMITS.frames,
		"public coverage bound",
	);

	require_(
		Object.keys(report).length === Object.keys(baseline).length &&
			Object.keys(report).every((key) => Object.hasOwn(baseline, key)),
		"unknown public field",
	);
	require_(
		report.samples + report.foreign_samples <= LIMITS.samples &&
			report.raw_bytes <= LIMITS.raw &&
			report.cap_code <= 3 &&
			report.failure_code <= 4,
		"public count bound",
	);
	require_(
		report.functions.length === baseline.functions.length &&
			report.functions.every(
				(row, id) =>
					row.id === id &&
					Object.keys(row).length === 5 &&
					Object.keys(row).every((key) =>
						Object.hasOwn(baseline.functions[id], key),
					),
			),
		"unknown function field",
	);
	for (const row of report.functions)
		require_(
			row.self_samples <= report.samples &&
				row.inclusive_samples <= report.samples &&
				row.self_period <= report.weighted_period &&
				row.inclusive_period <= report.weighted_period,
			"public function count bound",
		);
	require_(
		report.functions.reduce((sum, row) => sum + row.self_samples, 0) ===
			report.samples &&
			report.functions.reduce((sum, row) => sum + row.self_period, 0) ===
				report.weighted_period,
		"public denominator mismatch",
	);
	validateCoverage(report);
	await mkdir(outputDirectory, { recursive: true });
	await writeFile(
		path.join(outputDirectory, "headless-stacks-numeric.json"),
		`${JSON.stringify(report)}\n`,
		{ flag: "wx", mode: 0o600 },
	);
}
export async function createHeadlessStackCapture(cpu, options) {
	validateHeadlessStackOptions(options);
	const root = await mkdtemp(path.join(cpu.root, "headless-stacks-"));
	await chmod(root, 0o700);
	const report = emptyReport();
	try {
		const filename = options["headless-stack-preflight"];
		await privateDirectory(path.dirname(filename));
		const preflight = await privateJson(filename);
		require_(
			Object.keys(preflight).length === 8 &&
				typeof preflight.privileged === "boolean" &&
				preflight.proof &&
				Object.keys(preflight.proof).length === 6 &&
				[
					"existing_threads_verified",
					"late_threads_excluded",
					"unwind_verified",
					"lifetime_bound_verified",
					"control_disable_verified",
					"monotonic_clock_verified",
				].every((key) => preflight.proof[key] === 1) &&
				preflight.schema === 1 &&
				preflight.uid === process.getuid() &&
				Date.now() - preflight.created_ms >= 0 &&
				Date.now() - preflight.created_ms < 3600000 &&
				preflight.boot_id ===
					(await readFile("/proc/sys/kernel/random/boot_id", "utf8")).trim() &&
				preflight.perf_hash === (await verifiedTool(preflight.perf)) &&
				Object.values(preflight.proof).every((v) => v === 1),
			"stale/invalid host preflight",
		);
		report.preflight_available = 1;
		report.unwind_available = 1;
		report.privileged = Number(preflight.privileged);
		const binary = options["headless-stack-binary"];
		const sections = await boundedCommand(
			root,
			"binary-sections",
			"/usr/bin/readelf",
			["--sections", "--wide", binary],
			65536,
		);
		require_(
			sections.includes(".symtab") && sections.includes(".eh_frame"),
			"headless symbols/CFI unavailable",
		);
		const symbols = await boundedCommand(
			root,
			"binary-symbols",
			"/usr/bin/nm",
			["--demangle", "--defined-only", binary],
		);
		require_(
			symbols.split("\n").some((line) => {
				const id = functionId(line.replace(/^\S+\s+\S+\s+/u, ""));
				return (id > 0 && id <= 10) || id === 17;
			}),
			"headless function symbols unavailable",
		);
		report.symbols_available = 1;
		return new HeadlessStackCapture({
			root,
			preflight,
			binaryHash: hash(await readFile(binary)),
			outputDirectory: cpu.outputDirectory,
			report,
		});
	} catch (error) {
		report.failure_code = 2;
		await exportReport(cpu.outputDirectory, report);
		throw error;
	}
}
export class HeadlessStackCapture {
	constructor(configuration, io = {}) {
		Object.assign(this, configuration);
		this.io = io;
	}
	async retainPrivate(name, value) {
		if (this.io.writePrivate) return this.io.writePrivate(name, value);
		await privateDirectory(this.root);
		await writeFile(path.join(this.root, name), JSON.stringify(value), {
			flag: "wx",
			mode: 0o600,
		});
	}
	async run(cpu, measure) {
		let recording, binding, firstThreads, result, failure;
		const inspect = this.io.verifyIdentity ?? verifyIdentity;
		const listThreads = this.io.threads ?? threads;
		const open = this.io.openRecorder ?? openRecorder;
		const parse = this.io.decode ?? decode;
		try {
			this.report.pre_record_setup_stage = 1;
			binding = validatedServerBinding(cpu);
			this.report.pre_record_setup_stage = 2;
			await inspect(binding);
			this.report.pre_record_setup_stage = 3;
			this.report.pre_record_binary_match = Number(
				hash(await (this.io.readFile ?? readFile)(binding.executable)) ===
					this.binaryHash,
			);
			require_(
				this.report.pre_record_binary_match === 1,
				"running headless binary differs from pinned binary",
			);
			await this.retainPrivate(
				"owned-runtime-symbols.json",
				await (this.io.verifyRuntimeSymbols ?? verifyRuntimeSymbols)({
					root: this.root,
					binding,
				}),
			);
			this.report.pre_record_setup_stage = 4;
			firstThreads = await listThreads(binding.pid);
			this.report.initial_threads = firstThreads.length;
			await this.retainPrivate("owned-headless-before.json", {
				binding,
				initial_threads: firstThreads,
				binary_hash: this.binaryHash,
			});
			recording = await open({
				root: this.root,
				name: "headless",
				perf: this.preflight.perf,
				privileged: this.preflight.privileged,
				binding,
				// Sixty sleeps also include request/observer work. This finite lifetime
				// does not change the measurement polls or their elapsed denominators.
				seconds: 120,
			});
			const start = performance.now();
			this.report.recording_started = 1;
			try {
				result = await measure();
			} catch (error) {
				failure = error;
				this.report.workload_failed = 1;
			}
			const stopped = await recording.finish();
			for (const key of Object.keys(this.report))
				if (
					key.startsWith("record_") &&
					Object.hasOwn(stopped.diagnostics ?? {}, key)
				)
					this.report[key] = stopped.diagnostics[key];
			this.report.active_sampling_wall_ns =
				stopped.activeSamplingWallNs ?? null;
			this.report.recorder_ended_early = Number(stopped.endedEarly);
			this.report.recording_wall_ns = uint(
				Math.ceil((performance.now() - start) * 1e6),
			);
			await inspect(binding);
			require_(
				JSON.stringify(validatedServerBinding(cpu)) === JSON.stringify(binding),
				"server binding changed",
			);
			const finalThreads = await listThreads(binding.pid);
			await this.retainPrivate("owned-headless-after.json", {
				binding,
				final_threads: finalThreads,
				recorder_births: recording.recorderBirths,
				control_acks: stopped.ack,
				disable_ack_ns: String(stopped.disableAckNs),
				closed_ns: String(stopped.closedNs),
				recorder_gone: 1,
			});
			this.report.final_threads = finalThreads.length;
			this.report.late_threads = finalThreads.filter(
				(tid) => !firstThreads.includes(tid),
			).length;
			this.report.missing_threads = firstThreads.filter(
				(tid) => !finalThreads.includes(tid),
			).length;
			this.report.raw_bytes = recording.raw.bytes;
			const parsed = await parse(
				this.root,
				"headless-script",
				this.preflight.perf,
				recording.raw.filename,
				binding.pid,
				binding.executable,
			);
			validateDisabledSamples(parsed, stopped);
			this.report.disable_boundary_verified = 1;
			const fields = [
				"samples",
				"foreign_samples",
				"unknown_self_samples",
				"stackless_samples",
				"multi_frame_samples",
				"unresolved_frames",
				"depth_capped_samples",
				"malformed_lines",
				"decode_warnings",
				"lost_events",
				"weighted_period",
				"functions",
				"coverage",
			];
			for (const field of fields) this.report[field] = parsed.report[field];
			this.report.raw_bytes = recording.raw.bytes;
			this.report.cap_code = recording.capped ? 1 : 0;
			this.report.capture_complete = Number(
				!recording.capped &&
					!this.report.recorder_ended_early &&
					parsed.report.samples > 0 &&
					parsed.report.multi_frame_samples > 0 &&
					parsed.resolved > 0 &&
					parsed.report.unknown_self_samples < parsed.report.samples &&
					!parsed.report.unresolved_frames &&
					!this.report.late_threads &&
					!this.report.missing_threads &&
					!parsed.report.foreign_samples &&
					!parsed.report.stackless_samples &&
					!parsed.report.depth_capped_samples &&
					!parsed.report.malformed_lines &&
					!parsed.report.decode_warnings &&
					parsed.report.lost_events === 0 &&
					cpu.receipts.length === 61,
			);
			require_(this.report.capture_complete, "partial sampling coverage");
		} catch (error) {
			for (const key of Object.keys(this.report))
				if (
					key.startsWith("record_") &&
					Object.hasOwn(error.recordDiagnostics ?? {}, key)
				)
					this.report[key] = error.recordDiagnostics[key];
			this.report.failure_code = 3;
			if (!this.report.samples) this.report.lost_events = null;
			if (recording?.capped) this.report.cap_code = 1;
			else if (/cap/u.test(error.message)) this.report.cap_code = 3;
			failure ??= error;
		} finally {
			if (recording) this.report.raw_bytes = recording.raw.bytes;
			try {
				await recording?.finish();
			} catch (error) {
				this.report.failure_code = 4;
				failure ??= error;
			}
			try {
				await (this.io.exportReport ?? exportReport)(
					this.outputDirectory,
					this.report,
				);
			} catch (error) {
				failure ??= error;
			}
		}
		if (failure) throw failure;
		return result;
	}
}

if (
	process.argv[1] &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const [operation, perf, outputDirectory] = process.argv.slice(2);
	if (operation === "--pin-packaged") {
		console.log(await pinPackagedHeadless(perf, artifactPaths.tmp));
	} else {
		require_(
			operation === "--preflight" &&
				path.isAbsolute(perf ?? "") &&
				path.isAbsolute(outputDirectory ?? ""),
			"preflight usage",
		);
		const receipt = await hostPreflight(perf, outputDirectory);
		console.log(receipt);
	}
}
