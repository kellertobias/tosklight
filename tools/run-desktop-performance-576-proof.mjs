#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import crypto from "node:crypto";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import net from "node:net";
import assert from "node:assert/strict";
import { chmod, realpath } from "node:fs/promises";
import { artifactPaths } from "./artifact-paths.mjs";
import { capturePerformanceSample, ServerCounterClock } from "./desktop-performance-576-clock.mjs";
import { writePhaseDiagnostics } from "./desktop-performance-576-phases.mjs";
import { writeTickTraceDiagnostics } from "./desktop-performance-576-tick-trace.mjs";
import { writeCpuDiagnostics } from "./desktop-performance-576-cpu.mjs";
import { writePoolDiagnostics } from "./desktop-performance-576-pool.mjs";
import { writeApiDiagnostics } from "./desktop-performance-576-api.mjs";
import { writeObserverDiagnostics } from "./desktop-performance-576-observer.mjs";
import { ThreadCpuDiagnostics, threadCpuEnabled, verifiedClockTicks, writeThreadCpuDiagnostics } from "./desktop-performance-576-thread-cpu.mjs";
import { expectedWorkerReceiptCount, WorkerCountReceipt, writeWorkerDiagnostics } from "./desktop-performance-576-workers.mjs";
import {
  verifyWorkloadFunctions, configurePacketProof, waitStageStarted,
  waitStageComplete, waitStageReady, summarizeStageWindow, ProofCheckpoint,
  PROOF_MODES, waitProofModeAck, summarizeStageAbsence, proofSurfaceDurationSeconds,
} from "./desktop-performance-576-proof-observers.mjs";
import path from "node:path";
import { finished } from "node:stream/promises";
import {
	processTreeResourceDelta,
	readLinuxProcessTree,
} from "./process-tree-resources.mjs";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";
import { createPerformanceFixtureInputs } from "./stage-large-scene.mjs";

// Each case is measured over a full minute: a desk is judged by how it holds up over a show, not
// over a moment, and a short window hides the occasional slow frame entirely.
const DURATION_SECONDS = 60;
// Every case requests the same rate, so results are comparable across shows and across releases.
const REQUESTED_RATE_HZ = 40;

/**
 * How long to wait for the Sheet to draw the show before the measured window opens. One core
 * draws at roughly a fixed rate, so the wait grows with the show rather than sitting at a
 * constant that only ever suited the smaller ones.
 */
function drawBudgetMillis(performanceCase, executionMode) {
	return executionMode === "one_core"
		? Math.max(300_000, performanceCase.fixtureRecords * 500)
		: 60_000;
}
const LINUX_PROCESS_OPTIONS = {
	ticksPerSecond: linuxConfiguration("CLK_TCK", 100),
	pageSize: linuxConfiguration("PAGESIZE", 4096),
};
// The Desk surface is measured up to a thousand fixtures. Larger shows are covered by the
// headless benchmark, which does not depend on a web view drawing every row.
const CASES = [{ caseId: "sixteen_universe", fixtureRecords: 576 }];

const options = parseArguments(process.argv.slice(2));
const requestedMode = options.mode ?? "combined";
assert(["control","stage","combined"].includes(requestedMode),"unknown proof mode");
const proofMode = {control:"control-no-stage",stage:"stage-functional",combined:"combined"}[requestedMode];
assert(Object.hasOwn(PROOF_MODES,proofMode),"unknown proof mode");
const stageEnabled = proofMode !== "control-no-stage";
const numericPath = path.resolve(options["numeric-report"]);
assert(numericPath.startsWith(`${path.resolve(artifactPaths.results)}${path.sep}`), "numeric output must be beneath canonical test results");
const phaseNumericPath = path.join(path.dirname(numericPath), "phase-numeric.json");
// Separate opt-in diagnostic only; ordinary Control/Stage perform no additional probes.
const threadCpu = threadCpuEnabled(process.env,proofMode)
  ? new ThreadCpuDiagnostics(verifiedClockTicks(spawnSync("getconf",["CLK_TCK"],{encoding:"utf8",timeout:1000}))) : null;
let threadCpuNumericPath;
if(threadCpu) {
  assert(process.env.LIGHT_OUTPUT_THREAD_CPU_REPORT,"thread CPU report destination required");
  threadCpuNumericPath=path.resolve(process.env.LIGHT_OUTPUT_THREAD_CPU_REPORT);
  const proofRoot=path.dirname(path.dirname(numericPath));
  assert(threadCpuNumericPath.startsWith(`${path.resolve(artifactPaths.results)}${path.sep}`),"thread CPU report must be beneath canonical results");
  assert(!threadCpuNumericPath.startsWith(`${proofRoot}${path.sep}`),"thread CPU report must remain outside strict proof inventory");
  await writeThreadCpuDiagnostics(threadCpuNumericPath,threadCpu).catch(()=>{});
}
async function retainPhaseDiagnostics(file, window) {
  // This observer is diagnostic only: unavailable data or a write error never changes Control gates.
  await writePhaseDiagnostics(phaseNumericPath, file, window, {fullFrequency:process.env.LIGHT_OUTPUT_PHASE_DIAGNOSTICS === "all"}).catch(() => {});
  if (process.env.LIGHT_OUTPUT_PHASE_DIAGNOSTICS === "all") {
    await writeTickTraceDiagnostics(path.join(path.dirname(numericPath), "tick-trace-numeric.json"), file, window, {
      healthDeadlineMisses: checkpoint.result.deadline_miss_delta, completedFrames: checkpoint.result.completed_frame_delta,
    }).catch(() => {});
    await writeCpuDiagnostics(path.join(path.dirname(numericPath), "cpu-numeric.json"), file, window).catch(() => {});
    await writePoolDiagnostics(path.join(path.dirname(numericPath), "pool-numeric.json"), file, window).catch(() => {});
    await writeObserverDiagnostics(path.join(path.dirname(numericPath), "observer-numeric.json"), file, window).catch(() => {});
  }
}
const checkpoint = new ProofCheckpoint(numericPath);
checkpoint.result.proof_mode_code = PROOF_MODES[proofMode];
await checkpoint.enter(1);
await retainPhaseDiagnostics(null, null);
let published;
try {
  assert.equal(process.platform,"linux","The released Desk performance harness requires Linux /proc and Xvfb");
  await verifyWorkloadFunctions({prepareMixed, installDynamics, summarizePatch});
  await mkdir(path.resolve(options["output-dir"]), { recursive: true, mode: 0o700 });
  const privateRoot = await realpath(path.resolve(options["output-dir"]));
  const canonicalTmp = await realpath(artifactPaths.tmp);
  assert(privateRoot.startsWith(`${canonicalTmp}${path.sep}`), "private output must be beneath canonical tmp");
  assert(numericPath !== privateRoot && !numericPath.startsWith(`${privateRoot}${path.sep}`), "public report overlaps private output");
  await chmod(privateRoot,0o700);
  await checkpoint.enter(2);
  const result = await runCase(CASES[0],"unrestricted");
  await checkpoint.enter(14);
  await writeFile(path.resolve(options["output-dir"],"desktop-scenarios.json"),`${JSON.stringify({schema_version:1,scenarios:[result]},null,2)}\n`);
  published = await checkpoint.finish(1);
} catch(error) {
  await checkpoint.fail(checkpoint.progress.current_stage_code || 15);
  throw error;
}
assert.equal(published.all_gates_passed,1,"focused576 performance proof gates failed");

