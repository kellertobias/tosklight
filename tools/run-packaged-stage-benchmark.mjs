import { execFile, spawn } from "node:child_process";
import { readFile, stat, unlink, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { promisify } from "node:util";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";
import {
	histogramPercentileMicros,
	outputWindow,
} from "./output-histogram.mjs";
import {
	CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS,
	packagedStageControlDurationSeconds,
	packagedStageProfile,
	packagedStageSceneFailures,
} from "./packaged-stage-profile.mjs";
import {
	latestProgrammerActionId,
	summarizeProgrammerActionTiming,
} from "./programmer-action-timing.mjs";
import { startSlowVisualizationClient } from "./slow-visualization-client.mjs";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";
import {
	changingPresentationGaps,
	frameOverlapsApplicationSuspend,
	frameOverlapsContextRecovery,
	frameOverlapsShowSwitch,
	laneSourceCadenceGaps,
	latestChangingFrameDidNotSettle,
} from "./stage-frame-continuity.mjs";
import {
	countFixtureInstances,
	createDeterministicLargeStageInputs,
	LARGE_STAGE_DYNAMIC_INSTANCES,
	LARGE_STAGE_FIXTURE_INSTANCES,
	LARGE_STAGE_FIXTURE_RECORDS,
} from "./stage-large-scene.mjs";
import { summarizeStageLongRunResources } from "./stage-resource-stability.mjs";

const execFileAsync = promisify(execFile);

const durationSeconds = positiveInteger(
	process.argv[2] ?? "30",
	"duration seconds",
);
const profile = process.argv[3] ?? "default-stage";
const profileDefinition = packagedStageProfile(profile);
const controlDurationSeconds =
	packagedStageControlDurationSeconds(durationSeconds);
const application = packagedApplication();
const executable = application.executable;
await stat(executable).catch(() => {
	throw new Error("Build and open the debug app first with `npm run open`");
});

const stamp = new Date().toISOString().replaceAll(/[:.]/g, "-");
const samplesPath = path.join(
	artifactPaths.performance,
	"stage",
	`packaged-tauri-${profile}-${stamp}.jsonl`,
);
const reportPath = samplesPath.replace(/\.jsonl$/, ".json");
const dataPath = path.join(
	artifactPaths.performance,
	"stage",
	`packaged-tauri-${stamp}-data`,
);
const preparedPath = path.join(dataPath, "stage-profile-prepared");
await unlink(samplesPath).catch(() => undefined);
await unlink(preparedPath).catch(() => undefined);
await stopExistingDevelopmentDesk();

const benchmarkEnvironment = {
	LIGHT_STAGE_PACKAGED_BENCH_REPORT: samplesPath,
	LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS: String(durationSeconds),
	LIGHT_STAGE_PACKAGED_BENCH_CONTROL_DURATION_SECONDS: String(
		controlDurationSeconds,
	),
	LIGHT_STAGE_PACKAGED_BENCH_PROFILE: profile,
	LIGHT_STAGE_PACKAGED_BENCH_PREPARED: preparedPath,
	LIGHT_DESKTOP_TEST_DATA_DIR: dataPath,
};
const app = launchPackagedApplication(application, benchmarkEnvironment);

let records;
let scene;
let runtime;
let slowClient;
let showSwitchResult;
let applicationSuspendResult;
let memoryPhase = "startup";
const processMemory = [];
let memorySampleRunning = false;
const collectMemory = async () => {
	if (memorySampleRunning) return;
	memorySampleRunning = true;
	try {
		const residentBytes = await lightDesktopResidentBytes(
			application.direct ? app.pid : undefined,
		);
		if (residentBytes !== null)
			processMemory.push({
				recordedAt: Date.now(),
				phase: memoryPhase,
				residentBytes,
			});
	} finally {
		memorySampleRunning = false;
	}
};
const memorySampler = setInterval(() => void collectMemory(), 1_000);
try {
	await requireReadiness(app, dataPath);
	await collectMemory();
	const prepared = await prepareScene(profile);
	scene = prepared.scene;
	const before = await runtimeDiagnostics(prepared.session);
	const programmerTimingExercise = await exercisePackagedProgrammerTiming(
		prepared.session,
	);
	if (profile === "improved-beam-spike") {
		memoryPhase = "improved-beam-spike";
		await writeFile(preparedPath, `${JSON.stringify(scene)}\n`);
		records = await waitForComplete(samplesPath, 120_000);
		const after = await runtimeDiagnostics(prepared.session);
		runtime = {
			before,
			afterNoStage: before,
			after,
			programmerTimingExercise,
		};
	} else {
		memoryPhase = "no-stage";
		await writeFile(preparedPath, `${JSON.stringify(scene)}\n`);
		await waitForRecord(
			samplesPath,
			(record) => record.kind === "stage-started",
			(controlDurationSeconds + 30) * 1_000,
		);
		await activatePackagedApplication(application);
		const afterNoStage = await runtimeDiagnostics(prepared.session);
		memoryPhase = "stage";
		if (profile === "large-stage")
			slowClient = await startSlowVisualizationClient(
				"http://127.0.0.1:5000",
				prepared.session.token,
			);
		const showSwitch = exerciseShowSwitches(
			prepared.session,
			prepared.showSwitch,
			durationSeconds,
		);
		const applicationSuspend = exerciseApplicationSuspend(
			application,
			app,
			durationSeconds,
		);
		records = await waitForComplete(
			samplesPath,
			(controlDurationSeconds + durationSeconds + 30) * 1_000,
		);
		showSwitchResult = await showSwitch;
		applicationSuspendResult = await applicationSuspend;
		await slowClient?.close();
		slowClient = undefined;
		await new Promise((resolve) => setTimeout(resolve, 250));
		const after = await runtimeDiagnostics(prepared.session);
		runtime = {
			before,
			afterNoStage,
			after,
			showSwitch: showSwitchResult,
			applicationSuspend: applicationSuspendResult,
			programmerTimingExercise,
		};
	}
} finally {
	clearInterval(memorySampler);
	await collectMemory();
	await slowClient?.close();
	app.kill("SIGTERM");
	await stopPackagedDesktop();
}

const terminalError = records.findLast((record) => record.kind === "error");
if (terminalError)
	throw new Error(
		`Packaged Stage shell failed: ${terminalError.message ?? "unknown error"}`,
	);
const complete = records.findLast((record) => record.kind === "complete");
if (!complete)
	throw new Error("Packaged Stage report has no completion record");
const result =
	profile === "improved-beam-spike"
		? evaluateImprovedBeamSpike(
				complete,
				samplesPath,
				scene,
				runtime,
				processMemory,
			)
		: evaluate(
				complete,
				durationSeconds,
				samplesPath,
				profile,
				scene,
				runtime,
				processMemory,
			);
await writeFile(reportPath, `${JSON.stringify(result, null, 2)}\n`);
console.log(`Created ${reportPath}`);
if (!result.acceptanceGateEnforced) {
	for (const failure of result.failures)
		console.error(`Stage gate: ${failure}`);
	process.exitCode = 1;
}

async function requireReadiness(app, dataDirectory) {
	const deadline = Date.now() + 60_000;
	while (Date.now() < deadline) {
		if (app.exitCode !== null)
			throw new Error(
				`Packaged ToskLight exited during startup; inspect ${path.join(dataDirectory, "light-headless.log")}`,
			);
		const response = await fetch(
			"http://127.0.0.1:5000/api/v2/readiness",
		).catch(() => null);
		if (response?.ok) return;
		await new Promise((resolve) => setTimeout(resolve, 100));
	}
	throw new Error(
		`Packaged ToskLight did not become ready; inspect ${path.join(dataDirectory, "light-headless.log")}`,
	);
}

async function prepareScene(profile) {
	const session = await requestJson("POST", "/api/v2/sessions", {
		username: "Operator",
		desk_id: null,
	});
	const bootstrap = await requestJson("GET", "/api/v2/bootstrap");
	let showId = bootstrap.active_show?.id;
	if (!showId) throw new Error("Packaged Stage benchmark has no active Show");
	if (profile === "canonical-demo")
		showId = await openCanonicalDemoShow(session);
	const before = await requestJson("GET", "/api/v2/patch", undefined, {
		session,
		showId,
	});
	if (profile === "default-stage" || profile === "canonical-demo") {
		const scene = summarizeScene(profile, before.fixtures);
		const inventoryFailures = packagedStageSceneFailures(profile, scene);
		if (inventoryFailures.length > 0)
			throw new Error(inventoryFailures.join("; "));
		const benchmarkLook =
			profile === "canonical-demo"
				? await startCanonicalDemoBenchmarkLook(session, showId)
				: null;
		const showSwitch = await createShowSwitchTarget(session, showId, profile);
		if (profile === "canonical-demo") {
			await requestJson(
				"POST",
				"/api/v2/shows",
				{
					request_id: crypto.randomUUID(),
					action: {
						type: "open",
						show_id: showSwitch.alternateShowId,
						transition: "safe_blackout",
						transition_millis: null,
					},
				},
				{ session },
			);
			await startCanonicalDemoBenchmarkLook(
				session,
				showSwitch.alternateShowId,
			);
			await requestJson(
				"POST",
				"/api/v2/shows",
				{
					request_id: crypto.randomUUID(),
					action: {
						type: "open",
						show_id: showId,
						transition: "safe_blackout",
						transition_millis: null,
					},
				},
				{ session },
			);
			await startCanonicalDemoBenchmarkLook(session, showId);
		}
		return {
			scene: {
				...scene,
				profileLabel: packagedStageProfile(profile).label,
				benchmarkLook,
				source:
					profile === "canonical-demo"
						? "assets/demo.show"
						: "development default show",
			},
			session,
			showSwitch: {
				...showSwitch,
				canonicalDemo: profile === "canonical-demo",
			},
		};
	}
	const fixtureLibrary = await requestJson(
		"GET",
		"/api/v2/fixture-library/profiles",
		undefined,
		{ session },
	);
	const largeScene = createDeterministicLargeStageInputs(
		before.fixtures,
		fixtureLibrary.profiles,
		before.fixtures[0]?.layer_id ?? "default",
	);
	await requestJson(
		"POST",
		"/api/v2/patch/fixtures",
		{
			request_id: crypto.randomUUID(),
			fixtures: largeScene.fixtures,
			remove_fixture_ids: before.fixtures.map((fixture) => fixture.fixture_id),
			placements: [],
		},
		{ session, showId, revision: before.patch_revision },
	);
	const after = await requestJson("GET", "/api/v2/patch", undefined, {
		session,
		showId,
	});
	const dynamicsPlan = createLargeStageDynamicsPlan(after, largeScene);
	const dynamics = await installLargeStageDynamics(
		session,
		showId,
		dynamicsPlan,
	);
	await setStaticControlIntensity(
		session,
		showId,
		dynamicsPlan.staticControlFixtureIds,
	);
	const scene = {
		...summarizeScene(profile, after.fixtures),
		inventory: largeScene.inventory,
		categoryCounts: largeScene.categoryCounts,
		patch: largeScene.patch,
		dynamics,
		staticControlFixtureCount:
			dynamicsPlan.staticControlFixtureIds.length +
			largeScene.addedMultipatchInstances,
	};
	if (
		scene.fixtureRecords !== LARGE_STAGE_FIXTURE_RECORDS ||
		scene.fixtureInstances !== LARGE_STAGE_FIXTURE_INSTANCES
	)
		throw new Error(
			`Large Stage resolved to ${scene.fixtureRecords} records and ${scene.fixtureInstances} instances`,
		);
	const showSwitch = await createShowSwitchTarget(session, showId, profile);
	await requestJson(
		"POST",
		"/api/v2/shows",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "open",
				show_id: showSwitch.alternateShowId,
				transition: "safe_blackout",
				transition_millis: null,
			},
		},
		{ session },
	);
	await startLargeStageDynamics(
		session,
		showSwitch.alternateShowId,
		dynamics.definitionIds.map((definitionId, index) => ({
			definitionId,
			targets: dynamicsPlan.activations[index].targets,
		})),
	);
	await setStaticControlIntensity(
		session,
		showSwitch.alternateShowId,
		dynamicsPlan.staticControlFixtureIds,
	);
	await requireLargeStageDynamicsRuntime(
		session,
		showSwitch.alternateShowId,
		dynamics,
	);
	await requestJson(
		"POST",
		"/api/v2/shows",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "open",
				show_id: showId,
				transition: "safe_blackout",
				transition_millis: null,
			},
		},
		{ session },
	);
	await requireLargeStageDynamicsRuntime(session, showId, dynamics);
	return {
		scene: {
			...scene,
			addedFixtureRecords: largeScene.addedFixtureRecords,
			addedMultipatchInstances: largeScene.addedMultipatchInstances,
		},
		session,
		showSwitch: { ...showSwitch, dynamics },
	};
}

