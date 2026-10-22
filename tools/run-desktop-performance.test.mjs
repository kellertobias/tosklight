import assert from "node:assert/strict";
import { createSocket } from "node:dgram";
import { EventEmitter } from "node:events";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { PassThrough } from "node:stream";
import { finished } from "node:stream/promises";
import test, { after } from "node:test";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	performanceCounterInterval,
	readPerformanceCounterSample,
	requireNoPerformanceRenderer,
	summarizePerformancePatch,
} from "./desktop-performance-patch.mjs";

import {
	configurePerformanceOutputRoutes,
	createPerformanceOutputSink,
} from "./desktop-performance-routes.mjs";

const epochBase = 1_000_000;
const simulatedDate = (clock) =>
	class extends Date {
		static now() {
			return epochBase + clock();
		}
	};
function diagnosticAt(millis, output = {}) {
	return {
		output_sample: {
			monotonic_micros: millis * 1000,
			sampled_at_epoch_micros: (epochBase + millis) * 1000,
		},
		output: {
			frames_sent: 0,
			frame_hz: 40,
			packets_sent: 0,
			deadline_misses: 0,
			send_errors: 0,
			...output,
		},
	};
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = await readFile(
	process.env.LIGHT_DESKTOP_PERFORMANCE_TEST_SOURCE ??
		path.join(root, "tools/run-desktop-performance.mjs"),
	"utf8",
);
const temporaryRoot = artifactPaths.tmp;
await mkdir(temporaryRoot, { recursive: true });
const scratch = await mkdtemp(
	path.join(temporaryRoot, "desktop-performance-test-"),
);
after(() => rm(scratch, { recursive: true, force: true }));
function load(name, end, extra = {}) {
	const start = source.indexOf(`async function ${name}(`);
	assert.notEqual(start, -1, `production ${name} must exist`);
	const code = source.slice(start, source.indexOf(end, start));
	const context = {
		mkdir,
		mkdtemp,
		readFile,
		writeFile,
		path,
		options: { "output-dir": scratch },
		Date,
		JSON,
		Set,
		setTimeout,
		spawnSync: () => ({ status: null, error: { code: "ENOENT" } }),
		assertRunning: () => {},
		REQUESTED_RATE_HZ: 40,
		summarizePatch: summarizePerformancePatch,
		readPerformanceCounterSample,
		performanceCounterInterval,
		requireNoPerformanceRenderer,
		...extra,
	};
	return vm.runInNewContext(`${code}\n${name}`, context);
}
test("production failure evidence survives normal scratch cleanup and remains explicitly incomplete", async () => {
	const retain = load("retainCaseFailure", "\nfunction applicationInvocation(");
	const temporary = await mkdtemp(path.join(scratch, "case-"));
	const reportPath = path.join(temporary, "sheet.jsonl");
	const preparedPath = path.join(temporary, "prepared");
	const raw = '{"kind":"stage-started","recordedAt":"2026-10-14T10:51:00Z"}\n';
	await writeFile(reportPath, raw);
	await writeFile(preparedPath, '{"fixtureRecords":264}\n');
	const filename = await retain({
		performanceCase: { caseId: "demo" },
		executionMode: "unrestricted",
		state: {
			phase: "fixture-sheet-readiness",
			backendReady: true,
			prepared: true,
			sheet: { ready: false },
		},
		reportPath,
		preparedPath,
		child: { pid: 999, exitCode: null, signalCode: null },
		output: ["actual log\n"],
		error: "Fixture Sheet never reported itself ready",
	});
	await rm(temporary, { recursive: true });
	const saved = JSON.parse(await readFile(filename, "utf8"));
	assert.equal(saved.complete, false);
	assert.equal(saved.state.sheet.ready, false);
	assert.equal(await readFile(saved.files["fixture-sheet.jsonl"], "utf8"), raw);
	assert.equal(await readFile(saved.files.processLog, "utf8"), "actual log\n");
	assert.equal(saved.files.screenshot.unavailable, "ENOENT");
	assert.ok(!filename.startsWith(`${temporary}/`));
});
test("missing startup files retain honest unavailable state and original error", async () => {
	const retain = load("retainCaseFailure", "\nfunction applicationInvocation(");
	const filename = await retain({
		performanceCase: { caseId: "demo" },
		executionMode: "unrestricted",
		state: { phase: "backend-readiness" },
		reportPath: path.join(scratch, "absent"),
		preparedPath: path.join(scratch, "absent-marker"),
		child: { pid: 9, exitCode: 1, signalCode: null },
		output: [],
		error: "released Desk exited early (1)",
	});
	const saved = JSON.parse(await readFile(filename, "utf8"));
	assert.equal(saved.error, "released Desk exited early (1)");
	assert.equal(saved.files["fixture-sheet.jsonl"].unavailable, "ENOENT");
	assert.equal(saved.files["prepared.json"].unavailable, "ENOENT");
});
test("actual ready evidence satisfies unchanged exact count gate and is observed", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [264] },
	);
	const p = path.join(scratch, "ready.jsonl");
	await writeFile(
		p,
		'{"kind":"fixture-sheet-ready","fixtureRecords":264,"rowCount":264}\n',
	);
	let observed;
	await wait(p, 264, {}, 100, (s) => {
		observed = s;
	});
	assert.equal(observed.ready, true);
	assert.equal(observed.lastRecords[0].fixtureRecords, 264);
});
test("wrong count fails unchanged gate while preserving exact observed count", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [263] },
	);
	const p = path.join(scratch, "wrong.jsonl");
	await writeFile(p, '{"kind":"fixture-sheet-ready","fixtureRecords":263}\n');
	let observed;
	await assert.rejects(
		wait(p, 264, {}, 100, (s) => {
			observed = s;
		}),
		/did not converge to 264/,
	);
	assert.equal(observed.ready, false);
	assert.equal(observed.lastRecords[0].fixtureRecords, 263);
});
test("earlier ready evidence cannot hide a later renderer failure or active-case unmount", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [3] },
	);
	for (const kind of [
		"fixture-sheet-render-failed",
		"fixture-sheet-container-unmounted",
	]) {
		const p = path.join(scratch, `${kind}-after-ready.jsonl`);
		await writeFile(
			p,
			[
				{ kind: "fixture-sheet-container-mounted" },
				{ kind: "fixture-sheet-ready", fixtureRecords: 3 },
				{ kind, errorName: "Error", errorMessage: "late render failure" },
			]
				.map((value) => JSON.stringify(value))
				.join("\n"),
		);
		await assert.rejects(
			wait(p, 3, {}, 100),
			/Fixture Sheet (render failed|container unmounted)/,
		);
	}
});
test("late observed render failures and active-case unmounts fail even within existing heartbeat tolerances", async () => {
	const requireActive = load(
		"requireFixtureSheetActive",
		"\nfunction assertRunning(",
	);
	const startedAt = Date.parse("2026-10-16T08:00:00Z");
	for (const executionMode of ["unrestricted", "one_core"]) {
		for (const kind of [
			"fixture-sheet-render-failed",
			"fixture-sheet-container-unmounted",
		]) {
			const p = path.join(scratch, `${executionMode}-${kind}-late.jsonl`);
			await writeFile(
				p,
				[
					{ kind: "fixture-sheet-ready", fixtureRecords: 3 },
					...Array.from({ length: 56 }, (_, second) => ({
						kind: "fixture-sheet-heartbeat",
						fixtureRecords: 3,
						recordedAt: new Date(startedAt + second * 1_000).toISOString(),
					})),
					{
						kind,
						errorName: "Error",
						errorMessage: "failed after last beat",
						recordedAt: new Date(startedAt + 56_000).toISOString(),
					},
				]
					.map((value) => JSON.stringify(value))
					.join("\n"),
			);
			let observed;
			await assert.rejects(
				requireActive(
					p,
					3,
					executionMode,
					{ startedAt, endedAt: startedAt + 60_000 },
					(snapshot) => {
						observed = snapshot;
					},
				),
				/Fixture Sheet (render failed|container unmounted)/,
			);
			assert.equal(observed.ready, false);
			assert.equal(
				(kind === "fixture-sheet-render-failed"
					? observed.renderFailure
					: observed.surface
				).kind,
				kind,
			);
		}
	}
});
test("earlier Strict Mode container cleanup followed by mount does not fail a healthy active case", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [3] },
	);
	const requireActive = load(
		"requireFixtureSheetActive",
		"\nfunction assertRunning(",
	);
	const startedAt = Date.parse("2026-10-16T08:00:00Z");
	const p = path.join(scratch, "strict-mode-remount.jsonl");
	await writeFile(
		p,
		[
			{ kind: "fixture-sheet-container-mounted" },
			{ kind: "fixture-sheet-container-unmounted" },
			{ kind: "fixture-sheet-container-mounted" },
			{ kind: "fixture-sheet-ready", fixtureRecords: 3 },
			...Array.from({ length: 61 }, (_, second) => ({
				kind: "fixture-sheet-heartbeat",
				fixtureRecords: 3,
				recordedAt: new Date(startedAt + second * 1_000).toISOString(),
			})),
		]
			.map((value) => JSON.stringify(value))
			.join("\n"),
	);
	await wait(p, 3, {}, 100);
	await requireActive(p, 3, "unrestricted", {
		startedAt,
		endedAt: startedAt + 60_000,
	});
});
test("remounting after readiness cannot hide an active-case container unmount", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [3] },
	);
	const requireActive = load(
		"requireFixtureSheetActive",
		"\nfunction assertRunning(",
	);
	const startedAt = Date.parse("2026-10-16T08:00:00Z");
	const p = path.join(scratch, "post-ready-remount.jsonl");
	await writeFile(
		p,
		[
			{ kind: "fixture-sheet-container-mounted" },
			{ kind: "fixture-sheet-ready", fixtureRecords: 3 },
			{ kind: "fixture-sheet-container-unmounted" },
			{ kind: "fixture-sheet-container-mounted" },
			...Array.from({ length: 61 }, (_, second) => ({
				kind: "fixture-sheet-heartbeat",
				fixtureRecords: 3,
				recordedAt: new Date(startedAt + second * 1_000).toISOString(),
			})),
		]
			.map((value) => JSON.stringify(value))
			.join("\n"),
	);
	await assert.rejects(
		wait(p, 3, {}, 100),
		/Fixture Sheet container unmounted/,
	);
	await assert.rejects(
		requireActive(p, 3, "unrestricted", {
			startedAt,
			endedAt: startedAt + 60_000,
		}),
		/Fixture Sheet container unmounted/,
	);
});
test("missing report retains uncertainty instead of invented loading predicate", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{ sheetReadyCounts: async () => [] },
	);
	let observed;
	await assert.rejects(
		wait(path.join(scratch, "not-created"), null, {}, 100, (s) => {
			observed = s;
		}),
		/never reported itself ready/,
	);
	assert.equal(observed.reportPresent, false);
	assert.equal(observed.ready, false);
	assert.ok(observed.unobservedPredicates.includes("group-runtime loading"));
	assert.equal(observed.surface, null);
	assert.equal(observed.readiness, null);
	assert.equal(observed.renderFailure, null);
});
test("explicit render failure retains actual mount and blocked predicates before failing the gate", async () => {
	const wait = load(
		"waitForFixtureSheet",
		"\nasync function sheetReadyCounts(",
		{
			sheetReadyCounts: async () => [],
		},
	);
	const p = path.join(scratch, "diagnosed-not-ready.jsonl");
	const predicates = {
		kind: "fixture-sheet-readiness",
		active: true,
		rowCount: 0,
		fixtureRecords: 0,
		activeValuesLoading: false,
		groupRuntimeLoading: true,
	};
	await writeFile(
		p,
		[
			{ kind: "fixture-sheet-container-mounted" },
			predicates,
			{
				kind: "fixture-sheet-render-failed",
				errorName: "Error",
				errorMessage: "fixture display failed",
			},
		]
			.map((value) => JSON.stringify(value))
			.join("\n"),
	);
	let observed;
	await assert.rejects(
		wait(p, null, {}, 100, (value) => {
			observed = value;
		}),
		/Fixture Sheet render failed/,
	);
	assert.ok(observed.readiness, "actual readiness predicates must be retained");
	assert.deepEqual(JSON.parse(JSON.stringify(observed.readiness)), predicates);
	assert.equal(observed.surface.kind, "fixture-sheet-container-mounted");
	assert.equal(observed.renderFailure.errorMessage, "fixture display failed");
	assert.equal(observed.ready, false);
	assert.equal(
		observed.unobservedPredicates.includes("group-runtime loading"),
		false,
	);
	assert.ok(observed.unobservedPredicates.includes("Tauri append failure"));
});
test("production runner writes prior successful cases as incomplete on later failure without successful aggregate", async () => {
	const out = await mkdtemp(path.join(scratch, "aggregate-"));
	const start = source.indexOf("const results = [];");
	const end = source.indexOf("\nfunction parseArguments(", start);
	const code = source.slice(start, end);
	const context = {
		writeFile,
		path,
		options: { "output-dir": out },
		JSON,
		Error,
		String,
		CASES: [
			{ caseId: "first", demo: false },
			{ caseId: "second", demo: false },
		],
		runCase: async (c) => {
			if (c.caseId === "second") throw new Error("observed missing ready");
			return { case_id: "first", achieved_rate_hz: 40 };
		},
	};
	await assert.rejects(
		vm.runInNewContext(`(async()=>{${code}})()`, context),
		/observed missing ready/,
	);
	const partial = JSON.parse(
		await readFile(path.join(out, "desktop-scenarios.partial.json"), "utf8"),
	);
	assert.equal(partial.complete, false);
	assert.equal(partial.scenarios.length, 1);
	assert.equal(partial.failed_case.case_id, "second");
	await assert.rejects(
		readFile(path.join(out, "desktop-scenarios.json")),
		/ENOENT/,
	);
});
test("partial report IO failure preserves the original benchmark failure", async () => {
	const start = source.indexOf("const results = [];");
	const end = source.indexOf("\nfunction parseArguments(", start);
	const code = source.slice(start, end);
	const original = new Error(
		"precise fixture-sheet failure; diagnostics path retained",
	);
	const diagnostic = new Error("disk full writing partial aggregate");
	diagnostic.code = "ENOSPC";
	const messages = [];
	const context = {
		writeFile: async () => {
			throw diagnostic;
		},
		path,
		options: { "output-dir": scratch },
		JSON,
		Error,
		String,
		console: { error: (message) => messages.push(message) },
		CASES: [{ caseId: "demo", demo: false }],
		runCase: async () => {
			throw original;
		},
	};
	await assert.rejects(
		vm.runInNewContext(`(async()=>{${code}})()`, context),
		(error) => error === original,
	);
	assert.equal(messages.length, 1);
	assert.match(messages[0], /partial.*unavailable/i);
	assert.match(messages[0], /disk full/);
});

