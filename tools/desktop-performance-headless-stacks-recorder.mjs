import { execFile, spawn } from "node:child_process";
import {
	constants,
	closeSync,
	fstatSync,
	openSync,
	readSync,
	writeSync,
} from "node:fs";
import { lstat, readFile, readdir, readlink, realpath } from "node:fs/promises";
import path from "node:path";
import { parseLinuxProcessStat } from "./process-tree-resources.mjs";

export const LIMITS = Object.freeze({
	raw: 64 * 1024 * 1024,
	stderr: 65536,
	samples: 10000,
	frames: 128,
	line: 4096,
});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const require_ = (condition, message) => {
	if (!condition) throw new Error(`Headless stacks unavailable: ${message}`);
};

export class PrivateSink {
	constructor(filename, cap, overflow = () => {}) {
		this.filename = filename;
		this.cap = cap;
		this.overflow = overflow;
		this.bytes = 0;
		this.capped = false;
		this.fd = openSync(
			filename,
			constants.O_WRONLY |
				constants.O_CREAT |
				constants.O_EXCL |
				constants.O_NOFOLLOW,
			0o600,
		);
		this.identity = fstatSync(this.fd);
		require_(
			this.identity.isFile() &&
				this.identity.nlink === 1 &&
				this.identity.uid === process.getuid(),
			"private file ownership",
		);
	}
	write(chunk) {
		const bytes = Buffer.from(chunk);
		const remaining = Math.max(0, this.cap - this.bytes);
		if (remaining) {
			const part = bytes.subarray(0, remaining);
			let offset = 0;
			while (offset < part.length) {
				const written = writeSync(this.fd, part, offset, part.length - offset);
				require_(written > 0, "private write made no progress");
				offset += written;
				this.bytes += written;
			}
		}
		if (bytes.length > remaining && !this.capped) {
			this.capped = true;
			this.overflow();
		}
	}
	close() {
		if (this.fd !== null) {
			closeSync(this.fd);
			this.fd = null;
		}
	}
	async verify() {
		const value = await lstat(this.filename);
		require_(
			value.isFile() &&
				!value.isSymbolicLink() &&
				value.uid === process.getuid() &&
				value.nlink === 1 &&
				(value.mode & 0o7777) === 0o600 &&
				value.ino === this.identity.ino &&
				value.dev === this.identity.dev &&
				value.size === this.bytes &&
				value.size <= this.cap,
			"private file identity changed",
		);
	}
}
export async function privateDirectory(directory) {
	const value = await lstat(directory);
	require_(
		value.isDirectory() &&
			!value.isSymbolicLink() &&
			value.uid === process.getuid() &&
			(value.mode & 0o7777) === 0o700 &&
			(await realpath(directory)) === directory,
		"private directory",
	);
}
export async function privateText(filename, cap = LIMITS.stderr) {
	require_(
		Number.isSafeInteger(cap) && cap > 0 && cap <= LIMITS.raw,
		"private read cap",
	);
	const handle = openSync(filename, constants.O_RDONLY | constants.O_NOFOLLOW);
	try {
		const metadata = fstatSync(handle);
		require_(
			metadata.isFile() &&
				metadata.nlink === 1 &&
				metadata.uid === process.getuid() &&
				(metadata.mode & 0o7777) === 0o600 &&
				metadata.size <= cap,
			"private receipt",
		);
		const bytes = Buffer.alloc(cap + 1);
		let length = 0;
		while (length < bytes.length) {
			const count = readSync(
				handle,
				bytes,
				length,
				bytes.length - length,
				null,
			);
			if (!count) break;
			length += count;
		}
		require_(
			length <= cap &&
				fstatSync(handle).size === metadata.size &&
				length === metadata.size,
			"private receipt changed or exceeded cap",
		);
		return bytes.subarray(0, length).toString("utf8");
	} finally {
		closeSync(handle);
	}
}
export async function privateJson(filename) {
	return JSON.parse(await privateText(filename));
}

