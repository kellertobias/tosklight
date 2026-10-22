#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import crypto from "node:crypto";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { finished } from "node:stream/promises";
import {
	performanceCounterInterval,
	readPerformanceCounterSample,
	requireNoPerformanceRenderer,
	summarizePerformancePatch as summarizePatch,
} from "./desktop-performance-patch.mjs";
import {
	configurePerformanceOutputRoutes,
	createPerformanceOutputSink,
} from "./desktop-performance-routes.mjs";
import {
	processTreeResourceDelta,
	readLinuxProcessTree,
} from "./process-tree-resources.mjs";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";
import {
	createPerformanceFixtureInputs,
	createSmallPerformanceFixtureInputs,
} from "./stage-large-scene.mjs";

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
const CASES = [
	{ caseId: "one_universe", fixtureRecords: 24, small: true },
	{ caseId: "demo", fixtureRecords: 264, demo: true },
	{ caseId: "sixteen_universe", fixtureRecords: 576 },
	{ caseId: "required_1024", fixtureRecords: 1_024 },
];

const options = parseArguments(process.argv.slice(2));
if (process.platform !== "linux")
	throw new Error(
		"The released Desk performance harness requires Linux /proc and Xvfb",
	);
await mkdir(path.resolve(options["output-dir"]), { recursive: true });
const results = [];
// Every show is measured unrestricted. The demo show is measured a second time locked to one
// core, which is the case that says whether a modest machine can still run a real show.
for (const performanceCase of CASES)
	for (const executionMode of performanceCase.demo
		? ["unrestricted", "one_core"]
		: ["unrestricted"])
		try {
			results.push(await runCase(performanceCase, executionMode));
		} catch (error) {
			await writeFile(
				path.resolve(options["output-dir"], "desktop-scenarios.partial.json"),
				`${JSON.stringify(
					{
						schema_version: 1,
						complete: false,
						scenarios: results,
						failed_case: {
							case_id: performanceCase.caseId,
							execution_mode: executionMode,
							error: error instanceof Error ? error.message : String(error),
						},
					},
					null,
					2,
				)}\n`,
			).catch((diagnosticError) => {
				console.error(
					`Partial desktop report unavailable: ${diagnosticError instanceof Error ? diagnosticError.message : String(diagnosticError)}`,
				);
			});
			throw error;
		}
await writeFile(
	path.resolve(options["output-dir"], "desktop-scenarios.json"),
	`${JSON.stringify({ schema_version: 1, scenarios: results }, null, 2)}\n`,
);

function parseArguments(arguments_) {
	const values = {};
	for (let index = 0; index < arguments_.length; index += 2) {
		const option = arguments_[index];
		const value = arguments_[index + 1];
		if (!option?.startsWith("--") || value == null)
			throw new Error(`invalid argument list near ${option ?? "<end>"}`);
		values[option.slice(2)] = value;
	}
	for (const required of ["application", "output-dir", "demo-show"])
		if (!values[required]) throw new Error(`--${required} is required`);
	return values;
}