test("successful runtime evidence retains all log chunks and raw diagnostics after cleanup", async () => {
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const finish = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
	);
	const temporary = await mkdtemp(path.join(scratch, "success-"));
	const sheet = path.join(temporary, "sheet");
	const prepared = path.join(temporary, "prepared");
	await writeFile(sheet, '{"kind":"fixture-sheet-ready"}\n');
	await writeFile(prepared, '{"fixtureRecords":312}\n');
	const evidence = await create(temporary);
	for (let i = 0; i < 150; i++) evidence.log.write(`actual chunk ${i}\n`);
	evidence.samples.push({
		recordedAt: "2026-10-14T15:00:00Z",
		diagnostics: { output: { frames_sent: 40, send_errors: 1 } },
	});
	await finish(evidence, sheet, prepared, { phase: "complete" });
	await rm(temporary, { recursive: true });
	const saved = JSON.parse(await readFile(evidence.manifestPath, "utf8"));
	const raw = await readFile(saved.runtimeLog, "utf8");
	assert.match(raw, /actual chunk 0\n/);
	assert.match(raw, /actual chunk 149\n/);
	assert.equal(
		JSON.parse(await readFile(saved.measurement, "utf8")).samples[0].diagnostics
			.output.send_errors,
		1,
	);
	assert.equal(
		await readFile(saved.prepared, "utf8"),
		'{"fixtureRecords":312}\n',
	);
	assert.equal(
		await readFile(saved.fixtureSheet, "utf8"),
		'{"kind":"fixture-sheet-ready"}\n',
	);
	assert.ok(!saved.runtimeLog.startsWith(`${temporary}/`));
});
test("exception-path runtime evidence closes streams and qualifies unavailable startup files", async () => {
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const finish = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
	);
	const temporary = await mkdtemp(path.join(scratch, "exception-"));
	const evidence = await create(temporary);
	evidence.log.write("actual startup refusal\n");
	await finish(
		evidence,
		path.join(temporary, "absent-sheet"),
		path.join(temporary, "absent-prepared"),
		{ phase: "backend-readiness" },
	);
	await rm(temporary, { recursive: true });
	const saved = JSON.parse(await readFile(evidence.manifestPath, "utf8"));
	assert.equal(saved.fixtureSheet.unavailable, "ENOENT");
	assert.equal(saved.prepared.unavailable, "ENOENT");
	assert.equal(
		await readFile(saved.runtimeLog, "utf8"),
		"actual startup refusal\n",
	);
	assert.equal(evidence.log.writableFinished, true);
});