async function openCanonicalDemoShow(session) {
	const source = path.join(repositoryRoot, "assets", "demo.show");
	const dataBase64 = (await readFile(source)).toString("base64");
	const outcome = await requestJson(
		"POST",
		"/api/v2/shows",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "create",
				name: `Packaged canonical demo ${crypto.randomUUID()}`,
				data_base64: dataBase64,
				overwrite: false,
			},
		},
		{ session },
	);
	const showId = outcome?.result?.show?.id;
	if (!showId)
		throw new Error("Packaged canonical demo import returned no show identity");
	await requestJson(
		"POST",
		"/api/v2/shows",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "open",
				show_id: showId,
				transition: "safe_blackout",
				transition_millis: null,
			},
		},
		{ session },
	);
	return showId;
}

async function startCanonicalDemoBenchmarkLook(session, showId) {
	const activate = async (assignment) => {
		const address =
			assignment.kind === "physical"
				? {
						kind: "playback",
						playback_number: assignment.playbackNumber,
					}
				: {
						kind: "virtual",
						page: 1,
						playback_number: assignment.playbackNumber,
					};
		const pressedAction =
			assignment.kind === "physical"
				? { type: "go_to", cue_number: 1 }
				: { type: "on", pressed: true };
		await requestJson(
			"POST",
			"/api/v2/playback-actions",
			{
				request_id: crypto.randomUUID(),
				address,
				action: pressedAction,
				surface: assignment.kind === "physical" ? "physical" : "virtual",
			},
			{ session, showId, deskId: session.desk.id },
		);
		if (assignment.kind === "physical") return;
		await requestJson(
			"POST",
			"/api/v2/playback-actions",
			{
				request_id: crypto.randomUUID(),
				address,
				action: { type: "on", pressed: false },
				surface: assignment.kind === "physical" ? "physical" : "virtual",
			},
			{ session, showId, deskId: session.desk.id },
		);
	};
	for (const assignment of CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS)
		await activate(assignment);
	const identities = CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.map((assignment) =>
		assignment.kind === "physical"
			? {
					kind: "playback",
					playback_number: assignment.playbackNumber,
				}
			: {
					kind: "virtual",
					page: 1,
					playback_number: assignment.playbackNumber,
				},
	);
	const activationDeadline = Date.now() + 5_000;
	let snapshot;
	let active;
	do {
		snapshot = await requestJson(
			"POST",
			"/api/v2/playback-runtime/snapshot",
			{ identities },
			{ session, showId, deskId: session.desk.id },
		);
		active = snapshot.projections.filter((projection) =>
			projection.target === "cue_list"
				? projection.runtime?.enabled === true
				: projection.target === "dynamic" &&
					projection.runtime?.state === "active",
		);
		if (
			snapshot.projections.length ===
				CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length &&
			active.length === CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length
		)
			break;
		const inactiveNumbers = new Set(
			snapshot.projections
				.filter(
					(projection) =>
						!(projection.target === "cue_list"
							? projection.runtime?.enabled === true
							: projection.target === "dynamic" &&
								projection.runtime?.state === "active"),
				)
				.map((projection) => projection.requested.playback_number),
		);
		for (const assignment of CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS)
			if (inactiveNumbers.has(assignment.playbackNumber))
				await activate(assignment);
		await new Promise((resolve) => setTimeout(resolve, 25));
	} while (Date.now() < activationDeadline);
	if (
		snapshot.projections.length !==
			CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length ||
		active.length !== CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length
	)
		throw new Error(
			`Canonical demo activated ${active.length} of ${CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length} benchmark assignments: ${JSON.stringify(
				snapshot.projections.map((projection) => ({
					requested: projection.requested,
					target: projection.target,
					active:
						projection.target === "cue_list"
							? projection.runtime?.enabled
							: projection.target === "dynamic"
								? projection.runtime?.state
								: null,
				})),
			)}`,
		);
	return {
		assignments: CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.map(
			(assignment) => assignment.name,
		),
		activeAssignments: active.length,
	};
}