function parseArguments(arguments_) {
	const values = {};
	for (let index = 0; index < arguments_.length; index += 2) {
		const option = arguments_[index];
		const value = arguments_[index + 1];
		if (!option?.startsWith("--") || value == null)
			throw new Error(`invalid argument list near ${option ?? "<end>"}`);
		values[option.slice(2)] = value;
	}
	for (const required of ["application", "output-dir", "demo-show", "numeric-report"])
		if (!values[required]) throw new Error(`--${required} is required`);
	return values;
}

async function runCase(performanceCase, executionMode) {
  // Validate a proof-only expectation before launching any application process.
  const expectedWorkers=expectedWorkerReceiptCount(process.env);
	const temporaryRoot = await mkdtemp(
		path.join(
			path.resolve(options["output-dir"]),
			`${performanceCase.caseId}-`,
		),
	);
	const port = await freePort();
	const reportPath = path.join(temporaryRoot, "fixture-sheet.jsonl");
	// A bundle launched with a benchmark report runs the benchmark surface instead of the
	// operator interface, and that surface waits for this marker before it starts measuring.
	const preparedPath = path.join(temporaryRoot, "stage-profile-prepared");
	const invocation = applicationInvocation(
		path.resolve(options.application),
		executionMode,
	);
	const child = spawn(invocation.command, invocation.arguments, {
		cwd: process.cwd(),
		detached: true,
		env: {
			...process.env,
			APPIMAGE_EXTRACT_AND_RUN: "1",
			LIGHT_DESKTOP_TEST_BIND: `127.0.0.1:${port}`,
			LIGHT_DESKTOP_TEST_DATA_DIR: path.join(temporaryRoot, "data"),
			LIGHT_STAGE_PACKAGED_BENCH_PREPARED: preparedPath,
			LIGHT_STAGE_PACKAGED_BENCH_REPORT: reportPath,
			LIGHT_STAGE_PACKAGED_BENCH_PROFILE: performanceCase.caseId,
			LIGHT_STAGE_PACKAGED_BENCH_ADDITIONAL_STAGE_WINDOW: "0",
			LIGHT_STAGE_PACKAGED_BENCH_FIXTURE_SHEET: "1",
      LIGHT_STAGE_PACKAGED_BENCH_STAGE: stageEnabled ? "1" : "0",
			// The benchmark surface stops when its own duration elapses, and it defaults to
			// thirty seconds. Tell it how long this run actually measures for, with headroom
			// for the wait that precedes the window, or it falls silent midway through and
			// the liveness gate reports a Sheet that in fact did exactly as it was told.
			LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS: String(
        proofSurfaceDurationSeconds(stageEnabled,drawBudgetMillis(performanceCase, executionMode),DURATION_SECONDS),
			),
			// The demo show carries whatever it was generated with, which is not ours to
			// predict before it is loaded. The built workloads are exactly the size we ask for.
			...(performanceCase.demo
				? {}
				: {
						LIGHT_STAGE_PACKAGED_BENCH_EXPECTED_FIXTURE_RECORDS: String(
							performanceCase.fixtureRecords,
						),
					}),
		},
		stdio: ["ignore", "pipe", "pipe"],
	});
	const closed = new Promise((resolve) => child.once("close", resolve));
	child.once("error", (error) => {
		child.spawnError = error;
	});
	const state = {
		phase: "backend-readiness",
		startedAt: new Date().toISOString(),
		backendReady: false,
		prepared: false,
		sheet: null,
	};
	const output = [];
	let evidence;
  try { evidence = await createCaseEvidence(temporaryRoot); }
  catch(error) { await terminateProcessTree(child.pid); throw error; }
	evidence.closed = closed;
  if(threadCpu) evidence.threadCpu=threadCpu;
  const workerReceipt=expectedWorkers===null?null:new WorkerCountReceipt(expectedWorkers);
  const workerNumericPath=path.join(path.dirname(numericPath),"worker-numeric.json");
  if(workerReceipt) {
    evidence.workerReceipt=workerReceipt;
    await writeWorkerDiagnostics(workerNumericPath,workerReceipt.summary()).catch(()=>{});
  }
	let failure;
	let result;
  let packetProof;
  let phaseWindow = null;
  let watchdogFailure;
  const watchdog = setTimeout(() => {
    watchdogFailure = new Error("Owned desktop proof exceeded 400-second lifetime");
    void terminateProcessTree(child.pid).catch(() => {});
  }, 400_000);
	for (const stream of [child.stdout, child.stderr])
		stream.on("data", (chunk) => {
			evidence.log.write(chunk);
			output.push(chunk.toString());
			if (output.length > 100) output.shift();
		});
	try {
		await waitForReadiness(child, port);
		state.backendReady = true;
    await checkpoint.enter(3);
		state.phase = "show-preparation";
		const api = apiClient(port);
		// A v2 session is a client on a desk, not a named operator: the desk resolves which desk
		// the client belongs to and the request carries no user at all.
		const session = await api.request("POST", "/api/v2/sessions", {
			desk_id: null,
		});
		api.session = session;
		await configureScheduler(api);
		const prepared = performanceCase.demo
			? await prepareDemo(api)
			: await prepareMixed(api, performanceCase.fixtureRecords);
		assert.deepEqual(prepared, {fixtureRecords:576,physicalInstances:576,parameterCount:8064,universes:16,animatedAttributes:5408,masterLanes:20});
    Object.assign(checkpoint.result,{fixture_count:prepared.fixtureRecords,physical_instance_count:prepared.physicalInstances,parameter_count:prepared.parameterCount,universes:prepared.universes,animated_attribute_count:prepared.animatedAttributes,master_lane_count:prepared.masterLanes});
    await checkpoint.enter(4);
    packetProof = await configurePacketProof(api, port);
    // The show exists now, so let the unchanged benchmark surface begin.
		await writeFile(preparedPath, `${JSON.stringify(prepared)}\n`);
		state.prepared = true;
		state.phase = "fixture-sheet-readiness";
    await checkpoint.enter(5);
		// The Sheet counts the fixtures it shows, and a show holds what it could actually be
		// given: measure against the patch that exists rather than the size that was ordered.
		const sheetRecords = performanceCase.demo ? null : prepared.fixtureRecords;
		// One core draws a show at roughly a fixed rate, so the wait for it has to grow with
		// the show rather than sit at a constant that only ever suited the smaller ones.
		await waitForFixtureSheet(
			reportPath,
			sheetRecords,
			child,
			drawBudgetMillis(performanceCase, executionMode),
			(snapshot) => {
				state.sheet = snapshot;
			},
		);
    await checkpoint.enter(6);
    if(proofMode !== "combined") {
      await waitProofModeAck(reportPath,stageEnabled,() => assertRunning(child));
      checkpoint.result.mode_acknowledged=1;
    }
    if(stageEnabled) {
    await waitStageStarted(reportPath, () => assertRunning(child));
    // Original surface resizes at 8s, releases live at 15s and recreates at 16s.
    // Readiness here is provisional: final per-pane telemetry validates the window retrospectively.
    await new Promise(resolve => setTimeout(resolve, 17_000));
    // A lifecycle delay is not a presentation acknowledgment. The private readiness witness
    // precedes this window; final per-pane coverage still validates the actual window.
    await waitStageReady(reportPath, () => assertRunning(child));
    }
    assertRunning(child);
    await checkpoint.enter(7);
    packetProof.start();
		state.phase = "measurement";
    if(workerReceipt) {
      // Thread enumeration stays outside the fixed 61 server-counter observations.
      await workerReceipt.observe(await readLinuxProcessTree(child.pid),child.pid);
      await writeWorkerDiagnostics(workerNumericPath,workerReceipt.summary()).catch(()=>{});
    }
		const measurementStartedAt = Date.now();
    const measurementMonotonicStart = performance.now();
    const counterClock = new ServerCounterClock();
    let measured;
    try {
      measured = await measureWindow(api, child.pid, executionMode, evidence.samples, counterClock, threadCpu);
    } finally {
      // Preserve partial clock evidence even when a server observation fails validation.
      Object.assign(checkpoint.result, counterClock.summary());
      await checkpoint.save();
    }
    const measurementEndedAt = Date.now();
    // The health counter's same-lock UTC sample defines the packet and trace window.
    // UDP receivedAt remains the JS receive callback clock, not a kernel arrival timestamp.
    phaseWindow = counterClock.window();
    measured.packet_observation_window = {...phaseWindow};
    measured.wall_monotonic_difference_ms = Math.abs((measurementEndedAt-measurementStartedAt)-(performance.now()-measurementMonotonicStart));
    Object.assign(checkpoint.result,measured);
    if(workerReceipt) {
      // Bind the final existing owned-tree snapshot and recheck its live process identity.
      await workerReceipt.observe(evidence.samples.at(-1).processes,child.pid);
      await writeWorkerDiagnostics(workerNumericPath,workerReceipt.summary()).catch(()=>{});
    }
    await checkpoint.enter(8);
    const packets = packetProof.snapshot(phaseWindow);
    checkpoint.result.proof.packets = packets;
    await checkpoint.save();
    // Retain the frozen window after packet capture closes; parsing must not block live UDP proof.
    const interaction = await packetProof.interact(async (phase,partial) => {
      if(partial) checkpoint.result.proof.interaction = partial;
      await checkpoint.enter(phase);
    });
    checkpoint.result.proof.interaction = interaction;
    await checkpoint.save();
    await packetProof.close();
    await writeFile(path.join(temporaryRoot,"packet-proof-private.json"), JSON.stringify(packetProof.privateState()), {flag:"wx",mode:0o600});
		state.phase = "fixture-sheet-liveness";
    await checkpoint.enter(11);
		await requireFixtureSheetActive(
			reportPath,
			sheetRecords,
			executionMode,
			{
				startedAt: measurementStartedAt,
				endedAt: measurementEndedAt,
			},
			(snapshot) => {
				state.sheet = { ...state.sheet, ...snapshot };
			},
		);
    checkpoint.progress.sheet_liveness_checked=1;
    checkpoint.progress.sheet_liveness_passed=1;
    await checkpoint.enter(12);
    const complete = await waitStageComplete(reportPath, () => assertRunning(child));
    await checkpoint.enter(13);
    if(proofMode !== "combined") {
      assert.equal(complete.stageEnabled,stageEnabled,"complete mode mismatch");
      assert.equal(complete.config?.stageEnabled,stageEnabled,"complete configuration mismatch");
    }
    const stage = stageEnabled ? summarizeStageWindow(complete, measurementStartedAt, measurementEndedAt) : undefined;
    const absence = stageEnabled ? undefined : summarizeStageAbsence(complete,measurementStartedAt,measurementEndedAt,evidence.samples);
    if(stage) checkpoint.result.proof.stage = stage;
    if(absence) checkpoint.result.proof.absence = absence;
    await checkpoint.save();
    assert.equal(stageEnabled ? stage.validation_passed : absence.passed,1,stageEnabled ? "Stage retrospective coverage failed" : "Stage absence observation failed");
    if (watchdogFailure) throw watchdogFailure;
		state.phase = "complete";
		result = {
			case_id: performanceCase.caseId,
			case_name: performanceCase.demo
				? `Demo show — ${prepared.parameterCount.toLocaleString("en-US")} parameters / ${prepared.fixtureRecords.toLocaleString("en-US")} fixtures`
				: `${prepared.parameterCount.toLocaleString("en-US")} parameters / ${prepared.fixtureRecords.toLocaleString("en-US")} fixtures`,
			execution_mode: executionMode,
			cpu_limit: executionMode === "one_core" ? 1 : null,
			fixture_count: prepared.fixtureRecords,
			physical_instance_count: prepared.physicalInstances,
			parameter_count: prepared.parameterCount,
			universes: prepared.universes,
			animated_attribute_count: prepared.animatedAttributes,
			master_lane_count: prepared.masterLanes,
			requested_rate_hz: REQUESTED_RATE_HZ,
			below_target_hz: 38,
			measurement_seconds: DURATION_SECONDS,
			measurement_surface: "released-tauri-desk-fixture-sheet",
			...measured,
      proof_mode_code: PROOF_MODES[proofMode], mode_acknowledged: checkpoint.result.mode_acknowledged ?? 0,
      proof: {packets, interaction, ...(stage ? {stage} : {absence})},
		};
	} catch (error) {
    if(checkpoint.progress.current_stage_code === 11) checkpoint.progress.sheet_liveness_checked=1;
    await checkpoint.fail();
		const diagnostics = await retainCaseFailure({
			performanceCase,
			executionMode,
			state,
			reportPath,
			preparedPath,
			child,
			output,
			error: error instanceof Error ? error.message : String(error),
		}).catch(
			(diagnosticError) =>
				`unavailable (${diagnosticError instanceof Error ? diagnosticError.message : String(diagnosticError)})`,
		);
		failure = new Error(
			`${performanceCase.caseId}/${executionMode}: ${error instanceof Error ? error.message : String(error)}; diagnostics: ${diagnostics}${output.length ? `\n${output.join("")}` : ""}`,
		);
	}
  clearTimeout(watchdog);
  await checkpoint.enter(14);
  let cleanupPassed = 1;
  try { await packetProof?.close(); } catch (error) { cleanupPassed=0;failure ??= error;await checkpoint.fail(14); }
  if (packetProof && failure) {
    await writeFile(path.join(temporaryRoot,"packet-proof-failure-private.json"), JSON.stringify(packetProof.privateState()), {flag:"wx",mode:0o600}).catch(() => {});
  }
	const finalized = await finalizeCaseEvidence(
		child,
		evidence,
		reportPath,
		preparedPath,
		state,
		temporaryRoot,
		failure,
	);
  await retainPhaseDiagnostics(evidence.sidecarLogPath, phaseWindow);
  if(threadCpu) await writeThreadCpuDiagnostics(threadCpuNumericPath,threadCpu).catch(()=>{});
  failure = finalized.failure;
  cleanupPassed = Number(cleanupPassed && finalized.cleanupPassed);
  await checkpoint.finish(cleanupPassed);
	if (failure) throw failure;
	return result;
}