test("actual case finalization flushes termination logs on success and original failure without session secrets", async () => {
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const finish = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
	);
	for (const fail of [false, true, "spawn", "render", "routes"]) {
		const child = Object.assign(new EventEmitter(), {
			pid: 999,
			stdout: new EventEmitter(),
			stderr: new EventEmitter(),
		});
		const files = [];
		let sinkClosed = false;
		const finalize = load(
			"finalizeCaseEvidence",
			"\nasync function drainCaseLog(",
			{
				drainCaseLog: async (e) => await e.closed,
				terminateProcessTree: async () => {
					child.stderr.emit("data", Buffer.from("termination tail\n"));
					child.emit("close", 0);
				},
				finishCaseEvidence: finish,
				rm,
			},
		);
		const run = load("runCase", "\nasync function finalizeCaseEvidence(", {
			options: { "output-dir": scratch, application: "/isolated/stub.app" },
			finalizeCaseEvidence: finalize,
			freePort: async () => 19876,
			applicationInvocation: () => ({
				command: "isolated-stub",
				arguments: [],
			}),
			spawn: () => {
				if (fail === "spawn")
					queueMicrotask(() => {
						child.emit("error", new Error("literal startup spawn refusal"));
						child.emit("close", -1);
					});
				return child;
			},
			process: { cwd: () => scratch, env: {} },
			createCaseEvidence: async (temp) => {
				const e = await create(temp);
				files.push(e);
				return e;
			},
			finishCaseEvidence: finish,
			waitForReadiness: async () => {
				if (child.spawnError) throw child.spawnError;
				child.stdout.emit("data", Buffer.from("startup log\n"));
			},
			apiClient: () => ({
				request: async () => ({ token: "private-session-token" }),
			}),
			configureScheduler: async () => ({
				configured_rate_hz: 40,
				configuration_rate_verified: true,
			}),
			createPerformanceOutputSink: async () => ({
				destination: "127.0.0.1:19877",
				start() {},
				stop: () => ({ packets_received: 1 }),
				close: async () => {
					sinkClosed = true;
				},
			}),
			configurePerformanceOutputRoutes: async () => {
				if (fail === "routes")
					throw new Error("literal route configuration failure");
				return { routed_patch_universes: [101] };
			},
			requireRendererOff: async () => {},
			prepareDemo: async () => ({ parameterCount: 10, fixtureRecords: 312 }),
			waitForFixtureSheet: async () => {},
			drawBudgetMillis: () => 100,
			measureWindow: async (_api, _pid, _mode, samples) => {
				samples.push({ diagnostics: { output: { frames_sent: 40 } } });
				return { average_completed_hz: 40 };
			},
			requireFixtureSheetActive: async (
				_path,
				_expected,
				_mode,
				_window,
				observe,
			) => {
				if (fail === "render") {
					observe({
						ready: false,
						renderFailure: {
							kind: "fixture-sheet-render-failed",
							errorMessage: "literal renderer failure",
						},
					});
					throw new Error("literal renderer failure");
				}
				if (fail === true) throw new Error("literal liveness failure");
			},
			retainCaseFailure: async () => "retained-original-failure.json",
			DURATION_SECONDS: 60,
			REQUESTED_RATE_HZ: 40,
			terminateProcessTree: async () => {
				child.stderr.emit("data", Buffer.from("termination tail\n"));
				child.emit("close", 0);
			},
			rm,
		});
		if (fail)
			await assert.rejects(
				run({ caseId: "demo", demo: true }, "unrestricted"),
				fail === "spawn"
					? /literal startup spawn refusal/
					: fail === "render"
						? /literal renderer failure/
						: fail === "routes"
							? /literal route configuration failure/
							: /literal liveness failure/,
			);
		else
			assert.equal(
				(await run({ caseId: "demo", demo: true }, "unrestricted"))
					.average_completed_hz,
				40,
			);
		const e = files[0];
		const log = await readFile(e.runtimeLog, "utf8");
		assert.match(log, /termination tail/);
		assert.doesNotMatch(log, /private-session-token/);
		const saved = JSON.parse(await readFile(e.manifestPath, "utf8"));
		assert.equal(
			JSON.parse(await readFile(saved.measurement, "utf8")).samples.length,
			fail === "spawn" || fail === "routes" ? 0 : 1,
		);
		if (fail === "render") {
			const state = JSON.parse(await readFile(saved.measurement, "utf8")).state;
			assert.equal(state.sheet.ready, false);
			assert.equal(
				state.sheet.renderFailure.errorMessage,
				"literal renderer failure",
			);
			assert.equal(state.phase, "fixture-sheet-liveness");
		}
		assert.equal(e.log.writableFinished, true);
		assert.equal(sinkClosed, fail !== "spawn");
	}
});