export async function processIdentity(pid) {
	const value = parseLinuxProcessStat(
		await readFile(`/proc/${pid}/stat`, "utf8"),
	);
	return {
		pid: value.pid,
		parent_pid: value.parentPid,
		start_ticks: value.startTicks,
		executable: await readlink(`/proc/${pid}/exe`),
	};
}
export async function verifyIdentity(record, io = {}) {
	const inspect = io.inspect ?? processIdentity;
	require_(
		record &&
			Number.isSafeInteger(record.pid) &&
			record.pid > 0 &&
			Array.isArray(record.ancestry),
		"missing owned binding",
	);
	const seen = new Set();
	for (let index = 0; index < record.ancestry.length; index++) {
		const expected = record.ancestry[index];
		require_(!seen.has(expected.pid), "cyclic owned ancestry");
		seen.add(expected.pid);
		const actual = await inspect(expected.pid);
		require_(
			actual.pid === expected.pid &&
				actual.parent_pid === expected.parent_pid &&
				actual.start_ticks === expected.start_ticks,
			"owned birth or ancestry changed",
		);
		if (index + 1 < record.ancestry.length)
			require_(
				expected.parent_pid === record.ancestry[index + 1].pid,
				"broken owned ancestry",
			);
		if (index === 0)
			require_(
				actual.executable === record.executable &&
					actual.pid === record.pid &&
					actual.start_ticks === record.start_ticks &&
					actual.parent_pid === record.parent_pid,
				"owned executable changed",
			);
	}
	require_(
		record.ancestry.length > 0 && record.ancestry[0].pid === record.pid,
		"missing owned ancestor",
	);
}
export async function requireGone(records) {
	for (const record of records) {
		try {
			const actual = parseLinuxProcessStat(
				await readFile(`/proc/${record.pid}/stat`, "utf8"),
			);
			require_(
				actual.startTicks !== record.start_ticks,
				"owned recorder remains alive",
			);
		} catch (error) {
			if (!["ENOENT", "ESRCH"].includes(error.code)) throw error;
		}
	}
}
async function recorderDescendants(child, perf, privileged) {
	const parent = await processIdentity(process.pid);
	const pending = [{ pid: child.pid, ancestry: [parent] }],
		found = [],
		seen = new Set();
	while (pending.length) {
		const { pid, ancestry } = pending.shift();
		require_(
			Number.isSafeInteger(pid) && pid > 0,
			"invalid recorder descendant PID",
		);
		if (seen.has(pid)) continue;
		seen.add(pid);
		require_(seen.size <= 8, "recorder ancestry cap");
		const before = parseLinuxProcessStat(
			await readFile(`/proc/${pid}/stat`, "utf8"),
		);
		require_(
			before.pid === pid && before.parentPid === ancestry[0].pid,
			"unowned recorder descendant",
		);
		const identity = {
			pid,
			parent_pid: before.parentPid,
			start_ticks: before.startTicks,
		};
		const readExecutable = async () => {
			try {
				return await readlink(`/proc/${pid}/exe`);
			} catch (error) {
				if (!privileged || !["EACCES", "EPERM"].includes(error.code))
					throw error;
				// Only this freshly proven descendant; fixed read-only argv, never a caller path.
				return new Promise((resolve, reject) =>
					execFile(
						"/usr/bin/sudo",
						["-n", "--", "/usr/bin/readlink", "--", `/proc/${pid}/exe`],
						{ timeout: 2000, maxBuffer: 4096, encoding: "utf8" },
						(failure, stdout) =>
							failure ? reject(failure) : resolve(stdout.trimEnd()),
					),
				);
			}
		};
		const inspect = async () => {
			for (const expected of [identity, ...ancestry]) {
				const actual = parseLinuxProcessStat(
					await readFile(`/proc/${expected.pid}/stat`, "utf8"),
				);
				require_(
					actual.pid === expected.pid &&
						actual.parentPid === expected.parent_pid &&
						actual.startTicks === expected.start_ticks,
					"recorder ancestry changed around executable read",
				);
			}
		};
		await inspect();
		const executable = await readExecutable();
		await inspect();
		require_(
			(await readExecutable()) === executable,
			"recorder executable changed",
		);
		await inspect();
		const record = { ...identity, executable };
		if (executable === perf) {
			found.push(record);
			continue;
		}
		require_(
			executable === "/usr/bin/sudo",
			"unexpected recorder intermediary",
		);
		const children = (
			await readFile(`/proc/${pid}/task/${pid}/children`, "utf8")
		).trim();
		if (children)
			for (const entry of children.split(/\s+/u)) {
				require_(/^[1-9][0-9]*$/u.test(entry), "invalid recorder descendant");
				pending.push({ pid: Number(entry), ancestry: [record, ...ancestry] });
			}
	}
	require_(found.length === 1, "exact installed perf child unavailable");
	return found;
}
export async function threads(pid) {
	return (await readdir(`/proc/${pid}/task`))
		.filter((entry) => /^[1-9][0-9]*$/u.test(entry))
		.map(Number)
		.sort((a, b) => a - b);
}
async function until(check, milliseconds, child) {
	const end = performance.now() + milliseconds;
	do {
		const value = check();
		if (value) return value;
		if (child?.exitCode !== null || child?.signalCode)
			throw new Error(
				"Headless stacks unavailable: recorder exited before ack",
			);
		await sleep(10);
	} while (performance.now() < end);
	throw new Error(
		"Headless stacks unavailable: control acknowledgment timeout",
	);
}
export function recordArguments(pid, seconds) {
	require_(
		Number.isSafeInteger(pid) &&
			pid > 0 &&
			Number.isFinite(seconds) &&
			seconds > 0 &&
			seconds <= 75,
		"invalid record target/bound",
	);
	return [
		"record",
		"--event",
		"cpu-clock:u",
		"--freq",
		"49",
		"--strict-freq",
		"--call-graph",
		"dwarf,8192",
		"--no-inherit",
		"--clockid",
		"mono",
		"--max-size",
		"60M",
		"--mmap-pages",
		"128",
		"--no-buildid-cache",
		"--output",
		"-",
		"--delay=-1",
		"--control=fd:0,2",
		"--pid",
		String(pid),
		"--",
		"/usr/bin/sleep",
		String(seconds),
	];
}
function stderrClassification(stderr) {
	if (typeof stderr !== "string") return null;
	const text = stderr
		.split("\n")
		.filter((line) => line.trim() && line !== "ack")
		.join("\n");
	if (!text) return 0;
	const classifiers = [
		[2, /Operation not permitted|Permission denied|perf_event_paranoid/iu],
		[
			3,
			/Input\/output error|No space left on device|Broken pipe|failed to write|write error|failed to mmap/iu,
		],
		[
			4,
			/not supported|unknown option|unrecognized option|invalid option|not compiled in/iu,
		],
		[1, /no samples|zero samples|zero-sized data|no data collected/iu],
		[5, /\bfatal\b|segmentation fault|assertion.*failed/iu],
	];
	return classifiers.find(([, pattern]) => pattern.test(text))?.[0] ?? 6;
}

