import assert from "node:assert/strict";
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
	assert.equal(observed.readyFixtureRecords, 264);
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
	const observations = [];
	await requireActive(p, 3, "unrestricted", {
		startedAt,
		endedAt: startedAt + 60_000,
	}, observations.push.bind(observations));
	assert.equal(observations.at(-1).heartbeatCount, 61);
	assert.equal(observations.at(-1).livenessPassed, true);
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
	assert.equal(observed.readinessTimedOut, true);
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
	for (const fail of [false, true, "spawn", "render"]) {
		const child = Object.assign(new EventEmitter(), {
			pid: 999,
			stdout: new EventEmitter(),
			stderr: new EventEmitter(),
		});
		const files = [];
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
			configureScheduler: async () => {},
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
			fail === "spawn" ? 0 : 1,
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
		setTimeout: (cb) => {
			clock += 1000;
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
			request: async () => ({
				output: {
					frames_sent: 5 + 40 * reads++,
					send_errors: 7,
					frame_rate_band_counts: [1],
				},
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
	const processes = [
		"light-headless",
		"viz-renderer",
		"WebKitWebProces",
		"light-desktop",
	].map((command, index) => ({ command, pid: index + 1 }));
	const measure = load("measureWindow", "\nfunction apiClient(", {
		DURATION_SECONDS: 2,
		performance: { now: () => clock },
		setTimeout: (cb) => {
			clock += 1000;
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
				return {
					output: {
						frames_sent: (snapshot * 40) / 1000,
						frame_rate_band_counts: [1],
					},
				};
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
	assert.equal(samples[2].elapsed, 1.525);
	assert.equal(samples[1].processElapsed, 1.35);
	assert.equal(samples[2].processElapsed, 1.7);
	assert.equal(samples[0].diagnosticsMonotonicStartMillis, 0);
	assert.equal(samples[0].diagnosticsMonotonicEndMillis, 250);
	assert.equal(
		samples[2].diagnosticsMonotonicEndMillis -
			samples[2].diagnosticsMonotonicStartMillis,
		600,
	);
	assert.ok(cpuIntervals.includes(1.7));
	assert.equal(result.resources.application_cpu_average_percent, 4);
	assert.equal(result.resources.server.cpu_average_percent, 1);
	assert.equal(result.resources.desktop_webview.cpu_average_percent, 1);
	assert.equal(result.resources.native_renderer.cpu_average_percent, 1);
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

test("native renderer CPU is separate from WebKit and host while total stays complete", async () => {
	let clock = 0;
	let reads = 0;
	const processes = [
		"light-headless",
		"viz-renderer",
		"WebKitWebProces",
		"light-desktop",
	].map((command, index) => ({ command, pid: index + 1 }));
	const measure = load("measureWindow", "\nfunction apiClient(", {
		DURATION_SECONDS: 1,
		performance: { now: () => clock },
		setTimeout: (cb) => {
			clock += 1000;
			cb();
		},
		readLinuxProcessTree: async () => processes,
		processTreeResourceDelta: (_before, after) => ({
			cpuPercent: after.length,
			residentBytes: after.length,
		}),
		LINUX_PROCESS_OPTIONS: {},
		average: (a) => a.reduce((x, y) => x + y, 0) / a.length,
		percentile: (a) => a[0],
	});
	const result = await measure(
		{
			request: async () => ({
				output: {
					frames_sent: 40 * reads++,
					frame_rate_band_counts: [1],
				},
			}),
		},
		999,
		"unrestricted",
		[],
	);
	assert.equal(result.resources.desktop_webview.cpu_average_percent, 1);
	assert.equal(result.resources.native_renderer.cpu_average_percent, 1);
	assert.equal(result.resources.desktop_host.cpu_average_percent, 1);
	assert.equal(result.resources.server.cpu_average_percent, 1);
	assert.equal(result.resources.application_cpu_average_percent, 4);
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

test(
  "explicit CPU off/on select only unchanged 576 unrestricted case; ordinary run retains four cases",
	async () => {
	  const start = source.indexOf("const results = [];");
		const end = source.indexOf("\nfunction parseArguments(", start);
		const casesStart = source.indexOf("const CASES = [");
		const casesEnd = source.indexOf("\n];", casesStart) + 3;
		const cases = vm.runInNewContext(`${source.slice(casesStart, casesEnd)}\nCASES`);

		for (const mode of [undefined, "off", "on"]) {
		  const observed = [];

			await vm.runInNewContext(`(async()=>{${source.slice(start, end)}})()`, {
			  CASES: cases,

			  options: {
				  "output-dir": scratch,
				  "typed-cpu-diagnostics": mode
				},

			  path,
			  JSON,
			  writeFile: async () => {},
			  runCase: async (performanceCase, executionMode) => observed.push([performanceCase.caseId, performanceCase.fixtureRecords, executionMode])
			});

			assert.deepEqual(observed, mode ? [["sixteen_universe", 576, "unrestricted"]] : [
			  ["demo", 264, "unrestricted"],
			  ["demo", 264, "one_core"],
			  ["sixteen_universe", 576, "unrestricted"],
			  ["required_1024", 1024, "unrestricted"]
			]);
		}

		assert.match(source, /const DURATION_SECONDS = 60;/);
		assert.match(source, /const REQUESTED_RATE_HZ = 40;/);
	}
);

test(
  "CPU finalization exports after owned termination and drain, before deletion; original failures survive",
	async () => {
	  for (const scenario of ["success", "workload", "export", "drain"]) {
		  const order = [], original = scenario === "workload" ? new Error("original workload gate") : undefined;

			const evidence = {
			  manifestPath: "private/evidence.json",

			  cpu: {
				  finish: async () => {
					  order.push("export");

						if (scenario === "export")
						  throw new Error("numeric validation refused");
					}
				}
			};

			const finalize = load("finalizeCaseEvidence", "\nasync function drainCaseLog(", {
			  terminateProcessTree: async () => {
				  order.push("terminate");
				},

			  drainCaseLog: async () => {
				  order.push("drain");

					if (scenario === "drain")
					  throw new Error("drain unavailable");
				},

			  finishCaseEvidence: async () => {
				  order.push("private-evidence");
				},

			  rm: async () => {
				  order.push("rm");
				},

			  console: {
				  error: () => {}
				}
			});

			const child = {
			  pid: 100,

			  stdout: {
				  destroy() {}
				},

			  stderr: {
				  destroy() {}
				},

			  unref() {}
			};

			const outcome = await finalize(child, evidence, "sheet", "prepared", {}, "private/root", original);

			if (scenario === "success") {
			  assert.equal(outcome, undefined);
				assert.deepEqual(order, ["terminate", "drain", "private-evidence", "export", "rm"]);
			}

			if (scenario === "workload") {
			  assert.equal(outcome, original);
				assert.ok(!order.includes("rm"));
			}

			if (scenario === "export" || scenario === "drain") {
			  assert.ok(outcome);
				assert.ok(!order.includes("rm"));
			}

			if (scenario === "drain")
			  assert.ok(!order.includes("export"));
		}
	}
);

test("CPU capture failure cannot replace an existing workload failure", async () => {
  const original = new Error("original workload failure");

	const finalize = load("finalizeCaseEvidence", "\nasync function drainCaseLog(", {
	  terminateProcessTree: async () => {},
	  drainCaseLog: async () => {},
	  finishCaseEvidence: async () => {},

	  rm: async () => {
		  throw new Error("must retain private scratch");
		},

	  console: {
		  error: () => {}
		}
	});

	const result = await finalize({
	  pid: 100
	}, {
	  manifestPath: "private",

	  cpu: {
		  finish: async () => {
			  throw new Error("bad capture");
			}
		}
	}, "sheet", "prepared", {}, "private-root", original);

	assert.equal(result, original);
});