test("final evidence IO failure preserves original failure and recoverable scratch", async () => {
	const original = new Error("original readiness refusal");
	const messages = [];
	let removed = false;
	const finish = load(
		"finalizeCaseEvidence",
		"\nasync function drainCaseLog(",
		{
			terminateProcessTree: async () => {},
			drainCaseLog: async () => {},
			finishCaseEvidence: async () => {
				throw Object.assign(new Error("disk full"), { code: "ENOSPC" });
			},
			rm: async () => {
				removed = true;
			},
			console: { error: (m) => messages.push(m) },
		},
	);
	const evidence = { manifestPath: "/canonical/case-evidence.json" };
	assert.equal(
		await finish(
			{ pid: 999 },
			evidence,
			"sheet",
			"prepared",
			{},
			"/canonical/recoverable",
			original,
		),
		original,
	);
	assert.equal(removed, false);
	assert.match(messages[0], /scratch preserved.*recoverable.*disk full/);
	const result = await finish(
		{ pid: 999 },
		evidence,
		"sheet",
		"prepared",
		{},
		"/canonical/recoverable",
		undefined,
	);
	assert.match(result.message, /Runtime evidence unavailable/);
});
test("measured evidence reuses exact existing diagnostic reads without changing rate calculation", async () => {
	let clock = 0;
	let reads = 0;
	const samples = [];
	const measure = load("measureWindow", "\nfunction apiClient(", {
		DURATION_SECONDS: 2,
		performance: { now: () => clock },
		Date: simulatedDate(() => clock),
		setTimeout: (cb, millis) => {
			clock += millis;
			cb();
		},
		readLinuxProcessTree: async () => [],
		processTreeResourceDelta: () => ({ cpuPercent: 0, residentBytes: 0 }),
		LINUX_PROCESS_OPTIONS: {},
		average: (a) => a.reduce((x, y) => x + y, 0) / a.length,
		percentile: (a) => a[0],
	});
	const result = await measure(
		{
			request: async () =>
				diagnosticAt(clock, {
					frames_sent: 5 + 40 * reads++,
					send_errors: 7,
					frame_rate_band_counts: [1],
				}),
		},
		999,
		"unrestricted",
		samples,
	);
	assert.equal(reads, 3);
	assert.equal(samples.length, 3);
	assert.equal(samples[0].phase, "opening");
	assert.equal(samples[2].diagnostics.output.frames_sent, 85);
	assert.equal(samples[2].diagnostics.output.send_errors, 7);
	assert.ok(samples[2].diagnosticsStartedAt);
	assert.equal(result.average_completed_hz, 40);
	assert.equal(result.windows_below_target, 0);
});