async function finalizeCaseEvidence(
	child,
	evidence,
	reportPath,
	preparedPath,
	state,
	temporaryRoot,
	failure,
) {
  let cleanupPassed=1;
	try {
		await terminateProcessTree(child.pid);
	} catch (error) {
    cleanupPassed=0;
		failure ??= error;
	}
	let drainError;
	try {
		await drainCaseLog(evidence);
	} catch (error) {
    cleanupPassed=0;
		drainError = error;
		state.runtimeLogDrain = { complete: false, error: error.message };
		child.stdout.destroy();
		child.stderr.destroy();
		child.unref();
	}
	try {
		await finishCaseEvidence(evidence, reportPath, preparedPath, state);
		if (drainError) throw drainError;
		// Keep bounded private evidence under canonical tmp for independent review.
	} catch (error) {
    cleanupPassed=0;
		const message = `Runtime evidence unavailable at ${evidence.manifestPath}; scratch preserved at ${temporaryRoot}: ${error instanceof Error ? error.message : String(error)}`;
		if (failure) console.error(message);
		else failure = new Error(message);
	}
	return {failure,cleanupPassed};
}

async function drainCaseLog(evidence) {
	let timer;
	try {
		await Promise.race([
			evidence.closed,
			new Promise((_, reject) => {
				timer = setTimeout(
					() =>
						reject(
							new Error(
								"Owned child output pipes did not close within 5000ms after termination; runtime log may be incomplete",
							),
						),
					5000,
				);
			}),
		]);
	} finally {
		clearTimeout(timer);
	}
}