// Public numeric enums: stage0=none,1=timed helper,2=stop helper,3=headless.
// Signal0=none,TERM=15,KILL=9,INT=2,SEGV=11,ABRT=6,255=other; null=not observed.
// Stderr0=empty/ACK,1=no samples,2=permission,3=I/O/resource,4=unsupported,5=fatal,6=other.
export function recorderFailureDetails({
	name,
	privileged,
	result,
	spawnError,
	captureError,
	capped,
	ack,
	stopRequested,
	stopAcknowledged,
	disableRequested,
	disableAcknowledged,
	endedEarly,
	stderr,
}) {
	const stage =
		name === "headless"
			? 3
			: ["user-bound", "sudo-bound"].includes(name)
				? 1
				: ["user-control", "sudo-control"].includes(name)
					? 2
					: 0;
	const signal = result
		? result.signal == null
			? 0
			: ({ SIGTERM: 15, SIGKILL: 9, SIGINT: 2, SIGSEGV: 11, SIGABRT: 6 }[
					result.signal
				] ?? 255)
		: null;
	return {
		record_stage: stage,
		record_exit_code:
			Number.isSafeInteger(result?.code) &&
			result.code >= 0 &&
			result.code <= 255
				? result.code
				: null,
		record_signal: signal,
		record_privileged: Number(Boolean(privileged)),
		record_spawn_error: Number(Boolean(spawnError)),
		record_capture_error: Number(Boolean(captureError)),
		record_capped: Number(Boolean(capped)),
		record_stop_requested: stopRequested == null ? null : Number(stopRequested),
		record_stop_acknowledged:
			stopRequested === false || stopAcknowledged == null
				? null
				: Number(stopAcknowledged),
		record_disable_requested:
			disableRequested == null ? null : Number(disableRequested),
		record_disable_acknowledged:
			disableAcknowledged == null ? null : Number(disableAcknowledged),
		record_ack_count: Number.isSafeInteger(ack) && ack >= 0 ? ack : null,
		record_ended_early: endedEarly == null ? null : Number(endedEarly),
		record_stderr_class: stderrClassification(stderr),
	};
}