async function installLargeStageDynamics(session, showId, plan) {
	const activations = [];
	for (const activation of plan.activations) {
		const outcome = await requestJson(
			"POST",
			"/api/v2/dynamics/create",
			{ request_id: crypto.randomUUID(), definition: activation.definition },
			{ session, showId, deskId: session.desk.id },
		);
		if (!outcome?.object?.id)
			throw new Error("Large Stage Dynamic create returned no object identity");
		activations.push({
			definitionId: outcome.object.id,
			targets: activation.targets,
		});
	}
	await startLargeStageDynamics(session, showId, activations);
	const expected = {
		definitionIds: activations.map((activation) => activation.definitionId),
		instanceCount: LARGE_STAGE_DYNAMIC_INSTANCES,
		targetCount: plan.dynamicTargetCount,
		uniqueTargetCount: new Set(
			activations.flatMap((activation) => activation.targets),
		).size,
		laneCoverage: plan.laneCoverage,
		staticControlFixtureIds: plan.staticControlFixtureIds,
	};
	await requireLargeStageDynamicsRuntime(session, showId, expected);
	return expected;
}

async function startLargeStageDynamics(session, showId, activations) {
	for (const activation of activations)
		await requestJson(
			"POST",
			`/api/v2/dynamics/${encodeURIComponent(activation.definitionId)}/start`,
			{
				targets: activation.targets,
				overrides: {
					size: 1,
					speed_multiplier: { numerator: 1, denominator: 1 },
					phase_offset_degrees: 0,
				},
				timing: {},
				undo_group: "stage-capacity-dynamics",
			},
			{ session, showId, deskId: session.desk.id },
		);
}

async function requireLargeStageDynamicsRuntime(session, showId, expected) {
	const runtime = await requestJson(
		"GET",
		"/api/v2/dynamics/runtime",
		undefined,
		{ session, showId, deskId: session.desk.id },
	);
	if (runtime.instances.length !== expected.instanceCount)
		throw new Error(
			`Large Stage has ${runtime.instances.length} active Dynamic instances; expected ${expected.instanceCount}`,
		);
	const targets = runtime.instances.flatMap((instance) => instance.targets);
	if (
		targets.length !== expected.targetCount ||
		new Set(targets).size !== expected.uniqueTargetCount
	)
		throw new Error(
			`Large Stage Dynamic runtime has ${targets.length} targets (${new Set(targets).size} unique); expected ${expected.targetCount} targets (${expected.uniqueTargetCount} unique)`,
		);
	const unexpectedStatic = expected.staticControlFixtureIds.find((fixtureId) =>
		targets.includes(fixtureId),
	);
	if (unexpectedStatic)
		throw new Error(
			`Large Stage static control ${unexpectedStatic} is targeted by a Dynamic`,
		);
	return runtime;
}