async function createCaseEvidence(temporaryRoot) {
	const prefix = path.join(
		path.resolve(options["output-dir"]),
		path.basename(temporaryRoot),
	);
	const runtimeLog = `${prefix}-runtime.log`;
	const log = createWriteStream(runtimeLog, { flags: "wx" });
	// Observe errors immediately, even if setup fails before the finalizer awaits this stream.
	const completion = finished(log).then(
		() => null,
		(error) => error,
	);
	return {
		prefix,
		runtimeLog,
		sidecarLogPath: path.join(temporaryRoot, "data", "light-headless.log"),
		log,
		completion,
		samples: [],
		manifestPath: `${prefix}-evidence.json`,
	};
}

async function finishCaseEvidence(evidence, reportPath, preparedPath, state) {
	evidence.log.end();
	const logError = await evidence.completion;
	if (logError) throw logError;
	const files = { runtimeLog: evidence.runtimeLog };
	for (const [key, suffix, source] of [
		["fixtureSheet", "fixture-sheet.jsonl", reportPath],
		["prepared", "prepared.json", preparedPath],
		["sidecarLog", "light-headless.log", evidence.sidecarLogPath],
	]) {
		if (!source) {
			files[key] = { unavailable: "not_recorded" };
			continue;
		}
		let contents;
		try {
			contents = await readFile(source);
		} catch (error) {
			if (error.code !== "ENOENT") throw error;
			files[key] = { unavailable: error.code };
			continue;
		}
		files[key] = `${evidence.prefix}-${suffix}`;
		await writeFile(files[key], contents, { flag: "wx" });
	}
	// Summarize the same 61 captured responses after the window; never issue extra requests.
	await writeApiDiagnostics(path.join(path.dirname(numericPath), "api-numeric.json"), evidence.samples).catch(() => {});
	files.measurement = `${evidence.prefix}-measurement.json`;
	await writeFile(
		files.measurement,
		`${JSON.stringify({ schema_version: 1, state, samples: evidence.samples, ...(evidence.workerReceipt?{workerReceipt:evidence.workerReceipt.privateState()}:{}), ...(evidence.threadCpu?{threadCpu:evidence.threadCpu.privateState()}:{}) }, null, 2)}\n`,
		{ flag: "wx" },
	);
	await writeFile(
		evidence.manifestPath,
		`${JSON.stringify(files, null, 2)}\n`,
		{ flag: "wx" },
	);
}