test("matched counter and CPU intervals include slow diagnostics and process reads", async () => {
	let clock = 0;
	let reads = 0;
	const samples = [];
	const cpuIntervals = [];
	const processes = ["light-headless", "WebKitWebProces", "light-desktop"].map(
		(command, index) => ({ command, pid: index + 1 }),
	);
	const measure = load("measureWindow", "\nfunction apiClient(", {
		DURATION_SECONDS: 2,
		performance: { now: () => clock },
		Date: simulatedDate(() => clock),
		setTimeout: (cb, millis) => {
			clock += millis;
			cb();
		},
		readLinuxProcessTree: async () => {
			clock += 100;
			return processes;
		},
		processTreeResourceDelta: (_before, after, elapsed) => {
			cpuIntervals.push(elapsed);
			return { cpuPercent: after.length, residentBytes: after.length };
		},
		LINUX_PROCESS_OPTIONS: {},
		average: (a) => a.reduce((x, y) => x + y, 0) / a.length,
		percentile: (a) => a[0],
	});
	const result = await measure(
		{
			request: async () => {
				reads += 1;
				const duration = reads === 3 ? 600 : 250;
				const snapshot = clock + duration / 2;
				clock += duration;
				return diagnosticAt(snapshot, {
					frames_sent: (snapshot * 40) / 1000,
					frame_rate_band_counts: [1],
				});
			},
		},
		999,
		"unrestricted",
		samples,
	);
	assert.equal(reads, 3);
	assert.equal(result.average_completed_hz, 40);
	assert.equal(result.minimum_one_second_completed_hz, 40);
	assert.equal(result.windows_below_target, 0);
	assert.equal(samples[1].elapsed, 1.35);
	assert.equal(samples[2].elapsed, 1.075);
	assert.equal(samples[1].processElapsed, 1.35);
	assert.equal(samples[2].processElapsed, 1.25);
	assert.equal(samples[0].diagnosticsMonotonicStartMillis, 0);
	assert.equal(samples[0].diagnosticsMonotonicEndMillis, 250);
	assert.equal(
		samples[2].diagnosticsMonotonicEndMillis -
			samples[2].diagnosticsMonotonicStartMillis,
		600,
	);
	assert.ok(cpuIntervals.includes(1.25));
	assert.equal(result.resources.application_cpu_average_percent, 3);
	assert.equal(result.resources.server.cpu_average_percent, 1);
	assert.equal(result.resources.desktop_webview.cpu_average_percent, 1);
	assert.equal(result.resources.native_renderer.cpu_average_percent, 0);
	assert.equal(result.resources.desktop_host.cpu_average_percent, 1);
});

test("owned sidecar phase log survives successful and failed-case evidence cleanup", async () => {
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const finish = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
	);
	for (const phase of ["complete", "measurement"]) {
		const temporary = await mkdtemp(path.join(scratch, "sidecar-"));
		await mkdir(path.join(temporary, "data"));
		const contents = "slow output tick phase sample engine_micros=42000\n";
		await writeFile(path.join(temporary, "data/light-headless.log"), contents);
		const evidence = await create(temporary);
		await finish(
			evidence,
			path.join(temporary, "sheet"),
			path.join(temporary, "prepared"),
			{ phase },
		);
		await rm(temporary, { recursive: true });
		const saved = JSON.parse(await readFile(evidence.manifestPath, "utf8"));
		assert.equal(await readFile(saved.sidecarLog, "utf8"), contents);
		assert.ok(!saved.sidecarLog.startsWith(`${temporary}/`));
	}
});

test("renderer-disabled baseline rejects a renderer at opening or during measurement", async () => {
	for (const phase of ["opening", "during"]) {
		let clock = 0,
			reads = 0;
		const measure = load("measureWindow", "\nfunction apiClient(", {
			DURATION_SECONDS: 1,
			performance: { now: () => clock },
			Date: simulatedDate(() => clock),
			setTimeout: (cb, millis) => {
				clock += millis;
				cb();
			},
			readLinuxProcessTree: async () =>
				reads++ >= (phase === "opening" ? 0 : 1)
					? [{ pid: 1, command: "viz-renderer" }]
					: [],
			processTreeResourceDelta: () => ({ cpuPercent: 0, residentBytes: 0 }),
			LINUX_PROCESS_OPTIONS: {},
			average: () => 0,
			percentile: () => 0,
		});
		await assert.rejects(
			measure(
				{
					request: async () => diagnosticAt(clock, { frames_sent: clock / 25 }),
				},
				999,
				"unrestricted",
				[],
			),
			/unexpectedly started viz-renderer/,
		);
	}
});

test("actual sidecar evidence write failure preserves original case failure and scratch", async () => {
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const finish = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
		{
			writeFile: async (file, ...args) => {
				if (file.endsWith("-light-headless.log"))
					throw Object.assign(new Error("sidecar evidence disk full"), {
						code: "ENOSPC",
					});
				return writeFile(file, ...args);
			},
		},
	);
	const messages = [];
	let removed = false;
	const finalize = load(
		"finalizeCaseEvidence",
		"\nasync function drainCaseLog(",
		{
			terminateProcessTree: async () => {},
			drainCaseLog: async () => {},
			finishCaseEvidence: finish,
			rm: async () => {
				removed = true;
			},
			console: { error: (message) => messages.push(message) },
		},
	);
	const temporary = await mkdtemp(path.join(scratch, "sidecar-io-"));
	await mkdir(path.join(temporary, "data"));
	await writeFile(
		path.join(temporary, "data/light-headless.log"),
		"retained phase data\n",
	);
	const evidence = await create(temporary);
	const original = new Error("original output failure");
	assert.equal(
		await finalize(
			{ pid: 999 },
			evidence,
			path.join(temporary, "sheet"),
			path.join(temporary, "prepared"),
			{},
			temporary,
			original,
		),
		original,
	);
	assert.equal(removed, false);
	assert.match(messages[0], /sidecar evidence disk full/);
	assert.equal(
		await readFile(path.join(temporary, "data/light-headless.log"), "utf8"),
		"retained phase data\n",
	);
});

test("missing owned sidecar log is explicitly unavailable", async () => {
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const finish = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
	);
	const temporary = await mkdtemp(path.join(scratch, "sidecar-absent-"));
	const evidence = await create(temporary);
	await finish(
		evidence,
		path.join(temporary, "sheet"),
		path.join(temporary, "prepared"),
		{ phase: "backend-readiness" },
	);
	const saved = JSON.parse(await readFile(evidence.manifestPath, "utf8"));
	assert.equal(saved.sidecarLog.unavailable, "ENOENT");
});