export async function openRecorder({
	root,
	name,
	perf,
	privileged,
	binding,
	seconds = 75,
}) {
	require_(/^[a-z][a-z0-9-]{0,63}$/u.test(name), "private capture name");
	await verifyIdentity(binding);
	await privateDirectory(root);
	let capped = false,
		stopRequested = false,
		stopAckObserved = null,
		disableRequested = false,
		disableAcknowledged = null,
		enabledAt,
		activeSamplingWallNs = null,
		disableAckNs = null,
		closedNs = null;
	const requestStop = () => {
		stopRequested = true;
		child.stdin.write("stop\n");
	};
	const raw = new PrivateSink(
		path.join(root, `${name}.data`),
		LIMITS.raw,
		() => {
			capped = true;
			requestStop();
		},
	);
	const stderr = new PrivateSink(
		path.join(root, `${name}.stderr`),
		LIMITS.stderr,
		() => {
			capped = true;
			requestStop();
		},
	);
	const args = recordArguments(binding.pid, seconds);
	const child = spawn(
		privileged ? "/usr/bin/sudo" : perf,
		privileged ? ["-n", "--", perf, ...args] : args,
		{
			stdio: ["pipe", "pipe", "pipe"],
			env: {
				...process.env,
				LC_ALL: "C",
				DEBUGINFOD_URLS: "",
				PERF_CONFIG: "/dev/null",
			},
		},
	);
	child.stdin.on("error", () => {});
	let ack = 0,
		pending = "",
		spawnError,
		captureError;
	child.on("error", (error) => {
		spawnError = error;
	});
	child.stdout.on("data", (chunk) => {
		try {
			raw.write(chunk);
		} catch (error) {
			captureError ??= error;
			requestStop();
		}
	});
	child.stderr.on("data", (chunk) => {
		try {
			stderr.write(chunk);
		} catch (error) {
			captureError ??= error;
			requestStop();
		}
		pending += chunk.toString("utf8");
		if (pending.length > LIMITS.stderr) {
			pending = "";
			capped = true;
			requestStop();
		}
		let newline;
		while ((newline = pending.indexOf("\n")) >= 0) {
			if (pending.slice(0, newline) === "ack") ack++;
			pending = pending.slice(newline + 1);
		}
	});
	let closedResult = null;
	const closed = new Promise((resolve) =>
		child.once("close", (code, signal) => {
			closedNs = process.hrtime.bigint();
			closedResult = { code, signal };
			resolve(closedResult);
		}),
	);
	let recorderBirths = [],
		finishing,
		endedEarly = null;
	const finishOnce = async (sendDisable = true) => {
		endedEarly =
			sendDisable && (child.exitCode !== null || Boolean(child.signalCode));
		if (sendDisable && !endedEarly && !stopRequested) {
			const expectedAck = ack + 1;
			disableRequested = true;
			child.stdin.write("disable\n");
			try {
				await until(() => ack >= expectedAck, 2000, child);
				disableAckNs = process.hrtime.bigint();
				disableAcknowledged = true;
				activeSamplingWallNs = Number(disableAckNs - enabledAt);
			} catch {
				disableAcknowledged = false;
			}
		}
		// The sleep is perf's OWN workload: its exit terminates privileged perf, not a sudo timeout wrapper.
		let timer, result;
		try {
			result = await Promise.race([
				closed,
				new Promise((_, reject) => {
					timer = setTimeout(
						() =>
							reject(
								new Error(
									"Headless stacks unavailable: owned perf lifetime bound failed",
								),
							),
						(seconds + 5) * 1000,
					);
				}),
			]);
		} finally {
			clearTimeout(timer);
			raw.close();
			stderr.close();
		}
		await requireGone(recorderBirths);
		await raw.verify();
		await stderr.verify();
		if (/perf size limit reached/u.test(await privateText(stderr.filename)))
			capped = true;
		require_(
			!spawnError && !captureError && result.code === 0 && !result.signal,
			"record command failed",
		);
		require_(!capped && !stopRequested, "capture aborted by STOP or cap");
		require_(
			!sendDisable || (!endedEarly && disableAcknowledged),
			"disable acknowledgment unavailable or early exit",
		);
		return {
			raw,
			stderr,
			capped,
			ack,
			recorderBirths,
			endedEarly,
			activeSamplingWallNs,
			disableAckNs,
			closedNs,
			diagnostics: await failureDetails(),
		};
	};
	const failureDetails = async () => {
		let text = null;
		try {
			await stderr.verify();
			text = await privateText(stderr.filename);
		} catch {}
		return recorderFailureDetails({
			name,
			privileged,
			result: closedResult,
			spawnError,
			captureError,
			capped,
			ack,
			stopRequested,
			stopAcknowledged: stopAckObserved,
			disableRequested,
			disableAcknowledged,
			endedEarly,
			stderr: text,
		});
	};
	const finish = (sendDisable = true) => {
		finishing ??= finishOnce(sendDisable).catch(async (error) => {
			error.recordDiagnostics = await failureDetails();
			throw error;
		});
		return finishing;
	};
	try {
		child.stdin.write("ping\n");
		await until(() => ack >= 1, 2000, child);
		recorderBirths = await recorderDescendants(child, perf, privileged);
		await verifyIdentity(binding);
		child.stdin.write("enable\n");
		await until(() => ack >= 2, 2000, child);
		enabledAt = process.hrtime.bigint();
		return {
			finish,
			raw,
			stderr,
			child,
			recorderBirths,
			get capped() {
				return capped;
			},
			get ack() {
				return ack;
			},
		};
	} catch (error) {
		if (child.exitCode === null && !child.signalCode && !stopRequested)
			requestStop();
		await finish(false).catch(() => {});
		error.recordDiagnostics = await failureDetails();
		try {
			await stderr.verify();
			error.permissionDenied =
				/(?:Operation not permitted|Permission denied|perf_event_paranoid)/u.test(
					await privateText(stderr.filename),
				);
		} catch {}
		throw error;
	}
}