async function retainCaseFailure({
	performanceCase,
	executionMode,
	state,
	reportPath,
	preparedPath,
	child,
	output,
	error,
}) {
	const directory = await mkdtemp(
		path.join(
			path.resolve(options["output-dir"]),
			`${performanceCase.caseId}-${executionMode}-failure-`,
		),
	);
	const files = {};
	for (const [name, source] of [
		["fixture-sheet.jsonl", reportPath],
		["prepared.json", preparedPath],
	]) {
		try {
			await writeFile(path.join(directory, name), await readFile(source));
			files[name] = path.join(directory, name);
		} catch (error) {
			files[name] = { unavailable: error.code ?? "read_failed" };
		}
	}
	const log = path.join(directory, "process.log");
	await writeFile(log, output.join(""));
	files.processLog = log;
	// Best effort: screenshot availability never determines whether a readiness gate passes.
	const screenshotPath = path.join(directory, "desktop.png");
	const screenshot = spawnSync("import", ["-window", "root", screenshotPath], {
		timeout: 5000,
		encoding: "utf8",
	});
	files.screenshot =
		screenshot.status === 0
			? screenshotPath
			: {
					unavailable: screenshot.error?.code ?? "capture_failed",
					status: screenshot.status,
				};
	const report = path.join(directory, "failure.json");
	await writeFile(
		report,
		`${JSON.stringify(
			{
				schema_version: 1,
				complete: false,
				case_id: performanceCase.caseId,
				execution_mode: executionMode,
				failedAt: new Date().toISOString(),
				error,
				state,
				process: {
					pid: child.pid,
					exitCode: child.exitCode,
					signalCode: child.signalCode,
				},
				files,
				thresholdsUnchanged: true,
			},
			null,
			2,
		)}\n`,
	);
	return report;
}

function applicationInvocation(application, executionMode) {
	if (executionMode !== "one_core")
		return { command: application, arguments: [] };
	const affinity = spawnSync(
		"taskset",
		["--pid", "--cpu-list", String(process.pid)],
		{
			encoding: "utf8",
		},
	);
	const cpu = affinity.stdout?.match(/:\s*(\d+)/u)?.[1] ?? "0";
	return { command: "taskset", arguments: ["--cpu-list", cpu, application] };
}

async function prepareDemo(api) {
	const dataBase64 = (
		await readFile(path.resolve(options["demo-show"]))
	).toString("base64");
	const created = await api.request("POST", "/api/v2/shows", {
		request_id: crypto.randomUUID(),
		action: {
			type: "create",
			name: `Performance demo ${crypto.randomUUID()}`,
			data_base64: dataBase64,
			overwrite: false,
		},
	});
	const showId = created?.result?.show?.id;
	if (!showId) throw new Error("demo import returned no show identity");
	await api.request("POST", "/api/v2/shows", {
		request_id: crypto.randomUUID(),
		action: { type: "open", show_id: showId, transition: "safe_blackout" },
	});
	api.showId = showId;
	const patch = await api.request("GET", "/api/v2/patch");
	const dynamicFixtureIds = patch.fixtures
		.filter((fixture) => fixture.fixture_number != null)
		.map((fixture) => fixture.fixture_id);
	const plan = createLargeStageDynamicsPlan(patch, {
		dynamicFixtureIds,
		staticControlFixtureIds: [],
	});
	await installDynamics(api, plan);
	return summarizePatch(patch, plan);
}

async function prepareMixed(api, fixtureRecords) {
	const bootstrap = await api.request("GET", "/api/v2/bootstrap");
	api.showId = bootstrap.active_show?.id;
	if (!api.showId) throw new Error("Desk has no active show");
	const before = await api.request("GET", "/api/v2/patch");
	const library = await api.request("GET", "/api/v2/fixture-library/profiles");
	const scene = createPerformanceFixtureInputs(
		library.profiles,
		fixtureRecords,
		before.fixtures[0]?.layer_id ?? "default",
	);
	await api.request(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: scene.fixtures,
			remove_fixture_ids: before.fixtures.map((fixture) => fixture.fixture_id),
			placements: [],
		},
		before.patch_revision,
	);
	const patch = await api.request("GET", "/api/v2/patch");
	const plan = createLargeStageDynamicsPlan(patch, scene);
	await installDynamics(api, plan);
	return {
		...summarizePatch(patch, plan),
		parameterCount: scene.patch.occupiedSlots,
		universes: scene.patch.universeCount,
	};
}

async function installDynamics(api, plan) {
	for (const activation of plan.activations) {
		const created = await api.request("POST", "/api/v2/dynamics/create", {
			request_id: crypto.randomUUID(),
			definition: activation.definition,
		});
		await api.request(
			"POST",
			`/api/v2/dynamics/${encodeURIComponent(created.object.id)}/start`,
			{
				targets: activation.targets,
				overrides: {
					size: 1,
					speed_multiplier: { numerator: 1, denominator: 1 },
					phase_offset_degrees: 0,
				},
				timing: {},
				undo_group: "fixture-sheet-performance",
			},
		);
	}
}