test("never-closing child drain is bounded, destroys inherited pipes and preserves original gate error", async () => {
	const pending = new Set();
	let cleared = 0;
	const drain = load("drainCaseLog", "\nasync function createCaseEvidence(", {
		setTimeout: (callback) => {
			const id = {};
			pending.add(id);
			queueMicrotask(callback);
			return id;
		},
		clearTimeout: (id) => {
			pending.delete(id);
			cleared++;
		},
	});
	let destroyed = 0;
	let unref = 0;
	let ended = 0;
	let removed = 0;
	const messages = [];
	const create = load(
		"createCaseEvidence",
		"\nasync function finishCaseEvidence(",
		{ createWriteStream, finished },
	);
	const save = load(
		"finishCaseEvidence",
		"\nasync function retainCaseFailure(",
	);
	const temporary = await mkdtemp(path.join(scratch, "never-close-"));
	const evidence = await create(temporary);
	evidence.closed = new Promise(() => {});
	evidence.log.write("actual pre-timeout log\n");
	const child = {
		pid: 999,
		stdout: new PassThrough(),
		stderr: new PassThrough(),
		unref: () => unref++,
	};
	child.stdout.on("close", () => destroyed++);
	child.stderr.on("close", () => destroyed++);
	const finalize = load(
		"finalizeCaseEvidence",
		"\nasync function drainCaseLog(",
		{
			terminateProcessTree: async () => {},
			drainCaseLog: drain,
			finishCaseEvidence: async (...args) => {
				ended++;
				await save(...args);
			},
			rm: async () => removed++,
			console: { error: (m) => messages.push(m) },
		},
	);
	const state = {};
	const original = new Error("original liveness refusal");
	const result = await finalize(
		child,
		evidence,
		path.join(temporary, "absent-sheet"),
		path.join(temporary, "absent-prepared"),
		state,
		"/canonical/recoverable",
		original,
	);
	assert.equal(result, original);
	assert.equal(destroyed, 2);
	assert.equal(evidence.log.writableFinished, true);
	assert.equal(child.stdout.destroyed, true);
	assert.equal(child.stderr.destroyed, true);
	assert.equal(
		await readFile(evidence.runtimeLog, "utf8"),
		"actual pre-timeout log\n",
	);
	assert.equal(unref, 1);
	assert.equal(ended, 1);
	assert.equal(removed, 0);
	assert.equal(pending.size, 0);
	assert.equal(cleared, 1);
	assert.equal(state.runtimeLogDrain.complete, false);
	assert.match(messages[0], /5000ms.*incomplete/);
});
test("normally closed child drain clears its timeout without retaining timer handles", async () => {
	const pending = new Set();
	const drain = load("drainCaseLog", "\nasync function createCaseEvidence(", {
		setTimeout: () => {
			const id = {};
			pending.add(id);
			return id;
		},
		clearTimeout: (id) => pending.delete(id),
	});
	await drain({ closed: Promise.resolve() });
	assert.equal(pending.size, 0);
});

function countingPatch() {
	return {
		profile_revisions: [
			{
				profile_id: "p",
				profile_revision: 7,
				referenced_modes: [
					{
						mode_id: "m",
						splits: [
							{ split: 0, footprint: 4 },
							{ split: 1, footprint: 2 },
						],
					},
					{ mode_id: "visual", splits: [{ split: 0, footprint: 0 }] },
				],
			},
		],
		fixtures: [
			{
				fixture_id: "first",
				profile_id: "p",
				profile_revision: 7,
				mode_id: "m",
				split_patches: [
					{ split: 0, universe: 1, address: 1 },
					{ split: 1, universe: 2, address: 1 },
				],
				multipatch: [
					{
						split_patches: [
							{ split: 0, universe: 1, address: 5 },
							{ split: 1, universe: null, address: null },
						],
					},
				],
			},
			{
				fixture_id: "unpatched",
				profile_id: "p",
				profile_revision: 7,
				mode_id: "m",
				split_patches: [],
			},
			{
				fixture_id: "visual",
				profile_id: "p",
				profile_revision: 7,
				mode_id: "visual",
				virtual_fixture_number: 1,
				split_patches: [{ split: 0, universe: 99, address: 1 }],
			},
			{
				fixture_id: "overlap",
				profile_id: "p",
				profile_revision: 7,
				mode_id: "m",
				split_patches: [{ split: 0, universe: 1, address: 1 }],
			},
		],
	};
}
const countingPlan = {
	dynamicTargetCount: 1001,
	activations: Array(20).fill({}),
};

test("actual v2 Patch counts unique patched slots, splits and multipatches while preserving show workload", () => {
	assert.deepEqual(summarizePerformancePatch(countingPatch(), countingPlan), {
		fixtureRecords: 4,
		physicalInstances: 5,
		parameterCount: 10,
		universes: 2,
		animatedAttributes: 1001,
		masterLanes: 20,
	});
});

test("missing immutable profile or mode cannot silently become zero parameters", () => {
	for (const mutate of [
		(p) => {
			p.profile_revisions = [];
		},
		(p) => {
			p.fixtures[0].profile_revision = 8;
		},
		(p) => {
			p.fixtures[0].mode_id = "missing";
		},
	]) {
		const patch = countingPatch();
		mutate(patch);
		assert.throws(
			() => summarizePerformancePatch(patch, countingPlan),
			/cannot resolve/,
		);
	}
});

test("virtual/unpatched nonzero profiles remain show records without inventing DMX output", () => {
	const patch = countingPatch();
	patch.fixtures = [patch.fixtures[1]];
	patch.fixtures[0].virtual_fixture_number = 3;
	assert.equal(
		summarizePerformancePatch(patch, countingPlan).parameterCount,
		0,
	);
	assert.equal(
		summarizePerformancePatch(patch, countingPlan).fixtureRecords,
		1,
	);
});

test("invalid assigned split, half-patch or universe overflow fails closed", () => {
	for (const assignment of [
		{ split: 99, universe: 1, address: 1 },
		{ split: 0, universe: null, address: 1 },
		{ split: 0, universe: 1, address: 511 },
	]) {
		const patch = countingPatch();
		patch.fixtures[0].split_patches = [assignment];
		assert.throws(
			() => summarizePerformancePatch(patch, countingPlan),
			/Invalid performance/,
		);
	}
});