async function setStaticControlIntensity(session, showId, fixtureIds) {
	const userId = session.user.id;
	const [values, capture] = await Promise.all([
		requestJson(
			"GET",
			`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/snapshot`,
			undefined,
			{ session, showId, deskId: session.desk.id },
		),
		requestJson(
			"GET",
			`/api/v2/users/${encodeURIComponent(userId)}/programmer-capture-mode/snapshot`,
			undefined,
			{ session, showId, deskId: session.desk.id },
		),
	]);
	await requestJson(
		"POST",
		`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/actions`,
		{
			request_id: crypto.randomUUID(),
			expected_revision: values.projection.revision,
			expected_capture_mode_revision: capture.projection.revision,
			action: {
				type: "batch",
				mutations: fixtureIds.map((fixtureId) => ({
					type: "set_fixture",
					fixture_id: fixtureId,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.35 },
					timing: {
						fade: false,
						fade_millis: null,
						delay_millis: null,
					},
				})),
			},
		},
		{ session, showId, deskId: session.desk.id },
	);
}

async function createShowSwitchTarget(session, originalShowId, profile) {
	const response = await fetch(
		`http://127.0.0.1:5000/api/v2/shows/${encodeURIComponent(originalShowId)}/download`,
		{
			headers: { authorization: `Bearer ${session.token}` },
		},
	);
	if (!response.ok)
		throw new Error(
			`GET show download returned ${response.status}: ${await response.text()}`,
		);
	const dataBase64 = Buffer.from(await response.arrayBuffer()).toString(
		"base64",
	);
	const outcome = await requestJson(
		"POST",
		"/api/v2/shows",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "create",
				name: `Packaged Stage ${profile} switch target ${crypto.randomUUID()}`,
				data_base64: dataBase64,
				overwrite: false,
			},
		},
		{ session },
	);
	const alternateShowId = outcome?.result?.show?.id;
	if (!alternateShowId)
		throw new Error("Packaged Stage show-switch target has no show identity");
	return { originalShowId, alternateShowId };
}

async function exerciseShowSwitches(session, shows, duration) {
	const result = { attempted: 4, completed: 0, intervals: [], error: null };
	try {
		const startedAt = Date.now();
		const targets = [
			shows.alternateShowId,
			shows.originalShowId,
			shows.alternateShowId,
			shows.originalShowId,
		];
		for (const [index, showId] of targets.entries()) {
			const targetAt = startedAt + duration * 1_000 * (0.2 + index * 0.2);
			await new Promise((resolve) =>
				setTimeout(resolve, Math.max(0, targetAt - Date.now())),
			);
			const interval = {
				showId,
				startedAt: new Date().toISOString(),
				finishedAt: null,
			};
			result.intervals.push(interval);
			try {
				await requestJson(
					"POST",
					"/api/v2/shows",
					{
						request_id: crypto.randomUUID(),
						action: {
							type: "open",
							show_id: showId,
							transition: "safe_blackout",
							transition_millis: null,
						},
					},
					{ session },
				);
				if (shows.dynamics)
					await requireLargeStageDynamicsRuntime(
						session,
						showId,
						shows.dynamics,
					);
				if (shows.canonicalDemo)
					await startCanonicalDemoBenchmarkLook(session, showId);
			} finally {
				interval.finishedAt = new Date().toISOString();
			}
			result.completed++;
		}
	} catch (error) {
		result.error = error instanceof Error ? error.message : String(error);
	}
	return result;
}

async function exerciseApplicationSuspend(application, launched, duration) {
	const result = {
		attempted: process.platform !== "win32",
		completed: false,
		durationMillis: 1_000,
		startedAt: null,
		finishedAt: null,
		error: null,
	};
	if (!result.attempted) {
		result.error =
			"Process suspend/resume is not available through Node signals on Windows";
		return result;
	}
	let pid;
	let suspended = false;
	try {
		await new Promise((resolve) =>
			setTimeout(resolve, Math.min(8_000, Math.max(3_500, duration * 350))),
		);
		pid = await lightDesktopPid(application.direct ? launched.pid : undefined);
		if (!pid)
			throw new Error("Packaged ToskLight process identity is unavailable");
		result.startedAt = new Date().toISOString();
		process.kill(pid, "SIGSTOP");
		suspended = true;
		await new Promise((resolve) => setTimeout(resolve, result.durationMillis));
		process.kill(pid, "SIGCONT");
		suspended = false;
		result.finishedAt = new Date().toISOString();
		result.completed = true;
	} catch (error) {
		result.error = error instanceof Error ? error.message : String(error);
	} finally {
		if (suspended && pid) {
			try {
				process.kill(pid, "SIGCONT");
			} catch {}
		}
	}
	return result;
}

function runtimeDiagnostics(session) {
	return requestJson("GET", "/api/v2/diagnostics", undefined, { session });
}

async function exercisePackagedProgrammerTiming(session) {
	const commandLine = await requestJson(
		"GET",
		"/api/v2/command-line",
		undefined,
		{ session, deskId: session.desk.id },
	);
	await requestJson(
		"PUT",
		"/api/v2/command-line",
		{ text: "FIXTURE 999" },
		{
			session,
			deskId: session.desk.id,
			revision: commandLine.revision,
		},
	);
	await requestJson(
		"POST",
		"/api/v2/command-line/execute",
		{ request_id: crypto.randomUUID(), command: "FIXTURE 1 AT 1" },
		{ session, deskId: session.desk.id },
	);
	return {
		actions: ["command_line_edit", "command_execute"],
		sources: ["http"],
	};
}

async function requestJson(method, route, body, context = {}) {
	const headers = {};
	if (body !== undefined) headers["content-type"] = "application/json";
	if (context.session)
		headers.authorization = `Bearer ${context.session.token}`;
	if (context.showId) headers["x-tosk-show"] = context.showId;
	if (context.revision !== undefined)
		headers["if-match"] = String(context.revision);
	if (context.deskId) headers["x-tosk-desk"] = context.deskId;
	const response = await fetch(`http://127.0.0.1:5000${route}`, {
		method,
		headers,
		body: body === undefined ? undefined : JSON.stringify(body),
	});
	if (!response.ok)
		throw new Error(
			`${method} ${route} returned ${response.status}: ${await response.text()}`,
		);
	if (response.status === 204) return undefined;
	return response.json();
}

function summarizeScene(profile, fixtures) {
	return {
		profile,
		fixtureRecords: fixtures.length,
		fixtureInstances: countFixtureInstances(fixtures),
	};
}

async function stopExistingDevelopmentDesk() {
	if (process.platform === "darwin")
		await runAllowingNoMatch("launchctl", [
			"remove",
			"de.tokenet.tosklight.dev-server",
		]);
	if (process.platform === "win32") {
		for (const processName of ["light-desktop.exe", "light-headless.exe"])
			await runAllowingAnyExit("taskkill", ["/F", "/IM", processName]);
		return;
	}
	for (const processName of ["light-desktop", "light-headless", "ToskLight"])
		await runAllowingNoMatch("pkill", ["-x", processName]);
}

async function stopPackagedDesktop() {
	if (process.platform === "win32") {
		for (const processName of ["light-desktop.exe", "light-headless.exe"])
			await runAllowingAnyExit("taskkill", ["/F", "/IM", processName]);
		return;
	}
	for (const processName of ["light-desktop", "light-headless", "ToskLight"])
		await runAllowingNoMatch("pkill", ["-x", processName]);
}