function summarizePatch(patch, plan) {
	const universes = new Set();
	let parameters = 0;
	let physicalInstances = 0;
	for (const fixture of patch.fixtures) {
		// A fixture the show carries without a resolved profile has no footprint to count,
		// which is ordinary rather than a reason to abandon the measurement.
		const definition = fixture.definition;
		const mode = definition?.profile_snapshot?.modes?.find(
			(candidate) => candidate.id === definition.mode_id,
		);
		const footprint =
			mode?.splits?.reduce((total, split) => total + split.footprint, 0) ?? 0;
		const instances = [fixture, ...(fixture.multipatch ?? [])];
		physicalInstances += instances.length;
		parameters += footprint * instances.length;
		for (const instance of instances)
			for (const split of instance.split_patches ?? [])
				if (split.universe != null) universes.add(split.universe);
	}
	return {
		fixtureRecords: patch.fixtures.length,
		physicalInstances,
		parameterCount: parameters,
		universes: universes.size,
		animatedAttributes: plan.dynamicTargetCount,
		masterLanes: plan.activations.length,
	};
}

async function configureScheduler(api) {
	await api.request("POST", "/api/v2/configuration/update", {
		request_id: crypto.randomUUID(),
		patch: { frame_rate_hz: REQUESTED_RATE_HZ },
	});
}

async function measureWindow(api, rootPid, executionMode, samples, counterClock, threadCpu = null) {
	const rates = [];
	const resources = [];
  const observe = async (phase, second) => {
    const capture = await capturePerformanceSample(api);
    // Retain the actual response and both client clock envelopes before validation can fail.
    const record = {phase, ...(second === undefined ? {} : {second}), ...capture,
      recordedAt: new Date().toISOString(),
      diagnosticsStartedAt: new Date(capture.diagnosticsUtcStartMillis).toISOString(),
      diagnosticsEndedAt: new Date(capture.diagnosticsUtcEndMillis).toISOString()};
    samples.push(record);
    const {elapsedSeconds} = counterClock.observe(capture);
    return {record, elapsedSeconds};
  };
  const opening = await observe("opening");
  let diagnostics = opening.record.diagnostics;
	const openingBands = diagnostics.output.frame_rate_band_counts ?? [];
	let processMonotonicStartMillis = performance.now();
	let processes = await readLinuxProcessTree(rootPid);
	let processMonotonicEndMillis = performance.now();
	let processObservationMillis =
		(processMonotonicStartMillis + processMonotonicEndMillis) / 2;
  Object.assign(opening.record, {processMonotonicStartMillis, processMonotonicEndMillis, processes});
  if(threadCpu) await threadCpu.observe(processes,rootPid,opening.record);
	for (let second = 0; second < DURATION_SECONDS; second++) {
		await new Promise((resolve) => setTimeout(resolve, 1_000));
    const observation = await observe("measurement", second);
    const nextDiagnostics = observation.record.diagnostics;
    const elapsed = observation.elapsedSeconds;
		processMonotonicStartMillis = performance.now();
		const nextProcesses = await readLinuxProcessTree(rootPid);
		processMonotonicEndMillis = performance.now();
		const nextProcessObservationMillis =
			(processMonotonicStartMillis + processMonotonicEndMillis) / 2;
		const processElapsed =
			(nextProcessObservationMillis - processObservationMillis) / 1_000;
    Object.assign(observation.record, {elapsed, processElapsed,
      processMonotonicStartMillis, processMonotonicEndMillis, processes: nextProcesses});
    if(threadCpu) await threadCpu.observe(nextProcesses,rootPid,observation.record);
		rates.push(
			(nextDiagnostics.output.frames_sent - diagnostics.output.frames_sent) /
				elapsed,
		);
		const server = (entries) =>
			entries.filter((entry) => entry.command.includes("light-headless"));
		const native = (entries) =>
			entries.filter((entry) => entry.command.includes("viz-renderer"));
		const ui = (entries) =>
			entries.filter((entry) => entry.command.startsWith("WebKit"));
		const host = (entries) =>
			entries.filter(
				(entry) =>
					!entry.command.includes("light-headless") &&
					!entry.command.includes("viz-renderer") &&
					!entry.command.startsWith("WebKit"),
			);
		const resourceDelta = (before, after) =>
			processTreeResourceDelta(
				before,
				after,
				processElapsed,
				LINUX_PROCESS_OPTIONS,
			);
		resources.push({
			application: resourceDelta(processes, nextProcesses),
			server: resourceDelta(server(processes), server(nextProcesses)),
			ui: resourceDelta(ui(processes), ui(nextProcesses)),
			native: resourceDelta(native(processes), native(nextProcesses)),
			host: resourceDelta(host(processes), host(nextProcesses)),
		});
		diagnostics = nextDiagnostics;
		processes = nextProcesses;
		processObservationMillis = nextProcessObservationMillis;
	}
	const sorted = [...rates].sort((left, right) => left - right);
	// The band counts run from show start, so the window's histogram is the difference between
	// the reading taken before the first second and the reading taken after the last.
	const bandCounts = (openingBands ?? []).map((count, index) =>
		Math.max(
			0,
			(diagnostics.output.frame_rate_band_counts?.[index] ?? 0) - count,
		),
	);
	const cpu = resources.map((sample) =>
		executionMode === "one_core"
			? Math.min(100, sample.application.cpuPercent)
			: sample.application.cpuPercent,
	);
	const boundedCpu = (value) =>
		executionMode === "one_core" ? Math.min(100, value) : value;
	const component = (name) => ({
		cpu_average_percent: average(
			resources.map((sample) => boundedCpu(sample[name].cpuPercent)),
		),
		cpu_max_percent: Math.max(
			...resources.map((sample) => boundedCpu(sample[name].cpuPercent)),
		),
		peak_resident_bytes: Math.max(
			...resources.map((sample) => sample[name].residentBytes),
		),
	});
	const cpuSorted = [...cpu].sort((left, right) => left - right);
	return {
    ...counterClock.summary(),
    counter_observation_count: samples.length,
    counter_window_seconds: counterClock.elapsedSeconds(),
    completed_frame_delta: counterClock.delta("frames_sent"),
    deadline_miss_delta: counterClock.delta("deadline_misses"),
    send_error_delta: counterClock.delta("send_errors"),
		minimum_one_second_completed_hz: sorted[0],
		average_completed_hz: average(rates),
		// The p95 of a frame rate is its slow tail: the rate 95% of windows stayed at or above.
		// Taking the fast end instead would let a show that averages 60 Hz claim a p95 of 80.
		p95_one_second_completed_hz: percentile(sorted, 5),
		maximum_one_second_completed_hz: sorted.at(-1),
		frame_rate_band_bounds_hz:
			diagnostics.output.frame_rate_band_bounds_hz ?? [],
		frame_rate_band_counts: bandCounts,
		windows_below_target: rates.filter((rate) => rate < 38).length,
		resources: {
			application_cpu_average_percent: average(cpu),
			application_cpu_max_percent: Math.max(...cpu),
			// The headline CPU number is the p95 busiest second, not the single worst spike.
			application_cpu_p95_percent: percentile(cpuSorted, 95),
			application_peak_resident_bytes: Math.max(
				...resources.map((sample) => sample.application.residentBytes),
			),
			server: component("server"),
			desktop_webview: component("ui"),
			native_renderer: component("native"),
			desktop_host: component("host"),
			playwright: { launched: false },
			measurement:
				"Released Tauri Desk process tree: desktop host, WebKit, native Stage renderers, and bundled light-headless sidecar; excludes the Node coordinator and Xvfb",
		},
	};
}