export async function boundedCommand(
	root,
	name,
	executable,
	args,
	cap = LIMITS.raw,
	onOutput,
	inputFile,
	timeoutMs = 15000,
) {
	require_(/^[a-z][a-z0-9-]{0,63}$/u.test(name), "private command name");
	await privateDirectory(root);
	const out = new PrivateSink(path.join(root, `${name}.stdout`), cap);
	const err = new PrivateSink(path.join(root, `${name}.stderr`), LIMITS.stderr);
	let input = null,
		inputIdentity;
	try {
		if (inputFile) {
			require_(
				path.dirname(inputFile) === root,
				"decoder input escaped private root",
			);
			input = openSync(inputFile, constants.O_RDONLY | constants.O_NOFOLLOW);
			inputIdentity = fstatSync(input);
			require_(
				inputIdentity.isFile() &&
					inputIdentity.uid === process.getuid() &&
					inputIdentity.nlink === 1 &&
					(inputIdentity.mode & 0o7777) === 0o600 &&
					inputIdentity.size <= LIMITS.raw,
				"decoder input ownership",
			);
		}
	} catch (error) {
		if (input !== null) closeSync(input);
		out.close();
		err.close();
		throw error;
	}
	const child = spawn(executable, args, {
		stdio: ["ignore", "pipe", "pipe", input ?? "ignore"],
		env: {
			...process.env,
			LC_ALL: "C",
			DEBUGINFOD_URLS: "",
			PERF_CONFIG: "/dev/null",
		},
	});
	let failure;
	child.on("error", (error) => {
		failure = error;
	});
	child.stdout.on("data", (chunk) => {
		try {
			out.write(chunk);
			onOutput?.(chunk);
			if (out.capped) throw new Error("output byte cap");
		} catch (error) {
			failure ??= error;
			child.kill("SIGTERM");
		}
	});
	child.stderr.on("data", (chunk) => {
		try {
			err.write(chunk);
			if (err.capped) throw new Error("stderr byte cap");
		} catch (error) {
			failure ??= error;
			child.kill("SIGTERM");
		}
	});
	const timer = setTimeout(() => {
		failure ??= new Error("bounded tool timeout");
		child.kill("SIGKILL");
	}, timeoutMs);
	const status = await new Promise((resolve) => child.once("close", resolve));
	clearTimeout(timer);
	out.close();
	err.close();
	if (input !== null) {
		try {
			const after = fstatSync(input),
				named = await lstat(inputFile);
			require_(
				after.size === inputIdentity.size &&
					named.ino === inputIdentity.ino &&
					named.dev === inputIdentity.dev &&
					!named.isSymbolicLink() &&
					named.nlink === 1,
				"decoder input changed",
			);
		} finally {
			closeSync(input);
		}
	}
	await out.verify();
	await err.verify();
	if (failure) throw failure;
	require_(status === 0, "bounded tool failed");
	return onOutput ? null : privateText(out.filename, cap);
}