test("older compact profile metadata counts without an optional full profile snapshot", () => {
	const patch = countingPatch();
	assert.equal(patch.profile_revisions[0].profile_snapshot, undefined);
	assert.equal(
		summarizePerformancePatch(patch, countingPlan).parameterCount,
		10,
	);
});

test("scheduler is verified after configuration and reports real enabled output routes", async () => {
	const configure = load(
		"configureScheduler",
		"\nasync function requireRendererOff(",
		{
			REQUESTED_RATE_HZ: 40,
			crypto: { randomUUID: () => "request" },
		},
	);
	for (const rate of [40, 60, undefined]) {
		const calls = [];
		const task = configure({
			request: async (method, path) => {
				calls.push([method, path]);
				if (path.endsWith("/update")) return {};
				if (path.endsWith("/configuration"))
					return { configuration: { frame_rate_hz: rate } };
				return {
					output_routes: [
						{ enabled: true, universe: 1 },
						{ enabled: true, universe: 1 },
						{ enabled: false, universe: 2 },
					],
				};
			},
		});
		if (rate !== 40) await assert.rejects(task, /configuration read back/);
		else {
			const result = await task;
			assert.equal(result.configured_rate_hz, 40);
			assert.equal(result.configured_output_route_count, 3);
			assert.equal(result.configured_enabled_output_route_count, 2);
			assert.equal(result.configured_output_universe_count, 1);
			assert.equal(calls[1][0], "GET");
		}
	}
});

test("renderer-disabled acknowledgement must come from a host observation", async () => {
	const verify = load("requireRendererOff", "\nasync function measureWindow(");
	const file = path.join(scratch, "mode-ack.jsonl");
	for (const acknowledgement of [
		{ stageEnabled: false, hostObservationCount: 1 },
		{ stageEnabled: true, hostObservationCount: 1 },
		{ stageEnabled: false, hostObservationCount: 0 },
	]) {
		await writeFile(
			file,
			`${JSON.stringify({ kind: "stage-mode-ack", ...acknowledgement })}\n`,
		);
		if (acknowledgement.stageEnabled || !acknowledgement.hostObservationCount)
			await assert.rejects(verify(file, {}), /did not acknowledge/);
		else await verify(file, {});
	}
	assert.match(source, /LIGHT_STAGE_PACKAGED_BENCH_STAGE: "0"/);
});

test("same-lock server timestamps remove asymmetric HTTP midpoint cadence bias", () => {
	const sample = (start, end, at, frames) =>
		readPerformanceCounterSample(diagnosticAt(at, { frames_sent: frames }), {
			utcStartMillis: epochBase + start,
			utcEndMillis: epochBase + end,
			monotonicStartMillis: start,
			monotonicEndMillis: end,
		});
	const first = sample(0, 1000, 900, 10);
	const last = sample(2000, 3000, 2100, 58);
	const delta = performanceCounterInterval(first, last);
	assert.equal(delta.seconds, 1.2);
	assert.equal(delta.frames_sent / delta.seconds, 40);
	assert.notEqual(delta.frames_sent / 2, 40);
	assert.throws(
		() => readPerformanceCounterSample({ output: {} }, first.bounds),
		/same-lock/,
	);
	assert.throws(() => performanceCounterInterval(last, first), /nonmonotonic/);
});

test("fixed duration stops polling by deadline and records the actual server window", async () => {
	let clock = 0;
	const measure = load("measureWindow", "\nfunction apiClient(", {
		DURATION_SECONDS: 2,
		performance: { now: () => clock },
		Date: simulatedDate(() => clock),
		setTimeout: (cb, millis) => {
			clock += millis;
			cb();
		},
		readLinuxProcessTree: async () => {
			clock += 300;
			return [];
		},
		processTreeResourceDelta: () => ({ cpuPercent: 0, residentBytes: 0 }),
		LINUX_PROCESS_OPTIONS: {},
		average: () => 0,
		percentile: (values) => values[0],
	});
	const measured = await measure(
		{
			request: async () => {
				const at = clock + 400;
				clock += 800;
				return diagnosticAt(at, { frames_sent: at / 25 });
			},
		},
		1,
		"unrestricted",
		[],
	);
	assert.equal(measured.observed_window_count, 1);
	assert.equal(measured.requested_measurement_seconds, 2);
	assert.equal(measured.measurement_seconds, 2.1);
	assert.equal(measured.average_completed_hz, 40);
	assert.equal(
		measured.counter_clock_source,
		"server_output_health_monotonic_micros",
	);
	assert.equal(measured.minimum_sample_window_seconds, 2.1);
	assert.equal(measured.maximum_sample_window_seconds, 2.1);
});

test("single-universe preparation verifies 24 actual records and 224 actual slots instead of trusting scene metadata", async () => {
	const profiles = [22, 30, 1].map((footprint, index) => ({
		profile_id: `p${index}`,
		profile_revision: 1,
		referenced_modes: [{ mode_id: "mode", splits: [{ split: 0, footprint }] }],
	}));
	let address = 1;
	const fixtures = Array.from({ length: 24 }, (_, index) => {
		const category = index < 4 ? 0 : index < 8 ? 1 : 2;
		const fixture = {
			fixture_id: `fixture${index}`,
			profile_id: `p${category}`,
			profile_revision: 1,
			mode_id: "mode",
			split_patches: [{ split: 0, universe: 101, address }],
		};
		address += [22, 30, 1][category];
		return fixture;
	});
	const patch = { fixtures, profile_revisions: profiles };
	const plan = { dynamicTargetCount: 188, activations: Array(20).fill({}) };
	for (const mismatch of [false, true]) {
		let patched = false,
			smallRequested = false;
		const prepare = load("prepareMixed", "\nasync function installDynamics(", {
			crypto: { randomUUID: () => "request" },
			createPerformanceFixtureInputs: () => {
				throw new Error("large factory must not serve the small case");
			},
			createSmallPerformanceFixtureInputs: () => {
				smallRequested = true;
				return { fixtures, patch: { occupiedSlots: 224, universeCount: 1 } };
			},
			createLargeStageDynamicsPlan: () => plan,
			installDynamics: async () => {},
		});
		const result = prepare(
			{
				request: async (method, path) => {
					if (path.endsWith("/bootstrap"))
						return { active_show: { id: "show" } };
					if (path.endsWith("/profiles")) return { profiles: [] };
					if (method === "POST") {
						patched = true;
						return {};
					}
					if (!patched) return { fixtures: [], patch_revision: 1 };
					const actual = structuredClone(patch);
					if (mismatch) actual.fixtures[0].split_patches = [];
					return actual;
				},
			},
			24,
			true,
		);
		if (mismatch)
			await assert.rejects(result, /differs from the requested scene/);
		else {
			const observed = await result;
			assert.equal(observed.fixtureRecords, 24);
			assert.equal(observed.parameterCount, 224);
			assert.equal(observed.universes, 1);
			assert.equal(observed.animatedAttributes, 188);
			assert.equal(observed.masterLanes, 20);
		}
		assert.equal(smallRequested, true);
	}
	assert.match(
		source,
		/caseId: "one_universe", fixtureRecords: 24, small: true/,
	);
});