function apiClient(port) {
	return {
		session: null,
		showId: null,
		async request(method, route, body, revision) {
			const headers = {};
			if (body !== undefined) headers["content-type"] = "application/json";
			if (this.session) {
				headers.authorization = `Bearer ${this.session.token}`;
				headers["x-tosk-desk"] = this.session.desk.id;
			}
			if (this.showId) headers["x-tosk-show"] = this.showId;
			if (revision !== undefined) headers["if-match"] = String(revision);
			const response = await fetch(`http://127.0.0.1:${port}${route}`, {
				method,
				headers,
				body: body === undefined ? undefined : JSON.stringify(body),
        signal: AbortSignal.timeout(15_000), redirect: "error",
			});
			if (!response.ok)
				throw new Error(
					`${method} ${route} returned ${response.status}: ${await response.text()}`,
				);
			let size = 0; const chunks = [];
      for await (const chunk of response.body ?? []) {
        size += chunk.length; assert(size <= 32*1024*1024, "API response cap"); chunks.push(chunk);
      }
      return response.status === 204 ? undefined : JSON.parse(Buffer.concat(chunks).toString("utf8"));
		},
	};
}

async function waitForReadiness(child, port) {
	const deadline = Date.now() + 60_000;
	while (Date.now() < deadline) {
		assertRunning(child);
		try {
			const response = await fetch(
				`http://127.0.0.1:${port}/api/v2/readiness`,
				{
					signal: AbortSignal.timeout(2_000),
				},
			);
			if (response.ok) return;
		} catch {}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error("released Desk did not reach readiness");
}

async function waitForFixtureSheet(
	reportPath,
	expected,
	child,
	budgetMs,
	observe = () => {},
) {
	const deadline = Date.now() + budgetMs;
	while (Date.now() < deadline) {
		assertRunning(child);
		const records = await readFile(reportPath, "utf8").catch(() => "");
		const diagnostics = records
			.trim()
			.split("\n")
			.filter(Boolean)
			.flatMap((line) => {
				try {
					return [JSON.parse(line)];
				} catch {
					return [];
				}
			});
		const ready = diagnostics.find(
			(record) =>
				record.kind === "fixture-sheet-ready" &&
				(expected === null || record.fixtureRecords === expected),
		);
		const readiness =
			diagnostics.findLast(
				(record) => record.kind === "fixture-sheet-readiness",
			) ?? null;
		const readyAppendFailure =
			diagnostics.findLast(
				(record) => record.kind === "fixture-sheet-ready-append-failed",
			) ?? null;
		const surface =
			diagnostics.findLast(
				(record) =>
					record.kind === "fixture-sheet-container-mounted" ||
					record.kind === "fixture-sheet-container-unmounted",
			) ?? null;
		const renderFailure =
			diagnostics.findLast(
				(record) => record.kind === "fixture-sheet-render-failed",
			) ?? null;
		const lifecycleFailure = ready
			? (diagnostics
					.slice(diagnostics.indexOf(ready) + 1)
					.find(
						(record) => record.kind === "fixture-sheet-container-unmounted",
					) ?? null)
			: null;
		observe({
			observedAt: new Date().toISOString(),
			expectedFixtureRecords: expected,
			reportPresent: records.length > 0,
			ready:
				Boolean(ready) &&
				!renderFailure &&
				!lifecycleFailure &&
				surface?.kind !== "fixture-sheet-container-unmounted",
			surface,
			readiness,
			renderFailure,
			lifecycleFailure,
			readyAppendFailure,
			lastRecords: records
				.trim()
				.split("\n")
				.filter(Boolean)
				.slice(-12)
				.flatMap((line) => {
					try {
						const record = JSON.parse(line);
						return [
							{
								kind: record.kind,
								fixtureRecords: record.fixtureRecords,
								rowCount: record.rowCount,
								recordedAt: record.recordedAt,
							},
						];
					} catch {
						return [];
					}
				}),
			unobservedPredicates: [
				...(readiness
					? []
					: [
							"frontend active",
							"nonempty rows",
							"active-values loading",
							"group-runtime loading",
							"benchmark configuration",
						]),
				...(readyAppendFailure ? [] : ["Tauri append failure"]),
			],
		});
		if (renderFailure)
			throw new Error(
				`Fixture Sheet render failed: ${renderFailure.errorName ?? "Unknown"}: ${renderFailure.errorMessage ?? "unavailable"}`,
			);
		if (
			lifecycleFailure ||
			(ready && surface?.kind === "fixture-sheet-container-unmounted")
		)
			throw new Error(
				"Fixture Sheet container unmounted during the active benchmark",
			);
		if (ready) return;
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	// Say what the Sheet did report, so a mismatch names itself instead of being guessed at.
	const seen = await sheetReadyCounts(reportPath);
	throw new Error(
		expected === null
			? "Fixture Sheet never reported itself ready"
			: `Fixture Sheet did not converge to ${expected} fixture records (it reported ${seen.length ? seen.join(", ") : "nothing"})`,
	);
}

async function sheetReadyCounts(reportPath) {
	const records = await readFile(reportPath, "utf8").catch(() => "");
	return [
		...new Set(
			records
				.trim()
				.split("\n")
				.filter(Boolean)
				.flatMap((line) => {
					try {
						return [JSON.parse(line)];
					} catch {
						return [];
					}
				})
				.filter((record) => record.kind?.startsWith("fixture-sheet"))
				.map((record) => record.fixtureRecords),
		),
	];
}

async function requireFixtureSheetActive(
	reportPath,
	expected,
	executionMode,
	measurement,
	observe = () => {},
) {
	const records = (await readFile(reportPath, "utf8"))
		.trim()
		.split("\n")
		.filter(Boolean)
		.flatMap((line) => {
			try {
				return [JSON.parse(line)];
			} catch {
				return [];
			}
		});
	const renderFailure =
		records.findLast(
			(record) => record.kind === "fixture-sheet-render-failed",
		) ?? null;
	const surface =
		records.findLast(
			(record) =>
				record.kind === "fixture-sheet-container-mounted" ||
				record.kind === "fixture-sheet-container-unmounted",
		) ?? null;
	const readyIndex = records.findIndex(
		(record) =>
			record.kind === "fixture-sheet-ready" &&
			(expected === null || record.fixtureRecords === expected),
	);
	const lifecycleFailure =
		readyIndex === -1
			? null
			: (records
					.slice(readyIndex + 1)
					.find(
						(record) => record.kind === "fixture-sheet-container-unmounted",
					) ?? null);
	observe({
		observedAt: new Date().toISOString(),
		surface,
		renderFailure,
		lifecycleFailure,
		readiness:
			records.findLast((record) => record.kind === "fixture-sheet-readiness") ??
			null,
		ready:
			records.some(
				(record) =>
					record.kind === "fixture-sheet-ready" &&
					(expected === null || record.fixtureRecords === expected),
			) &&
			!renderFailure &&
			!lifecycleFailure &&
			surface?.kind !== "fixture-sheet-container-unmounted",
	});
	// Explicit workload loss is a failure even inside the scheduling-noise tolerance below.
	// This runs before owned-process termination, so normal terminal cleanup is not judged.
	if (renderFailure)
		throw new Error(
			`Fixture Sheet render failed: ${renderFailure.errorName ?? "Unknown"}: ${renderFailure.errorMessage ?? "unavailable"}`,
		);
	if (lifecycleFailure || surface?.kind === "fixture-sheet-container-unmounted")
		throw new Error(
			"Fixture Sheet container unmounted during the active benchmark",
		);
	const beats = records
		.filter(
			(record) =>
				record.kind === "fixture-sheet-heartbeat" &&
				(expected === null || record.fixtureRecords === expected),
		)
		.map((record) => Date.parse(record.recordedAt))
		.filter((at) => Number.isFinite(at))
		.sort((left, right) => left - right);
	// The Sheet beats once a second, and a desk deliberately confined to one core delivers
	// those beats less punctually. Late is what this case is measuring; silent is the fault,
	// so this tolerates ordinary scheduling noise and only calls a long silence a death.
	const tolerance = executionMode === "one_core" ? 30_000 : 10_000;
	// Judge continuity across the window rather than how recent the final beat happens to be.
	// A last-beat check reads one instant: it failed a run whose Sheet had beaten steadily
	// for a minute but hiccupped at the very end, and passed one that fell silent in the
	// middle and recovered just in time.
	const covering = beats.filter(
		(at) =>
			at >= measurement.startedAt - tolerance &&
			at <= measurement.endedAt + tolerance,
	);
	const edges = [measurement.startedAt, ...covering, measurement.endedAt];
	let widestSilence = 0;
	for (let index = 1; index < edges.length; index += 1)
		widestSilence = Math.max(widestSilence, edges[index] - edges[index - 1]);
	if (covering.length && widestSilence <= tolerance) return;
	const seconds = (value) => `${(value / 1_000).toFixed(1)}s`;
	// A silence has several possible causes and they are told apart by the shape of the
	// beats, not by the summary. Print the timeline so one run settles it: when the beats
	// stop, whether they were ever spaced a second apart, and what each one believed the
	// show contained.
	const all = records.filter(
		(record) => record.kind === "fixture-sheet-heartbeat",
	);
	const timeline = all
		.map((record) => {
			const at = Date.parse(record.recordedAt) - measurement.startedAt;
			return `${(at / 1_000).toFixed(1)}s/${record.fixtureRecords}r`;
		})
		.join(" ");
	// One line: a multi-line failure is easy to lose to log grouping and truncation, and this
	// message is the only evidence a CI-only measurement leaves behind.
	throw new Error(
		`Fixture Sheet did not remain active through the timed measurement window: ${covering.length} beats across ${seconds(measurement.endedAt - measurement.startedAt)}, widest silence ${seconds(widestSilence)}, tolerated ${seconds(tolerance)}; expected fixture records ${expected === null ? "any" : expected}; beats (offset/records) ${all.length ? timeline : "none"}`,
	);
}

function assertRunning(child) {
	if (child.spawnError) throw child.spawnError;
	if (child.exitCode !== null || child.signalCode !== null)
		throw new Error(
			`released Desk exited early (${child.exitCode ?? child.signalCode})`,
		);
}

async function freePort() {
	const server = net.createServer();
	await new Promise((resolve, reject) => {
		server.once("error", reject);
		server.listen(0, "127.0.0.1", resolve);
	});
	const address = server.address();
	if (!address || typeof address === "string") throw new Error("no free port");
	await new Promise((resolve) => server.close(resolve));
	return address.port;
}

async function terminateProcessTree(pid) {
	if (!pid) return;
	try {
		process.kill(-pid, "SIGTERM");
	} catch {}
	await new Promise((resolve) => setTimeout(resolve, 500));
	try {
		process.kill(-pid, "SIGKILL");
	} catch {}
}

function average(values) {
	return values.reduce((total, value) => total + value, 0) / values.length;
}

function linuxConfiguration(name, fallback) {
	const result = spawnSync("getconf", [name], { encoding: "utf8" });
	const value = Number.parseInt(result.stdout?.trim() ?? "", 10);
	return Number.isSafeInteger(value) && value > 0 ? value : fallback;
}

function percentile(sorted, percentile_) {
	return sorted[
		Math.min(
			sorted.length - 1,
			Math.ceil((percentile_ / 100) * sorted.length) - 1,
		)
	];
}