async function runCase(performanceCase, executionMode) {
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
			LIGHT_STAGE_PACKAGED_BENCH_STAGE: "0",
			LIGHT_STAGE_PACKAGED_BENCH_FIXTURE_SHEET: "1",
			// The benchmark surface stops when its own duration elapses, and it defaults to
			// thirty seconds. Tell it how long this run actually measures for, with headroom
			// for the wait that precedes the window, or it falls silent midway through and
			// the liveness gate reports a Sheet that in fact did exactly as it was told.
			LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS: String(
				Math.ceil(drawBudgetMillis(performanceCase, executionMode) / 1_000) +
					DURATION_SECONDS +
					120,
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
	const evidence = await createCaseEvidence(temporaryRoot);
	evidence.closed = closed;
	let failure;
	let result;
	let packetSink;
	for (const stream of [child.stdout, child.stderr])
		stream.on("data", (chunk) => {
			evidence.log.write(chunk);
			output.push(chunk.toString());
			if (output.length > 100) output.shift();
		});
	try {
		await waitForReadiness(child, port);
		state.backendReady = true;
		state.phase = "show-preparation";
		const api = apiClient(port);
		// A v2 session is a client on a desk, not a named operator: the desk resolves which desk
		// the client belongs to and the request carries no user at all.
		const session = await api.request("POST", "/api/v2/sessions", {
			desk_id: null,
		});
		api.session = session;
		const prepared = performanceCase.demo
			? await prepareDemo(api)
			: await prepareMixed(
					api,
					performanceCase.fixtureRecords,
					performanceCase.small,
				);
		packetSink = await createPerformanceOutputSink();
		const outputRouteEvidence = await configurePerformanceOutputRoutes(
			api,
			packetSink.destination,
		);
		const schedulerConfiguration = await configureScheduler(api);
		// The show exists now, so let the benchmark surface begin.
		await writeFile(preparedPath, `${JSON.stringify(prepared)}\n`);
		state.prepared = true;
		state.phase = "fixture-sheet-readiness";
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
		state.phase = "renderer-disabled-verification";
		await requireRendererOff(reportPath, child);
		state.phase = "measurement";
		const measurementStartedAt = Date.now();
		packetSink.start(outputRouteEvidence.routed_patch_universes);
		const measured = await measureWindow(
			api,
			child.pid,
			executionMode,
			evidence.samples,
		);
		const receivedPackets = packetSink.stop();
		state.phase = "fixture-sheet-liveness";
		await requireFixtureSheetActive(
			reportPath,
			sheetRecords,
			executionMode,
			{
				startedAt: measurementStartedAt,
				endedAt: Date.now(),
			},
			(snapshot) => {
				state.sheet = { ...state.sheet, ...snapshot };
			},
		);
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
			...schedulerConfiguration,
			...outputRouteEvidence,
			loopback_packet_receipt: receivedPackets,
			below_target_hz: 38,
			measurement_seconds: DURATION_SECONDS,
			measurement_surface: "released-tauri-desk-fixture-sheet",
			stage_enabled: false,
			stage_mode_acknowledged: true,
			renderer_state: "disabled",
			renderer_process_verified_absent: true,
			...measured,
		};
	} catch (error) {
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
	try {
		failure = await finalizeCaseEvidence(
			child,
			evidence,
			reportPath,
			preparedPath,
			state,
			temporaryRoot,
			failure,
		);
		if (failure) throw failure;
		return result;
	} finally {
		await packetSink?.close();
	}
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
	try {
		await terminateProcessTree(child.pid);
	} catch (error) {
		failure ??= error;
	}
	let drainError;
	try {
		await drainCaseLog(evidence);
	} catch (error) {
		drainError = error;
		state.runtimeLogDrain = { complete: false, error: error.message };
		child.stdout.destroy();
		child.stderr.destroy();
		child.unref();
	}
	try {
		await finishCaseEvidence(evidence, reportPath, preparedPath, state);
		if (drainError) throw drainError;
		await rm(temporaryRoot, { recursive: true, force: true });
	} catch (error) {
		const message = `Runtime evidence unavailable at ${evidence.manifestPath}; scratch preserved at ${temporaryRoot}: ${error instanceof Error ? error.message : String(error)}`;
		if (failure) console.error(message);
		else failure = new Error(message);
	}
	return failure;
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
	files.measurement = `${evidence.prefix}-measurement.json`;
	await writeFile(
		files.measurement,
		`${JSON.stringify({ schema_version: 1, state, samples: evidence.samples }, null, 2)}\n`,
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

async function prepareMixed(api, fixtureRecords, small = false) {
	const bootstrap = await api.request("GET", "/api/v2/bootstrap");
	api.showId = bootstrap.active_show?.id;
	if (!api.showId) throw new Error("Desk has no active show");
	const before = await api.request("GET", "/api/v2/patch");
	const library = await api.request("GET", "/api/v2/fixture-library/profiles");
	const layerId = before.fixtures[0]?.layer_id ?? "default";
	const scene = small
		? createSmallPerformanceFixtureInputs(library.profiles, layerId)
		: createPerformanceFixtureInputs(library.profiles, fixtureRecords, layerId);
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
	const observed = summarizePatch(patch, plan);
	if (
		observed.fixtureRecords !== fixtureRecords ||
		observed.parameterCount !== scene.patch.occupiedSlots ||
		observed.universes !== scene.patch.universeCount
	)
		throw new Error(
			"Persisted performance Patch differs from the requested scene addresses",
		);
	return observed;
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

async function configureScheduler(api) {
	await api.request("POST", "/api/v2/configuration/update", {
		request_id: crypto.randomUUID(),
		patch: { frame_rate_hz: REQUESTED_RATE_HZ },
	});
	const snapshot = await api.request("GET", "/api/v2/configuration");
	const configured = snapshot?.configuration?.frame_rate_hz;
	if (configured !== REQUESTED_RATE_HZ)
		throw new Error(
			`Requested ${REQUESTED_RATE_HZ} Hz but configuration read back ${configured}`,
		);
	const diagnostics = await api.request("GET", "/api/v2/diagnostics");
	if (!Array.isArray(diagnostics?.output_routes))
		throw new Error(
			"Performance diagnostics lack configured output route evidence",
		);
	const enabled = diagnostics.output_routes.filter((route) => route.enabled);
	return {
		configured_rate_hz: configured,
		configuration_rate_verified: true,
		configured_output_route_count: diagnostics.output_routes.length,
		configured_enabled_output_route_count: enabled.length,
		configured_output_universe_count: new Set(
			enabled.map((route) => route.universe),
		).size,
		configured_output_routes: diagnostics.output_routes,
	};
}

async function requireRendererOff(reportPath, child) {
	const deadline = Date.now() + 15_000;
	while (Date.now() < deadline) {
		assertRunning(child);
		const records = (await readFile(reportPath, "utf8"))
			.split("\n")
			.flatMap((line) => {
				try {
					return [JSON.parse(line)];
				} catch {
					return [];
				}
			});
		const acknowledgement = records.findLast(
			(record) => record.kind === "stage-mode-ack",
		);
		if (acknowledgement) {
			if (
				acknowledgement.stageEnabled !== false ||
				!(acknowledgement.hostObservationCount > 0)
			)
				throw new Error(
					"Released Desk did not acknowledge renderer-disabled mode",
				);
			return;
		}
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
	throw new Error("Released Desk never acknowledged renderer-disabled mode");
}

async function measureWindow(api, rootPid, executionMode, samples) {
	const rates = [];
	const resources = [];
	let diagnosticsMonotonicStartMillis = performance.now();
	let diagnosticsUtcStartMillis = Date.now();
	let diagnostics = await api.request("GET", "/api/v2/diagnostics/performance");
	let diagnosticsUtcEndMillis = Date.now();
	let diagnosticsMonotonicEndMillis = performance.now();
	let counterSample = readPerformanceCounterSample(
		diagnostics,
		{
			utcStartMillis: diagnosticsUtcStartMillis,
			utcEndMillis: diagnosticsUtcEndMillis,
			monotonicStartMillis: diagnosticsMonotonicStartMillis,
			monotonicEndMillis: diagnosticsMonotonicEndMillis,
		},
		REQUESTED_RATE_HZ,
	);
	const openingCounterSample = counterSample;
	const measurementDeadline =
		diagnosticsMonotonicEndMillis + DURATION_SECONDS * 1000;
	const sampleDurations = [];
	const openingBands = diagnostics.output.frame_rate_band_counts ?? [];
	const openingFrames = diagnostics.output.frames_sent;
	const openingSchedulerRate = diagnostics.output.frame_hz ?? null;
	let processMonotonicStartMillis = performance.now();
	let processes = await readLinuxProcessTree(rootPid);
	requireNoPerformanceRenderer(processes);
	let processMonotonicEndMillis = performance.now();
	let processObservationMillis =
		(processMonotonicStartMillis + processMonotonicEndMillis) / 2;
	samples.push({
		phase: "opening",
		recordedAt: new Date().toISOString(),
		diagnosticsMonotonicStartMillis,
		diagnosticsMonotonicEndMillis,
		diagnosticsUtcStartMillis,
		diagnosticsUtcEndMillis,
		processMonotonicStartMillis,
		processMonotonicEndMillis,
		diagnostics,
		processes,
	});
	while (performance.now() < measurementDeadline) {
		const second = rates.length;
		await new Promise((resolve) =>
			setTimeout(
				resolve,
				Math.max(0, Math.min(1000, measurementDeadline - performance.now())),
			),
		);
		const diagnosticsStartedAt = new Date().toISOString();
		diagnosticsMonotonicStartMillis = performance.now();
		diagnosticsUtcStartMillis = Date.now();
		const nextDiagnostics = await api.request(
			"GET",
			"/api/v2/diagnostics/performance",
		);
		diagnosticsUtcEndMillis = Date.now();
		diagnosticsMonotonicEndMillis = performance.now();
		const diagnosticsEndedAt = new Date().toISOString();
		// HTTP bounds validate the server clock; cadence uses the same-lock server sample.
		const nextCounterSample = readPerformanceCounterSample(
			nextDiagnostics,
			{
				utcStartMillis: diagnosticsUtcStartMillis,
				utcEndMillis: diagnosticsUtcEndMillis,
				monotonicStartMillis: diagnosticsMonotonicStartMillis,
				monotonicEndMillis: diagnosticsMonotonicEndMillis,
			},
			REQUESTED_RATE_HZ,
		);
		const interval = performanceCounterInterval(
			counterSample,
			nextCounterSample,
		);
		const elapsed = interval.seconds;
		sampleDurations.push(elapsed);
		processMonotonicStartMillis = performance.now();
		const nextProcesses = await readLinuxProcessTree(rootPid);
		requireNoPerformanceRenderer(nextProcesses);
		processMonotonicEndMillis = performance.now();
		const nextProcessObservationMillis =
			(processMonotonicStartMillis + processMonotonicEndMillis) / 2;
		const processElapsed =
			(nextProcessObservationMillis - processObservationMillis) / 1_000;
		samples.push({
			phase: "measurement",
			second,
			elapsed,
			processElapsed,
			diagnosticsMonotonicStartMillis,
			diagnosticsMonotonicEndMillis,
			diagnosticsUtcStartMillis,
			diagnosticsUtcEndMillis,
			processMonotonicStartMillis,
			processMonotonicEndMillis,
			diagnosticsStartedAt,
			diagnosticsEndedAt,
			diagnostics: nextDiagnostics,
			processes: nextProcesses,
		});
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
		counterSample = nextCounterSample;
		processObservationMillis = nextProcessObservationMillis;
	}
	if (!rates.length)
		throw new Error(
			"No performance counter interval completed before the measurement deadline",
		);
	const measuredCounters = performanceCounterInterval(
		openingCounterSample,
		counterSample,
	);
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
		completed_frame_counter: "output.frames_sent",
		completed_counter_includes_action_wakeups: true,
		completed_frame_count: diagnostics.output.frames_sent - openingFrames,
		requested_measurement_seconds: DURATION_SECONDS,
		measurement_seconds: measuredCounters.seconds,
		observed_measurement_seconds: measuredCounters.seconds,
		completed_packet_count: measuredCounters.packets_sent,
		output_send_errors: measuredCounters.send_errors,
		output_deadline_misses: measuredCounters.deadline_misses,
		counter_clock_source: "server_output_health_monotonic_micros",
		rate_window_semantics: "sampled_server_counter_intervals",
		cadence_window_kind: "sampled-server-monotonic",
		measurement_duration_source: "server-monotonic",
		minimum_sample_window_seconds: Math.min(...sampleDurations),
		maximum_sample_window_seconds: Math.max(...sampleDurations),
		rate_window_durations_seconds: sampleDurations,
		observed_window_count: rates.length,
		scheduler_rate_hz_start: openingSchedulerRate,
		scheduler_rate_hz_end: diagnostics.output.frame_hz ?? null,
		minimum_one_second_completed_hz: sorted[0],
		average_completed_hz:
			measuredCounters.frames_sent / measuredCounters.seconds,
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
			});
			if (!response.ok)
				throw new Error(
					`${method} ${route} returned ${response.status}: ${await response.text()}`,
				);
			return response.status === 204 ? undefined : response.json();
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