test("reported scheduler cadence must match the verified configuration", () => {
	const bounds = {
		utcStartMillis: epochBase,
		utcEndMillis: epochBase,
		monotonicStartMillis: 0,
		monotonicEndMillis: 0,
	};
	assert.throws(
		() =>
			readPerformanceCounterSample(
				diagnosticAt(0, { frame_hz: 60 }),
				bounds,
				40,
			),
		/scheduler reports 60 Hz/,
	);
	assert.equal(
		readPerformanceCounterSample(diagnosticAt(0), bounds, 40).counters
			.frames_sent,
		0,
	);
});

function routeTestApi(universes, fault) {
	const objects = Array.from({ length: 9 }, (_, index) => ({
		id: `seed-${index + 1}`,
		revision: index + 3,
		body: {
			enabled: true,
			protocol: "art_net",
			logical_universe: index + 1,
			destination_universe: index + 1,
		},
	}));
	const calls = [];
	const patch = {
		profile_revisions: [
			{
				profile_id: "routed",
				profile_revision: 1,
				referenced_modes: [
					{
						mode_id: "mode",
						splits: [
							{ split: 0, footprint: 1 },
							{ split: 1, footprint: 0 },
						],
					},
				],
			},
		],
		fixtures: universes.map((universe) => ({
			fixture_id: `f${universe}`,
			profile_id: "routed",
			profile_revision: 1,
			mode_id: "mode",
			split_patches: [
				{ split: 0, universe, address: 1 },
				{ split: 1, universe: 999, address: 1 },
			],
		})),
	};
	return {
		objects,
		calls,
		async request(method, path, body) {
			calls.push({ method, path, body });
			if (path.endsWith("/patch")) return patch;
			if (path.endsWith("/objects/route"))
				return { objects: structuredClone(objects) };
			if (path.endsWith("/diagnostics")) {
				let routes = objects
					.filter((object) => object.body.enabled)
					.map(({ body }) => ({
						...body,
						universe: body.destination_universe,
					}));
				if (fault === "missing") routes = routes.slice(1);
				if (fault === "extra") routes.push({ ...routes[0], universe: 321 });
				return { output_routes: routes };
			}
			assert.equal(path, "/api/v2/output-routes/actions");
			const action = body.action;
			if (action.type === "update") {
				const object = objects.find((object) => object.id === action.route_id);
				assert.equal(action.expected_revision, object.revision);
				assert.deepEqual(action.patch, { enabled: false });
				object.body.enabled = false;
				object.revision++;
			} else {
				assert.equal(action.type, "create");
				objects.push({ id: action.route_id, revision: 1, body: action.route });
			}
			return {};
		},
	};
}

test("performance routes replace seeded universes1–9 with exact real101+ coverage", async () => {
	for (const universes of [[101], [101, 102, 116]]) {
		const api = routeTestApi(universes);
		const evidence = await configurePerformanceOutputRoutes(
			api,
			"127.0.0.1:19000",
		);
		assert.deepEqual(evidence.routed_patch_universes, universes);
		const enabled = api.objects.filter((object) => object.body.enabled);
		assert.equal(enabled.length, universes.length);
		assert.equal(
			api.objects.filter(
				(object) => object.id.startsWith("seed-") && object.body.enabled,
			).length,
			0,
		);
		for (const { body } of enabled) {
			assert.equal(body.destination, "127.0.0.1:19000");
			assert.equal(body.logical_universe, body.destination_universe);
			assert.equal(body.minimum_slots, 512);
			assert.equal(body.delivery_mode, "unicast");
		}
		assert.equal(
			api.calls.filter((call) => call.body?.action.type === "update").length,
			9,
		);
	}
});

test("performance route coverage fails if live diagnostics omit or add a universe", async () => {
	for (const fault of ["missing", "extra"])
		await assert.rejects(
			configurePerformanceOutputRoutes(
				routeTestApi([101, 102], fault),
				"127.0.0.1:19000",
			),
			/diagnostics disagree/,
		);
	const api = routeTestApi([101]);
	await assert.rejects(
		configurePerformanceOutputRoutes(api, "192.0.2.1:6454"),
		/owned loopback/,
	);
	assert.equal(api.calls.length, 0);
});

test("owned loopback receiver discards payloads, counts per universe and closes idempotently", async () => {
	const sink = await createPerformanceOutputSink();
	const sender = createSocket("udp4");
	try {
		sink.start([101]);
		const packet = Buffer.alloc(530);
		packet.write("Art-Net\0", 0, "binary");
		packet.writeUInt16LE(0x5000, 8);
		packet.writeUInt16BE(14, 10);
		packet.writeUInt16LE(101, 14);
		packet.writeUInt16BE(512, 16);
		const port = Number(sink.destination.split(":")[1]);
		await new Promise((resolve, reject) =>
			sender.send(packet, port, "127.0.0.1", (error) =>
				error ? reject(error) : resolve(),
			),
		);
		const deadline = Date.now() + 2000;
		while (sink.snapshot().packets_received < 1 && Date.now() < deadline)
			await new Promise((resolve) => setTimeout(resolve, 10));
		const received = sink.stop();
		assert.equal(received.packets_received, 1);
		assert.deepEqual(received.universes, [{ universe: 101, packets: 1 }]);
		assert.equal(received.payload_slots, 512);
		assert.equal(received.invalid_datagrams, 0);
		assert.equal(received.unexpected_universe_datagrams, 0);
	} finally {
		sender.close();
		await sink.close();
		await sink.close();
	}
});