async function lightDesktopResidentBytes(directPid) {
	const pid = await lightDesktopPid(directPid);
	if (!pid) return null;
	if (process.platform === "win32") {
		const memory = await execFileAsync("powershell", [
			"-NoProfile",
			"-Command",
			`(Get-Process -Id ${pid}).WorkingSet64`,
		]).catch(() => ({ stdout: "" }));
		const residentBytes = Number.parseInt(memory.stdout.trim(), 10);
		return Number.isFinite(residentBytes) ? residentBytes : null;
	}
	const memory = await execFileAsync("ps", ["-o", "rss=", "-p", pid]).catch(
		() => ({ stdout: "" }),
	);
	const residentKiB = Number.parseInt(memory.stdout.trim(), 10);
	return Number.isFinite(residentKiB) ? residentKiB * 1_024 : null;
}

async function lightDesktopPid(directPid) {
	let pid = directPid ? String(directPid) : null;
	if (!pid && process.platform !== "win32") {
		const { stdout } = await execFileAsync("pgrep", [
			"-x",
			"light-desktop",
		]).catch(() => ({ stdout: "" }));
		pid = stdout.trim().split(/\s+/).filter(Boolean).at(-1) ?? null;
	}
	return pid ? Number.parseInt(pid, 10) : null;
}

function packagedApplication() {
	if (process.platform === "darwin") {
		const bundle = path.join(
			artifactPaths.cargo,
			"debug/bundle/macos/ToskLight.app",
		);
		return {
			direct: false,
			executable: path.join(bundle, "Contents/MacOS/light-desktop"),
			bundle,
		};
	}
	return {
		direct: true,
		executable: path.join(
			artifactPaths.cargo,
			"debug",
			process.platform === "win32" ? "light-desktop.exe" : "light-desktop",
		),
	};
}

function launchPackagedApplication(application, environment) {
	if (!application.direct)
		return spawn(
			"open",
			[
				"-W",
				"-F",
				...Object.entries(environment).flatMap(([name, value]) => [
					"--env",
					`${name}=${value}`,
				]),
				application.bundle,
			],
			{
				cwd: repositoryRoot,
				stdio: "ignore",
			},
		);
	return spawn(application.executable, [], {
		cwd: repositoryRoot,
		env: { ...process.env, ...environment },
		stdio: "ignore",
	});
}

async function activatePackagedApplication(application) {
	if (process.platform !== "darwin" || application.direct) return;
	await execFileAsync("open", [application.bundle]);
	await execFileAsync("osascript", [
		"-e",
		'tell application "System Events" to tell process "ToskLight" to set frontmost to true',
	]);
	await new Promise((resolve) => setTimeout(resolve, 250));
}

async function runAllowingNoMatch(command, arguments_) {
	await new Promise((resolve, reject) => {
		const child = spawn(command, arguments_, { stdio: "ignore" });
		child.once("error", reject);
		child.once("exit", (code) => {
			if (code === 0 || code === 1 || code === 3) resolve();
			else reject(new Error(`${command} exited with ${code}`));
		});
	});
}

async function runAllowingAnyExit(command, arguments_) {
	await new Promise((resolve) => {
		const child = spawn(command, arguments_, { stdio: "ignore" });
		child.once("error", () => resolve());
		child.once("exit", () => resolve());
	});
}

async function waitForComplete(file, timeoutMs) {
	return waitForRecord(
		file,
		(record) => record.kind === "complete" || record.kind === "error",
		timeoutMs,
	);
}

async function waitForRecord(file, predicate, timeoutMs) {
	const started = Date.now();
	while (Date.now() - started < timeoutMs) {
		const records = await readRecords(file);
		if (records.some(predicate)) return records;
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(`Timed out waiting for packaged Stage report ${file}`);
}

async function readRecords(file) {
	const text = await readFile(file, "utf8").catch(() => "");
	return text
		.split("\n")
		.filter(Boolean)
		.flatMap((line) => {
			try {
				return [JSON.parse(line)];
			} catch {
				return [];
			}
		});
}

function evaluate(
	complete,
	duration,
	samplesFile,
	profile,
	scene,
	runtime,
	processMemory,
) {
	const stage = complete.frontend?.stage;
	const timeline = Array.isArray(complete.timeline) ? complete.timeline : [];
	const frames = Array.isArray(stage?.frames)
		? stage.frames.map((frame) => ({ ...frame, visibilitySegment: 0 }))
		: uniqueTimelineFrames(timeline);
	const lifecycleSettled = frames.filter(
		(frame) =>
			Number.isFinite(frame.sourceToSettledCanvasMs) &&
			Number.isFinite(frame.settledCanvasSubmittedAt) &&
			!frameOverlapsApplicationSuspend(frame, runtime.applicationSuspend) &&
			!frameOverlapsShowSwitch(frame, runtime.showSwitch) &&
			!frameOverlapsContextRecovery(frame, complete.contextRecovery),
	);
	const firstReadySample = timeline.find((sample) => sample.renders > 0);
	const steadyStateStartedAt = Number.isFinite(firstReadySample?.recordedAt)
		? firstReadySample.recordedAt + 5_000
		: null;
	const additionalStageOpenedAt =
		timeline.find((sample) => sample.additionalStageWindow === "opened")
			?.recordedAt ?? null;
	const operationalSettled =
		steadyStateStartedAt === null
			? lifecycleSettled
			: lifecycleSettled.filter(
					(frame) =>
						Date.parse(frame.sourceGeneratedAt) >= steadyStateStartedAt,
				);
	const settled = Number.isFinite(additionalStageOpenedAt)
		? operationalSettled.filter(
				(frame) =>
					Date.parse(frame.sourceGeneratedAt) < additionalStageOpenedAt,
			)
		: operationalSettled;
	const startupSettled =
		steadyStateStartedAt === null
			? []
			: lifecycleSettled.filter(
					(frame) => Date.parse(frame.sourceGeneratedAt) < steadyStateStartedAt,
				);
	const latencies = settled
		.map((frame) => frame.sourceToSettledCanvasMs)
		.sort((left, right) => left - right);
	const operationalLatencies = operationalSettled
		.map((frame) => frame.sourceToSettledCanvasMs)
		.sort((left, right) => left - right);
	const lanes = [
		...new Set(operationalSettled.map((frame) => frame.lane)),
	].sort();
	const qualities = [
		...new Set(timeline.map((sample) => sample.quality).filter(Boolean)),
	];
	const qualityObjects = summarizeQualityObjects(timeline);
	const renderSummary = summarizeRenders(timeline);
	const focusedSamples = timeline.filter(
		(sample) => sample.mainDocumentFocused,
	);
	const presentationGaps = changingPresentationGaps(
		frames,
		runtime.applicationSuspend,
		runtime.showSwitch,
		complete.contextRecovery,
	);
	const sourceCadenceGaps = laneSourceCadenceGaps(
		frames,
		runtime.applicationSuspend,
		runtime.showSwitch,
		complete.contextRecovery,
	);
	const realTimeStageGateEnforced = profile !== "large-stage";
	const failures = [];
	if (realTimeStageGateEnforced && !settled.length)
		failures.push("no changing frame reached a packaged canvas");
	if (!(stage?.rafCallbacks > 0))
		failures.push(
			"the packaged WebView submitted no RAF callback; keep the operator session unlocked and ToskLight visible",
		);
	if (realTimeStageGateEnforced && !lanes.includes("normal"))
		failures.push("Live lane produced no settled canvas sample");
	if (realTimeStageGateEnforced && !lanes.includes("preload"))
		failures.push("Preload lane produced no settled canvas sample");
	if (realTimeStageGateEnforced && percentile(latencies, 95) > 120)
		failures.push("packaged engine-frame-to-canvas p95 exceeded 120 ms");
	if (realTimeStageGateEnforced && Math.max(0, ...operationalLatencies) > 200)
		failures.push("a changing frame exceeded the 200 ms hard latency ceiling");
	if (realTimeStageGateEnforced && Math.max(0, ...presentationGaps) > 200)
		failures.push("a changing lane had a presentation gap longer than 200 ms");
	if (realTimeStageGateEnforced && Math.max(0, ...sourceCadenceGaps) > 200)
		failures.push("a claimed lane had a source cadence gap longer than 200 ms");
	if (
		realTimeStageGateEnforced &&
		latestChangingFrameDidNotSettle(frames, complete.recordedAt)
	)
		failures.push(
			"the final unsuperseded changing frame did not reach a packaged canvas",
		);
	if (qualities.length !== 4)
		failures.push(
			"the packaged run did not exercise all four render qualities",
		);
	if (!timeline.some((sample) => sample.additionalStageWindow === "opened"))
		failures.push("the representative additional Stage window did not open");
	if (
		profile === "large-stage" &&
		!["stage-3d", "fixture-sheet"].every((surface) =>
			complete.activeUiSurfaces?.includes(surface),
		)
	)
		failures.push(
			"the interactive large tier did not expose both Stage 3D and Fixture Sheet surfaces",
		);
	assertPackagedQualityObjects(qualityObjects, failures);
	const outstandingContexts =
		(stage?.rendererContextsCreated ?? 0) -
		(stage?.rendererContextsDisposed ?? 0);
	if (outstandingContexts > 2)
		failures.push(
			"renderer context ownership grew beyond the two visible surfaces",
		);
	if ((stage?.rendererContextLosses ?? 0) < 1)
		failures.push("the Stage benchmark did not observe a WebGL context loss");
	if ((stage?.rendererContextRestores ?? 0) < 1)
		failures.push(
			"the Stage benchmark did not observe WebGL context restoration",
		);
	if (complete.contextRecoveryMethod !== "webgl_lose_context")
		failures.push(
			"the packaged context gate did not use the WEBGL_lose_context extension",
		);
	if ((stage?.desktopMirrorRenders ?? 0) < 1)
		failures.push(
			"the sibling Stage window did not acknowledge a mirrored canvas render",
		);
	if ((runtime.showSwitch?.completed ?? 0) !== 4)
		failures.push(
			"the packaged Stage run did not complete two active-show round trips",
		);
	if (
		process.platform !== "win32" &&
		runtime.applicationSuspend?.completed !== true
	)
		failures.push(
			"the packaged Stage run did not complete its application suspend/resume cycle",
		);
	const output = summarizeOutputComparison(runtime);
	const achievedOutputHz = output.stage.frames_sent / duration;
	if (!output.boundedWindowGatePassed)
		failures.push(
			"packaged Stage output p99 regressed by more than 1 ms or 5 percent",
		);
	if (output.stageWindowDeadlineMisses > 0)
		failures.push("packaged Stage output window missed a scheduler deadline");
	if (output.stageWindowSendErrors > 0)
		failures.push("packaged Stage output window recorded a send error");
	const visualization = summarizeVisualizationWindow(runtime);
	const programmerActionTiming =
		summarizePackagedProgrammerActionTiming(runtime);
	for (const failure of programmerActionTiming.failures)
		failures.push(`programmer action timing: ${failure}`);
	const maximumSharedProjectionCount = duration * 25;
	if (
		(runtime.after.visualization?.projections ?? 0) >
		maximumSharedProjectionCount
	)
		failures.push(
			"opening the sibling Stage window multiplied server visualization projection work",
		);
	if ((runtime.after.visualization?.preload_subscribers ?? 0) > 1)
		failures.push(
			"opening the sibling Stage window created more than one preload visualization subscriber",
		);
	if (visualization.finalStreamQueueDepth !== 0)
		failures.push("packaged visualization stream retained a queued frame");
	if (
		profile === "large-stage" &&
		visualization.streamQueueDrops + visualization.streamSendFailures === 0
	)
		failures.push(
			"packaged paused visualization client caused no bounded queue replacement or send failure",
		);
	if (profile === "large-stage") {
		if (
			scene.fixtureRecords !== LARGE_STAGE_FIXTURE_RECORDS ||
			scene.fixtureInstances !== LARGE_STAGE_FIXTURE_INSTANCES
		)
			failures.push(
				"packaged large Stage did not retain the exact 970-record and 1,000-instance scene",
			);
		if (
			scene.categoryCounts?.sunstrip !== 40 ||
			scene.categoryCounts?.moving !== 500
		)
			failures.push(
				"packaged large Stage did not retain the required Sunstrip and moving-light inventory",
			);
		if (
			scene.patch?.universeCount !== 37 ||
			scene.patch?.occupiedSlots !== 18_840
		)
			failures.push(
				"packaged large Stage did not retain the required multi-universe DMX occupancy",
			);
		if (scene.dynamics?.instanceCount !== LARGE_STAGE_DYNAMIC_INSTANCES)
			failures.push(
				"packaged large Stage did not prepare exactly 20 Dynamic instances",
			);
		if (scene.staticControlFixtureCount !== 440)
			failures.push(
				"packaged large Stage did not retain 440 fixed-dimmer control instances",
			);
	}
	for (const failure of packagedStageSceneFailures(profile, scene))
		failures.push(failure);
	if (
		profile === "canonical-demo" &&
		scene.benchmarkLook?.activeAssignments !==
			CANONICAL_DEMO_BENCHMARK_ASSIGNMENTS.length
	)
		failures.push(
			"packaged canonical demo did not activate all 12 benchmark assignments",
		);
	const longRun = summarizeStageLongRunResources(
		timeline,
		processMemory,
		duration,
	);
	for (const failure of longRun.failures) failures.push(failure);
	return {
		schemaVersion: 2,
		tier: profileDefinition.tier,
		measurementSurface: "packaged-tauri-webview",
		profile,
		profileLabel: profileDefinition.label,
		scene,
		host: hostHardware(),
		durationSeconds: duration,
		activeUiSurfaces: complete.activeUiSurfaces ?? [],
		mainWindowFocus: {
			samples: timeline.length,
			focusedSamples: focusedSamples.length,
			unfocusedSamples: timeline.length - focusedSamples.length,
		},
		visualizationEnabled: true,
		rate: {
			targetHz: profileDefinition.targetHz,
			achievedOutputHz,
			blocking: profileDefinition.blocking,
		},
		samplesFile,
		acceptanceGateEnforced: failures.length === 0,
		failures,
		qualities,
		qualityObjects,
		lanes,
		latency: {
			realTimeGateEnforced: realTimeStageGateEnforced,
			samples: latencies.length,
			p50Ms: percentile(latencies, 50),
			p95Ms: percentile(latencies, 95),
			maxMs: latencies.at(-1) ?? null,
			allSurfaceSamples: operationalLatencies.length,
			allSurfaceMaxMs: operationalLatencies.at(-1) ?? null,
			steadyStateStartedAt:
				steadyStateStartedAt === null
					? null
					: new Date(steadyStateStartedAt).toISOString(),
			additionalStageOpenedAt:
				additionalStageOpenedAt === null
					? null
					: new Date(additionalStageOpenedAt).toISOString(),
			startupSamples: startupSettled.length,
			startupMaxMs: startupSettled.length
				? Math.max(
						...startupSettled.map((frame) => frame.sourceToSettledCanvasMs),
					)
				: null,
			maxPresentationGapMs: presentationGaps.length
				? Math.max(...presentationGaps)
				: null,
			maxSourceCadenceGapMs: sourceCadenceGaps.length
				? Math.max(...sourceCadenceGaps)
				: null,
		},
		resources: {
			initialBrowserMemoryBytes: complete.initialBrowserMemoryBytes ?? null,
			finalBrowserMemoryBytes: complete.browserMemoryBytes ?? null,
			browserMemoryGrowthBytes:
				Number.isFinite(complete.initialBrowserMemoryBytes) &&
				Number.isFinite(complete.browserMemoryBytes)
					? complete.browserMemoryBytes - complete.initialBrowserMemoryBytes
					: null,
			sceneBuilds: collectionSize(stage?.sceneBuilds),
			renders: collectionSize(stage?.renders),
			rafCallbacks: stage?.rafCallbacks ?? 0,
			rendererContextsCreated: stage?.rendererContextsCreated ?? 0,
			rendererContextsDisposed: stage?.rendererContextsDisposed ?? 0,
			rendererContextLosses: stage?.rendererContextLosses ?? 0,
			rendererContextRestores: stage?.rendererContextRestores ?? 0,
			desktopMirrorRenders: stage?.desktopMirrorRenders ?? 0,
			modelCacheHits: stage?.modelCacheHits ?? 0,
			modelCacheMisses: stage?.modelCacheMisses ?? 0,
			modelCacheDisposals: stage?.modelCacheDisposals ?? 0,
			...renderSummary,
			processMemory: {
				measurement: `${process.platform} light-desktop main-process resident set`,
				samples: processMemory,
			},
			longRun,
		},
		server: {
			...runtime,
			outputComparison: output,
			visualizationWindow: visualization,
			programmerActionTiming,
		},
		capabilities: complete.capabilities ?? null,
	};
}

function evaluateImprovedBeamSpike(
	complete,
	samplesFile,
	scene,
	runtime,
	processMemory,
) {
	const spike = complete.spike;
	const failures = [];
	const programmerActionTiming =
		summarizePackagedProgrammerActionTiming(runtime);
	for (const failure of programmerActionTiming.failures)
		failures.push(`programmer action timing: ${failure}`);
	if (!spike)
		failures.push("packaged Improved-beam spike returned no measurement");
	if (!(spike?.performance?.frames > 0))
		failures.push("packaged Improved-beam spike rendered no measured frames");
	if (spike?.termination?.firstHitDistance === null)
		failures.push("packaged Improved-beam spike recorded no occluder distance");
	if (!Number.isFinite(spike?.visual?.litReceiverLuminance))
		failures.push("packaged Improved-beam spike recorded no pixel evidence");
	return {
		schemaVersion: 1,
		measurementSurface: "packaged-tauri-webview",
		profile: "improved-beam-spike",
		scene,
		host: hostHardware(),
		samplesFile,
		acceptanceGateEnforced: failures.length === 0,
		failures,
		capabilityDecision: spike?.extensionAccepted ? "accepted" : "rejected",
		spike,
		server: {
			...runtime,
			programmerActionTiming,
		},
		processMemory: {
			measurement: `${process.platform} light-desktop main-process resident set`,
			samples: processMemory,
		},
	};
}

function summarizePackagedProgrammerActionTiming(runtime) {
	const before = runtime?.before?.programmer_action_timing;
	const after = runtime?.after?.programmer_action_timing;
	return summarizeProgrammerActionTiming(
		after,
		latestProgrammerActionId(before),
		{
			minimumSamples: 2,
			sources: ["http"],
			actions: ["command_line_edit", "command_execute"],
			frameRateBands: ["at-or-below-60"],
		},
	);
}

function summarizeVisualizationWindow(runtime) {
	return {
		projections:
			runtime.after.visualization.projections -
			runtime.afterNoStage.visualization.projections,
		skippedSourceFrames:
			runtime.after.visualization.skipped_source_frames -
			runtime.afterNoStage.visualization.skipped_source_frames,
		latestProjectionMicros: runtime.after.visualization.projection_micros,
		latestPayloadBytes: runtime.after.visualization.payload_bytes,
		latestSourceAgeMillis: runtime.after.visualization.source_age_millis,
		streamSendFailures:
			runtime.after.visualization.stream_send_failures -
			runtime.afterNoStage.visualization.stream_send_failures,
		streamQueueDrops:
			runtime.after.visualization.stream_queue_drops -
			runtime.afterNoStage.visualization.stream_queue_drops,
		finalStreamQueueDepth: runtime.after.visualization.stream_queue_depth,
	};
}

function hostHardware() {
	const cpus = os.cpus();
	return {
		platform: process.platform,
		architecture: os.machine(),
		osRelease: os.release(),
		logicalCpuCount: cpus.length,
		cpuModels: [
			...new Set(cpus.map((cpu) => cpu.model.trim()).filter(Boolean)),
		],
		totalMemoryBytes: os.totalmem(),
	};
}

function summarizeOutputComparison(runtime) {
	const noStage = outputWindow(
		runtime.before.output,
		runtime.afterNoStage.output,
	);
	const stage = outputWindow(runtime.afterNoStage.output, runtime.after.output);
	const noStageP99TickMicros = histogramPercentileMicros(noStage, 99);
	const stageP99TickMicros = histogramPercentileMicros(stage, 99);
	const allowedP99RegressionMicros =
		noStageP99TickMicros === null
			? null
			: Math.max(1_000, noStageP99TickMicros * 0.05);
	const p99RegressionMicros =
		noStageP99TickMicros === null || stageP99TickMicros === null
			? null
			: stageP99TickMicros - noStageP99TickMicros;
	return {
		noStage,
		stage,
		noStageP99TickMicros,
		stageP99TickMicros,
		allowedP99RegressionMicros,
		p99RegressionMicros,
		boundedWindowGatePassed:
			p99RegressionMicros !== null &&
			allowedP99RegressionMicros !== null &&
			p99RegressionMicros <= allowedP99RegressionMicros,
		stageWindowDeadlineMisses: stage.deadline_misses,
		stageWindowSendErrors: stage.send_errors,
		releaseGateEnforced: true,
	};
}

function summarizeRenders(timeline) {
	const renders = timelineRenders(timeline);
	const cpuDurations = renders
		.map((render) => render.durationMs)
		.filter(Number.isFinite)
		.sort((left, right) => left - right);
	const maximum = (field) =>
		Math.max(0, ...renders.map((render) => Number(render[field] ?? 0)));
	return {
		maxDrawCalls: maximum("calls"),
		maxTransparentDrawCalls: maximum("transparentDrawCalls"),
		maxTriangles: maximum("triangles"),
		maxGeometries: maximum("geometries"),
		maxTextures: maximum("textures"),
		cpuFrameP95Ms: percentile(cpuDurations, 95),
		cpuFrameMaxMs: cpuDurations.at(-1) ?? null,
		gpuFrameP95Ms: null,
		gpuFrameMeasurement:
			"Unavailable in the current Three.js/WebKit diagnostics path",
	};
}

function collectionSize(value) {
	return Array.isArray(value) ? value.length : Number(value ?? 0);
}

function summarizeQualityObjects(timeline) {
	const summaries = {};
	for (const render of timelineRenders(timeline)) {
		if (!render?.visibleObjects) continue;
		const summary = summaries[render.renderQuality] ?? {};
		summaries[render.renderQuality] = summary;
		for (const [name, value] of Object.entries(render.visibleObjects))
			summary[name] = Math.max(summary[name] ?? 0, value);
	}
	return summaries;
}

function timelineRenders(timeline) {
	const exact = timeline.flatMap((sample) =>
		Array.isArray(sample.newRenders) ? sample.newRenders : [],
	);
	if (exact.length > 0) return exact;
	const deduplicated = new Map();
	for (const sample of timeline) {
		const render = sample.latestRender;
		if (!render) continue;
		const identity =
			render.benchmarkSequence ??
			`${render.paneId}:${render.submittedAt}:${render.renderQuality}`;
		deduplicated.set(identity, render);
	}
	return [...deduplicated.values()];
}

function assertPackagedQualityObjects(qualityObjects, failures) {
	const linesOnly = qualityObjects.lines_only;
	const linesAndBeams = qualityObjects.lines_and_beams;
	const beams = qualityObjects.beams;
	const improved = qualityObjects.improved_beams;
	if (
		!linesOnly ||
		linesOnly.beamVolumes !== 0 ||
		linesOnly.improvedBeamVolumes !== 0 ||
		linesOnly.improvedBeamLights !== 0 ||
		!(linesOnly.centerLines > 0) ||
		!(linesOnly.groundFootprints > 0)
	)
		failures.push(
			"Lines only did not render its exact line/footprint object set",
		);
	if (
		!linesAndBeams ||
		!(linesAndBeams.beamVolumes > 0) ||
		linesAndBeams.improvedBeamVolumes !== 0 ||
		linesAndBeams.improvedBeamLights !== 0 ||
		!(linesAndBeams.centerLines > 0) ||
		!(linesAndBeams.groundFootprints > 0)
	)
		failures.push(
			"Lines + beams did not render its exact volume/line/footprint object set",
		);
	if (
		!beams ||
		!(beams.beamVolumes > 0) ||
		beams.improvedBeamVolumes !== 0 ||
		beams.improvedBeamLights !== 0 ||
		beams.centerLines !== 0 ||
		beams.groundFootprints !== 0
	)
		failures.push("Beams did not render its exact volume-only object set");
	if (
		!improved ||
		improved.beamVolumes !== 0 ||
		!(improved.improvedBeamVolumes > 0) ||
		!(improved.improvedBeamLights > 0) ||
		improved.improvedBeamLights > 8 ||
		improved.centerLines !== 0 ||
		improved.groundFootprints !== 0
	)
		failures.push(
			"Improved beams did not render its exact feathered-volume object set",
		);
}

function uniqueTimelineFrames(timeline) {
	const frames = new Map();
	let normalVisibilitySegment = 0;
	let previousLiveCanvasVisible = true;
	for (const sample of timeline) {
		const liveCanvasVisible = sample.liveVisible && sample.liveView === "3d";
		if (liveCanvasVisible && !previousLiveCanvasVisible)
			normalVisibilitySegment++;
		previousLiveCanvasVisible = liveCanvasVisible;
		for (const lane of ["normal", "preload"]) {
			if (lane === "normal" && !liveCanvasVisible) continue;
			const frame = sample.latestFrames?.[lane];
			if (!frame) continue;
			const key = `${lane}:${frame.showId ?? "unknown"}:${frame.scopeActivation ?? 0}:${frame.sourceFrame ?? frame.sourceGeneratedAt}`;
			const candidate = {
				...frame,
				visibilitySegment: lane === "normal" ? normalVisibilitySegment : 0,
			};
			const previous = frames.get(key);
			if (
				!previous ||
				(!Number.isFinite(previous.sourceToSettledCanvasMs) &&
					Number.isFinite(candidate.sourceToSettledCanvasMs))
			)
				frames.set(key, candidate);
		}
	}
	return [...frames.values()];
}

function percentile(sorted, value) {
	if (!sorted.length) return null;
	return sorted[Math.max(0, Math.ceil((value / 100) * sorted.length) - 1)];
}

function positiveInteger(value, label) {
	const parsed = Number.parseInt(value, 10);
	if (!Number.isInteger(parsed) || parsed <= 0)
		throw new Error(`${label} must be a positive integer`);
	return parsed;
}
