import { execFile, spawn } from "node:child_process";
import dgram from "node:dgram";
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
	LIGHT_STAGE_PACKAGED_BENCH_ADDITIONAL_STAGE_WINDOW:
		profile === "supported-scale" ? "0" : "1",
	LIGHT_STAGE_PACKAGED_BENCH_PREPARED: preparedPath,
	LIGHT_DESKTOP_TEST_DATA_DIR: dataPath,
};
const app = launchPackagedApplication(application, benchmarkEnvironment);

let records;
let scene;
let runtime;
let benchmarkFailure;
let benchmarkPhase = "startup";
let slowClient;
let networkCapture;
let oscHardware;
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
	benchmarkPhase = "scene-preparation";
	const prepared = await prepareScene(profile);
	scene = prepared.scene;
	if (profile === "supported-scale") {
		benchmarkPhase = "scheduler-configuration";
		await configureSupportedScaleScheduler(prepared.session);
		networkCapture = await startSupportedScaleNetworkCapture(
			prepared.scene.playbackWorkload.dmxAddress,
		);
		await configureSupportedScaleOutputRoutes(
			prepared.session,
			prepared.showId,
			networkCapture,
			prepared.scene.playbackWorkload.logicalUniverse,
		);
		oscHardware = await startPackagedOscHardware(
			prepared.session.desk.osc_alias,
		);
	}
	benchmarkPhase = "control-window";
	const before = await runtimeDiagnostics(prepared.session);
	const networkBefore = networkCapture?.snapshot() ?? null;
	const programmerTimingExercise = await exercisePackagedProgrammerTiming(
		prepared.session,
	);
	const controlPlaybackTimingExercise =
		profile === "supported-scale"
			? await exerciseSupportedScaleOscPlayback(
					prepared.session,
					prepared.showId,
					oscHardware,
					networkCapture,
				)
			: null;
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
			controlPlaybackTimingExercise,
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
		benchmarkPhase = "stage-window";
		const afterNoStage = await runtimeDiagnostics(prepared.session);
		const networkAfterNoStage = networkCapture?.snapshot() ?? null;
		const stagePlaybackTimingExercise =
			profile === "supported-scale"
				? await exerciseSupportedScaleOscPlayback(
						prepared.session,
						prepared.showId,
						oscHardware,
						networkCapture,
					)
				: null;
		memoryPhase = "stage";
		if (isLargeOperatorProfile(profile))
			slowClient = await startSlowVisualizationClient(
				"http://127.0.0.1:5000",
				prepared.session.token,
			);
		const showSwitch =
			profile === "supported-scale"
				? Promise.resolve({ attempted: false, completed: 0, intervals: [] })
				: exerciseShowSwitches(
						prepared.session,
						prepared.showSwitch,
						durationSeconds,
					);
		const applicationSuspend =
			profile === "supported-scale"
				? Promise.resolve({ attempted: false, completed: false })
				: exerciseApplicationSuspend(application, app, durationSeconds);
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
		const networkAfter = networkCapture?.snapshot() ?? null;
		runtime = {
			before,
			afterNoStage,
			after,
			showSwitch: showSwitchResult,
			applicationSuspend: applicationSuspendResult,
			programmerTimingExercise,
			controlPlaybackTimingExercise,
			stagePlaybackTimingExercise,
			networkCapture: {
				before: networkBefore,
				afterNoStage: networkAfterNoStage,
				after: networkAfter,
			},
		};
	}
} catch (reason) {
	benchmarkFailure = reason;
} finally {
	clearInterval(memorySampler);
	await collectMemory();
	await slowClient?.close();
	await networkCapture?.close();
	await oscHardware?.close();
	app.kill("SIGTERM");
	await stopPackagedDesktop();
}

let result;
if (benchmarkFailure) {
	result = benchmarkFailureReport(
		benchmarkFailure,
		benchmarkPhase,
		samplesPath,
		profile,
		scene,
		processMemory,
	);
} else {
	const terminalError = records.findLast((record) => record.kind === "error");
	if (terminalError)
		throw new Error(
			`Packaged Stage shell failed: ${terminalError.message ?? "unknown error"}`,
		);
	const complete = records.findLast((record) => record.kind === "complete");
	if (!complete)
		throw new Error("Packaged Stage report has no completion record");
	result =
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
}
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

async function configureSupportedScaleScheduler(session) {
	await requestJson(
		"POST",
		"/api/v2/configuration/update",
		{
			request_id: crypto.randomUUID(),
			patch: { frame_rate_hz: 60 },
		},
		{ session },
	);
	const snapshot = await requestJson(
		"GET",
		"/api/v2/configuration",
		undefined,
		{
			session,
		},
	);
	const configuredHz =
		snapshot.configuration?.frame_rate_hz ?? snapshot.frame_rate_hz ?? null;
	if (configuredHz !== 60)
		throw new Error(
			`production output scheduler retained ${configuredHz ?? "no"} Hz after requesting 60 Hz`,
		);
}

async function startSupportedScaleNetworkCapture(trackedAddress) {
	if (!Number.isSafeInteger(trackedAddress) || trackedAddress < 1)
		throw new Error(
			"supported-scale network capture has no tracked DMX address",
		);
	const openReceiver = async (protocol) => {
		const socket = dgram.createSocket("udp4");
		const state = {
			protocol,
			packets: 0,
			bytes: 0,
			firstReceivedAt: null,
			lastReceivedAt: null,
			lastPacketPrefixHex: null,
			lastDmxHex: null,
			dmxChanges: 0,
			lastDmxChangedAt: null,
			trackedAddress,
			lastTrackedValue: null,
			trackedValueChanges: 0,
			trackedValueHistory: [],
		};
		socket.on("message", (packet) => {
			const receivedAt = new Date().toISOString();
			state.packets++;
			state.bytes += packet.length;
			state.firstReceivedAt ??= receivedAt;
			state.lastReceivedAt = receivedAt;
			state.lastPacketPrefixHex = packet.subarray(0, 24).toString("hex");
			const dmx = decodeNetworkDmxPayload(protocol, packet);
			if (!dmx) return;
			const dmxHex = dmx.toString("hex");
			if (state.lastDmxHex !== null && state.lastDmxHex !== dmxHex) {
				state.dmxChanges++;
				state.lastDmxChangedAt = receivedAt;
			}
			state.lastDmxHex = dmxHex;
			const trackedValue = dmx[trackedAddress - 1];
			if (
				trackedValue !== undefined &&
				state.lastTrackedValue !== null &&
				state.lastTrackedValue !== trackedValue
			) {
				state.trackedValueChanges++;
				state.trackedValueHistory.push({
					changedAt: receivedAt,
					value: trackedValue,
				});
				if (state.trackedValueHistory.length > 2_048)
					state.trackedValueHistory.shift();
			}
			if (trackedValue !== undefined) state.lastTrackedValue = trackedValue;
		});
		await new Promise((resolve, reject) => {
			socket.once("error", reject);
			socket.bind(0, "127.0.0.1", () => {
				socket.off("error", reject);
				resolve();
			});
		});
		return {
			port: socket.address().port,
			snapshot: () => ({
				...state,
				trackedValueHistory: [...state.trackedValueHistory],
			}),
			close: () =>
				new Promise((resolve) => {
					socket.close(() => resolve());
				}),
		};
	};
	const artnet = await openReceiver("art_net");
	const sacn = await openReceiver("sacn");
	return {
		artnet,
		sacn,
		snapshot: () => ({ artnet: artnet.snapshot(), sacn: sacn.snapshot() }),
		close: async () => {
			await Promise.all([artnet.close(), sacn.close()]);
		},
	};
}

function decodeNetworkDmxPayload(protocol, packet) {
	if (
		protocol === "art_net" &&
		packet.length >= 18 &&
		packet.subarray(0, 8).equals(Buffer.from("Art-Net\0")) &&
		packet.readUInt16LE(8) === 0x5000
	) {
		const length = packet.readUInt16BE(16);
		return packet.length >= 18 + length
			? packet.subarray(18, 18 + length)
			: null;
	}
	if (protocol === "sacn" && packet.length >= 126) {
		const propertyValueCount = packet.readUInt16BE(123);
		const dmxLength = Math.max(0, propertyValueCount - 1);
		return packet.length >= 126 + dmxLength
			? packet.subarray(126, 126 + dmxLength)
			: null;
	}
	return null;
}

async function startPackagedOscHardware(deskAlias) {
	const command = dgram.createSocket("udp4");
	const feedback = dgram.createSocket("udp4");
	await Promise.all([bindUdp(command), bindUdp(feedback)]);
	const messages = [];
	feedback.on("message", (packet) => {
		const message = parseOscMessage(packet);
		if (message) messages.push({ ...message, receivedAt: Date.now() });
	});
	const send = (address, arguments_ = []) =>
		new Promise((resolve, reject) => {
			command.send(
				encodeOscMessage(address, arguments_),
				9000,
				"127.0.0.1",
				(error) => (error ? reject(error) : resolve()),
			);
		});
	const clientId = `supported-scale-${crypto.randomUUID()}`;
	const feedbackPort = feedback.address().port;
	const subscribe = async () => {
		const baseline = messages.length;
		await send("/light/subscribe", [clientId, deskAlias, feedbackPort]);
		return waitForOscMessage(
			messages,
			(message) => message.address === `/light/${deskAlias}/feedback/page`,
			1_000,
			baseline,
		);
	};
	for (let attempt = 0; attempt < 5; attempt++) {
		const subscribed = await subscribe().catch(() => null);
		if (subscribed)
			return {
				deskAlias,
				messages,
				send,
				subscribe,
				close: async () => {
					await send("/light/unsubscribe", [clientId]).catch(() => undefined);
					await Promise.all([closeUdp(command), closeUdp(feedback)]);
				},
			};
	}
	await Promise.all([closeUdp(command), closeUdp(feedback)]);
	throw new Error(`OSC hardware could not subscribe to desk ${deskAlias}`);
}

async function exerciseSupportedScaleOscPlayback(
	session,
	showId,
	hardware,
	networkCapture,
) {
	if (!hardware) throw new Error("supported-scale OSC hardware is unavailable");
	if (!networkCapture)
		throw new Error("supported-scale network capture is unavailable");
	await hardware.subscribe();
	for (const action of [
		{ type: "go_to", cue_number: 1 },
		{ type: "master", value: 0.5 },
	])
		await requestJson(
			"POST",
			"/api/v2/playback-actions",
			{
				request_id: crypto.randomUUID(),
				address: { kind: "playback", playback_number: 1 },
				action,
				surface: "physical",
			},
			{ session, showId, deskId: session.desk.id },
		);
	await new Promise((resolve) => setTimeout(resolve, 100));
	const actions = [
		["go", true, `osc-go-${crypto.randomUUID()}`],
		["flash", true, `osc-flash-press-${crypto.randomUUID()}`],
		["flash", false, `osc-flash-release-${crypto.randomUUID()}`],
		["master", 0.75, `osc-master-${crypto.randomUUID()}`],
	];
	const measurements = [];
	for (const [action, value, requestId] of actions) {
		const baseline = hardware.messages.length;
		const networkBaseline = networkCapture.snapshot();
		const sentAt = Date.now();
		await hardware.send(`/light/playback/1/${action}`, [value, requestId]);
		const [feedback, network] = await Promise.all([
			waitForOscMessage(
				hardware.messages,
				(message) =>
					message.address === `/light/${hardware.deskAlias}/feedback/action` &&
					message.arguments[0] === requestId,
				2_000,
				baseline,
			).catch((error) => {
				throw new Error(
					`Timed out waiting for packaged OSC ${action} feedback (${requestId}): ${error.message}`,
				);
			}),
			waitForNetworkDmxChange(networkCapture, networkBaseline, sentAt, 2_000),
		]);
		measurements.push({ action, value, requestId, sentAt, feedback, network });
		await new Promise((resolve) =>
			setTimeout(resolve, action === "go" ? 1_600 : 75),
		);
	}
	for (const action of [
		{ type: "go_to", cue_number: 1 },
		{ type: "master", value: 0.5 },
	])
		await requestJson(
			"POST",
			"/api/v2/playback-actions",
			{
				request_id: crypto.randomUUID(),
				address: { kind: "playback", playback_number: 1 },
				action,
				surface: "physical",
			},
			{ session, showId, deskId: session.desk.id },
		);
	return {
		source: "osc",
		measurements,
	};
}

async function waitForNetworkDmxChange(
	capture,
	baseline,
	sentAt,
	timeoutMillis,
) {
	const deadline = Date.now() + timeoutMillis;
	while (Date.now() < deadline) {
		const snapshot = capture.snapshot();
		const protocols = {};
		let complete = true;
		for (const protocol of ["artnet", "sacn"]) {
			const current = snapshot[protocol];
			const changed =
				current.trackedValueChanges > baseline[protocol].trackedValueChanges;
			const changedAt = changed
				? current.trackedValueHistory.at(-1)?.changedAt
				: null;
			complete &&= changed;
			protocols[protocol] = {
				changed,
				valueChanges:
					current.trackedValueChanges - baseline[protocol].trackedValueChanges,
				changedAt,
				elapsedMillis: changedAt ? Date.parse(changedAt) - sentAt : null,
			};
		}
		if (complete) return protocols;
		await new Promise((resolve) => setTimeout(resolve, 2));
	}
	const snapshot = capture.snapshot();
	return Object.fromEntries(
		["artnet", "sacn"].map((protocol) => [
			protocol,
			{
				changed:
					snapshot[protocol].trackedValueChanges >
					baseline[protocol].trackedValueChanges,
				valueChanges:
					snapshot[protocol].trackedValueChanges -
					baseline[protocol].trackedValueChanges,
				changedAt:
					snapshot[protocol].trackedValueHistory.at(-1)?.changedAt ?? null,
				elapsedMillis: null,
			},
		]),
	);
}

function bindUdp(socket) {
	return new Promise((resolve, reject) => {
		socket.once("error", reject);
		socket.bind(0, "127.0.0.1", () => {
			socket.off("error", reject);
			resolve();
		});
	});
}

function closeUdp(socket) {
	return new Promise((resolve) => socket.close(() => resolve()));
}

async function waitForOscMessage(
	messages,
	predicate,
	timeoutMillis,
	baseline = 0,
) {
	const deadline = Date.now() + timeoutMillis;
	while (Date.now() < deadline) {
		const message = messages.slice(baseline).find(predicate);
		if (message) return message;
		await new Promise((resolve) => setTimeout(resolve, 5));
	}
	throw new Error("Timed out waiting for packaged OSC feedback");
}

function encodeOscMessage(address, arguments_) {
	const tags = `,${arguments_
		.map((value) =>
			typeof value === "string"
				? "s"
				: typeof value === "boolean"
					? value
						? "T"
						: "F"
					: Number.isInteger(value)
						? "i"
						: "f",
		)
		.join("")}`;
	const parts = [oscString(address), oscString(tags)];
	for (const value of arguments_) {
		if (typeof value === "string") parts.push(oscString(value));
		else if (typeof value === "number") {
			const bytes = Buffer.alloc(4);
			if (Number.isInteger(value)) bytes.writeInt32BE(value);
			else bytes.writeFloatBE(value);
			parts.push(bytes);
		}
	}
	return Buffer.concat(parts);
}

function parseOscMessage(packet) {
	try {
		const address = readOscString(packet, 0);
		const tags = readOscString(packet, address.next);
		let offset = tags.next;
		const arguments_ = [];
		for (const tag of tags.value.slice(1)) {
			if (tag === "s") {
				const value = readOscString(packet, offset);
				arguments_.push(value.value);
				offset = value.next;
			} else if (tag === "i") {
				arguments_.push(packet.readInt32BE(offset));
				offset += 4;
			} else if (tag === "f") {
				arguments_.push(packet.readFloatBE(offset));
				offset += 4;
			} else if (tag === "T" || tag === "F") arguments_.push(tag === "T");
			else return null;
		}
		return { address: address.value, arguments: arguments_ };
	} catch {
		return null;
	}
}

function oscString(value) {
	const bytes = Buffer.from(`${value}\0`);
	const result = Buffer.alloc(Math.ceil(bytes.length / 4) * 4);
	bytes.copy(result);
	return result;
}

function readOscString(packet, offset) {
	const end = packet.indexOf(0, offset);
	if (end < 0) throw new Error("unterminated OSC string");
	return {
		value: packet.subarray(offset, end).toString("utf8"),
		next: Math.ceil((end + 1) / 4) * 4,
	};
}

async function configureSupportedScaleOutputRoutes(
	session,
	showId,
	capture,
	logicalUniverse,
) {
	if (!Number.isSafeInteger(logicalUniverse))
		throw new Error("supported-scale Playback fixture has no logical universe");
	for (const [routeId, route] of [
		[
			"supported-scale-artnet",
			{
				protocol: "art_net",
				logical_universe: logicalUniverse,
				destination_universe: 101,
				delivery_mode: "unicast",
				destination: `127.0.0.1:${capture.artnet.port}`,
				enabled: true,
				minimum_slots: 512,
			},
		],
		[
			"supported-scale-sacn",
			{
				protocol: "sacn",
				logical_universe: logicalUniverse,
				destination_universe: 102,
				delivery_mode: "unicast",
				destination: `127.0.0.1:${capture.sacn.port}`,
				enabled: true,
				minimum_slots: 512,
			},
		],
	]) {
		await requestJson(
			"POST",
			"/api/v2/output-routes/actions",
			{
				request_id: crypto.randomUUID(),
				action: { type: "create", route_id: routeId, route },
			},
			{ session, showId },
		);
	}
	const deadline = Date.now() + 5_000;
	while (Date.now() < deadline) {
		const snapshot = capture.snapshot();
		if (snapshot.artnet.packets > 0 && snapshot.sacn.packets > 0) return;
		await new Promise((resolve) => setTimeout(resolve, 25));
	}
	throw new Error("configured Art-Net and sACN routes emitted no UDP packets");
}

function benchmarkFailureReport(
	reason,
	phase,
	samplesFile,
	profile,
	scene,
	processMemory,
) {
	const message = reason instanceof Error ? reason.message : String(reason);
	return {
		schemaVersion: 1,
		tier: packagedStageProfile(profile).tier,
		measurementSurface: "packaged-tauri-webview",
		profile,
		profileLabel: packagedStageProfile(profile).label,
		scene: scene ?? null,
		host: hostHardware(),
		durationSeconds: 0,
		samplesFile,
		acceptanceGateEnforced: false,
		failures: [`packaged benchmark failed during ${phase}: ${message}`],
		execution: {
			phase,
			requestedOutputHz: packagedStageProfile(profile).targetHz,
			completed: false,
			error: message,
		},
		processMemory: {
			measurement: `${process.platform} light-desktop main-process resident set`,
			samples: processMemory,
		},
	};
}

function isLargeOperatorProfile(candidate) {
	return candidate === "large-stage" || candidate === "supported-scale";
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
			showId,
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
	const playbackFixtureId = dynamicsPlan.staticControlFixtureIds.at(-1);
	const playbackFixture = after.fixtures.find(
		(fixture) => fixture.fixture_id === playbackFixtureId,
	);
	if (!playbackFixtureId || !playbackFixture?.fixture_number)
		throw new Error("Supported-scale Playback workload has no static fixture");
	const playbackPatch =
		playbackFixture.split_patches?.find(
			(patch) => patch.universe != null && patch.address != null,
		) ?? playbackFixture;
	if (
		!Number.isSafeInteger(playbackPatch.universe) ||
		!Number.isSafeInteger(playbackPatch.address)
	)
		throw new Error("Supported-scale Playback fixture has no DMX patch");
	const playbackWorkload =
		profile === "supported-scale"
			? await installSupportedScalePlaybackWorkload(
					session,
					showId,
					playbackFixtureId,
					playbackFixture.fixture_number,
					playbackPatch.universe,
					playbackPatch.address,
				)
			: null;
	const staticControlFixtureIds = playbackWorkload
		? dynamicsPlan.staticControlFixtureIds.filter(
				(fixtureId) => fixtureId !== playbackFixtureId,
			)
		: dynamicsPlan.staticControlFixtureIds;
	await setStaticControlIntensity(session, showId, staticControlFixtureIds);
	const scene = {
		...summarizeScene(profile, after.fixtures),
		inventory: largeScene.inventory,
		categoryCounts: largeScene.categoryCounts,
		patch: largeScene.patch,
		dynamics,
		playbackWorkload,
		staticControlFixtureCount:
			staticControlFixtureIds.length + largeScene.addedMultipatchInstances,
	};
	if (
		scene.fixtureRecords !== LARGE_STAGE_FIXTURE_RECORDS ||
		scene.fixtureInstances !== LARGE_STAGE_FIXTURE_INSTANCES
	)
		throw new Error(
			`Large Stage resolved to ${scene.fixtureRecords} records and ${scene.fixtureInstances} instances`,
		);
	if (profile === "supported-scale")
		return {
			scene: {
				...scene,
				addedFixtureRecords: largeScene.addedFixtureRecords,
				addedMultipatchInstances: largeScene.addedMultipatchInstances,
			},
			session,
			showId,
			showSwitch: null,
		};
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
		showId,
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

async function installSupportedScalePlaybackWorkload(
	session,
	showId,
	fixtureId,
	fixtureNumber,
	logicalUniverse,
	dmxAddress,
) {
	await setProgrammerFixtureIntensity(session, showId, fixtureId, 0.2);
	const first = await recordSupportedScaleCue(session, showId, 1, 1_000);
	await setProgrammerFixtureIntensity(session, showId, fixtureId, 0.8);
	const second = await recordSupportedScaleCue(
		session,
		showId,
		2,
		1_500,
		first.show_revision,
	);
	const page = await requestJson(
		"GET",
		"/api/v2/objects/playback_page/1",
		undefined,
		{ session, showId },
	);
	const playback = second.projections?.playback;
	if (!playback || playback.body?.number !== 1)
		throw new Error("Supported-scale Cue recording returned no Playback 1");
	await requestJson(
		"POST",
		"/api/v2/playback-topology/actions",
		{
			request_id: crypto.randomUUID(),
			action: {
				type: "map_existing_playback",
				page: 1,
				slot: 1,
				playback_number: 1,
				expected_page_revision: page.object?.revision ?? 0,
				expected_page_object_id: page.object?.id ?? null,
				expected_playback_revision: playback.revision,
				expected_playback_object_id: playback.id,
			},
		},
		{
			session,
			showId,
			deskId: session.desk.id,
			revision: second.show_revision,
		},
	);
	await requestJson(
		"POST",
		`/api/v2/control-desks/${encodeURIComponent(session.desk.id)}/actions`,
		{
			request_id: crypto.randomUUID(),
			action: { type: "set_page", page: 1, existing_only: true },
		},
		{ session, showId, deskId: session.desk.id },
	);
	await clearProgrammerValues(session, showId);
	await requestJson(
		"POST",
		"/api/v2/playback-actions",
		{
			request_id: crypto.randomUUID(),
			address: { kind: "playback", playback_number: 1 },
			action: { type: "go_to", cue_number: 1 },
			surface: "physical",
		},
		{ session, showId, deskId: session.desk.id },
	);
	return {
		playbackNumber: 1,
		page: 1,
		slot: 1,
		fixtureId,
		fixtureNumber,
		logicalUniverse,
		dmxAddress,
		cueListId: second.projections.cue_list.id,
		cueCount: 2,
		cueFadeMillis: [1_000, 1_500],
	};
}

async function recordSupportedScaleCue(
	session,
	showId,
	cueNumber,
	fadeMillis,
	revision,
) {
	const currentRevision =
		revision ??
		(
			await requestJson("GET", "/api/v2/objects/cue_list", undefined, {
				session,
				showId,
			})
		).show_revision;
	return requestJson(
		"POST",
		"/api/v2/cues/record",
		{
			request_id: crypto.randomUUID(),
			target: { kind: "pool", playback_number: 1 },
			operation: "overwrite",
			cue_number: cueNumber,
			timing: { fade_millis: fadeMillis, delay_millis: 0 },
			cue_only: false,
			name: "Plan 31 Scale Playback",
			capture_policy: "current_capture",
			activation_policy: "hold",
		},
		{ session, showId, revision: currentRevision },
	);
}

async function setProgrammerFixtureIntensity(
	session,
	showId,
	fixtureId,
	value,
) {
	return mutateProgrammerValues(session, showId, {
		type: "batch",
		mutations: [
			{
				type: "set_fixture",
				fixture_id: fixtureId,
				attribute: "intensity",
				value: { kind: "normalized", value },
				timing: {
					fade: false,
					fade_millis: null,
					delay_millis: null,
				},
			},
		],
	});
}

async function clearProgrammerValues(session, showId) {
	return mutateProgrammerValues(session, showId, { type: "clear" });
}

async function mutateProgrammerValues(session, showId, action) {
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
	return requestJson(
		"POST",
		`/api/v2/users/${encodeURIComponent(userId)}/programmer-values/actions`,
		{
			request_id: crypto.randomUUID(),
			expected_revision: values.projection.revision,
			expected_capture_mode_revision: capture.projection.revision,
			action,
		},
		{ session, showId, deskId: session.desk.id },
	);
}

async function setStaticControlIntensity(session, showId, fixtureIds) {
	return mutateProgrammerValues(session, showId, {
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
	});
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
	const realTimeStageGateEnforced = !isLargeOperatorProfile(profile);
	const lifecycleStressEnforced = profile !== "supported-scale";
	const playbackIndication = summarizePackagedPlaybackIndication(
		complete.playbackActions,
	);
	const fixtureSheetConvergence = summarizeFixtureSheetConvergence(
		complete.fixtureSheetActions,
	);
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
	if (
		lifecycleStressEnforced &&
		!timeline.some((sample) => sample.additionalStageWindow === "opened")
	)
		failures.push("the representative additional Stage window did not open");
	if (
		isLargeOperatorProfile(profile) &&
		!["stage-3d", "fixture-sheet"].every((surface) =>
			complete.activeUiSurfaces?.includes(surface),
		)
	)
		failures.push(
			"the interactive large tier did not expose both Stage 3D and Fixture Sheet surfaces",
		);
	if (profile === "supported-scale") {
		for (const surface of ["command-line", "playback-bank"])
			if (!complete.activeUiSurfaces?.includes(surface))
				failures.push(`supported scale did not expose the bundled ${surface}`);
		for (const failure of playbackIndication.failures)
			failures.push(`Playback indication: ${failure}`);
		for (const failure of fixtureSheetConvergence.failures)
			failures.push(`Fixture Sheet: ${failure}`);
	}
	assertPackagedQualityObjects(qualityObjects, failures);
	const outstandingContexts =
		(stage?.rendererContextsCreated ?? 0) -
		(stage?.rendererContextsDisposed ?? 0);
	if (outstandingContexts > 2)
		failures.push(
			"renderer context ownership grew beyond the two visible surfaces",
		);
	if (lifecycleStressEnforced && (stage?.rendererContextLosses ?? 0) < 1)
		failures.push("the Stage benchmark did not observe a WebGL context loss");
	if (lifecycleStressEnforced && (stage?.rendererContextRestores ?? 0) < 1)
		failures.push(
			"the Stage benchmark did not observe WebGL context restoration",
		);
	if (
		lifecycleStressEnforced &&
		complete.contextRecoveryMethod !== "webgl_lose_context"
	)
		failures.push(
			"the packaged context gate did not use the WEBGL_lose_context extension",
		);
	if (lifecycleStressEnforced && (stage?.desktopMirrorRenders ?? 0) < 1)
		failures.push(
			"the sibling Stage window did not acknowledge a mirrored canvas render",
		);
	if (lifecycleStressEnforced && (runtime.showSwitch?.completed ?? 0) !== 4)
		failures.push(
			"the packaged Stage run did not complete two active-show round trips",
		);
	if (
		lifecycleStressEnforced &&
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
	if (profile === "supported-scale") {
		for (const protocol of ["artnet", "sacn"]) {
			const beforePackets =
				runtime.networkCapture?.afterNoStage?.[protocol]?.packets ?? 0;
			const afterPackets =
				runtime.networkCapture?.after?.[protocol]?.packets ?? 0;
			if (afterPackets <= beforePackets)
				failures.push(
					`packaged Stage window dispatched no actual ${protocol} UDP packets`,
				);
		}
	}
	const visualization = summarizeVisualizationWindow(runtime);
	const programmerActionTiming = summarizePackagedProgrammerActionTiming(
		runtime,
		profile,
	);
	for (const failure of programmerActionTiming.failures)
		failures.push(`programmer action timing: ${failure}`);
	const playbackOutputCorrelation = summarizePlaybackOutputCorrelation(
		runtime,
		programmerActionTiming,
		complete.playbackActions,
	);
	if (profile === "supported-scale")
		for (const failure of playbackOutputCorrelation.failures)
			failures.push(`Playback output correlation: ${failure}`);
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
		isLargeOperatorProfile(profile) &&
		visualization.streamQueueDrops + visualization.streamSendFailures === 0
	)
		failures.push(
			"packaged paused visualization client caused no bounded queue replacement or send failure",
		);
	if (isLargeOperatorProfile(profile)) {
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
		const expectedStaticControls = profile === "supported-scale" ? 439 : 440;
		if (scene.staticControlFixtureCount !== expectedStaticControls)
			failures.push(
				`packaged large Stage did not retain ${expectedStaticControls} fixed-dimmer Programmer instances`,
			);
		if (
			profile === "supported-scale" &&
			(scene.playbackWorkload?.cueCount !== 2 ||
				scene.playbackWorkload?.cueFadeMillis?.[1] !== 1_500)
		)
			failures.push(
				"supported scale did not prepare the two-Cue fading Playback workload",
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
			playbackIndication,
			fixtureSheetConvergence,
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
			playbackOutputCorrelation,
		},
		capabilities: complete.capabilities ?? null,
	};
}

function summarizePackagedPlaybackIndication(samples) {
	const actions = Array.isArray(samples) ? samples : [];
	const failures = [];
	for (const action of ["go", "flash_press", "flash_release", "master"]) {
		const sample = actions.find((candidate) => candidate.action === action);
		if (!sample) failures.push(`missing ${action} DOM input sample`);
		else if (!sample.changed)
			failures.push(`${action} produced no visible bundled indication`);
		else if (sample.indicationMillis > 50)
			failures.push(
				`${action} indication took ${sample.indicationMillis.toFixed(2)} ms`,
			);
	}
	return {
		samples: actions.length,
		maximumMillis: Math.max(
			0,
			...actions.map((sample) => sample.indicationMillis ?? 0),
		),
		measurements: actions,
		passed: failures.length === 0,
		failures,
	};
}

function summarizeFixtureSheetConvergence(samples) {
	const actions = Array.isArray(samples) ? samples : [];
	const failures = [];
	for (const action of ["single", "burst"]) {
		const sample = actions.find((candidate) => candidate.action === action);
		if (!sample) failures.push(`missing ${action} convergence sample`);
		else if (!sample.changed)
			failures.push(`${action} update did not reach a visible row`);
		else if (sample.convergenceMillis > 500)
			failures.push(
				`${action} convergence took ${sample.convergenceMillis.toFixed(2)} ms`,
			);
	}
	if (actions.some((sample) => sample.loadingOverlayVisible))
		failures.push("a measured update displayed a loading overlay");
	if (
		actions.some((sample) => sample.visibleRows >= LARGE_STAGE_FIXTURE_RECORDS)
	)
		failures.push(
			"the DOM retained every fixture instead of visible virtual rows",
		);
	if (actions.some((sample) => sample.visibleRows === 0))
		failures.push("a measured update had no visible virtual rows");
	return {
		samples: actions.length,
		maximumMillis: Math.max(
			0,
			...actions.map((sample) => sample.convergenceMillis ?? 0),
		),
		measurements: actions,
		passed: failures.length === 0,
		failures,
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

function summarizePackagedProgrammerActionTiming(runtime, profile = null) {
	const before = runtime?.before?.programmer_action_timing;
	const after = runtime?.after?.programmer_action_timing;
	const requirements =
		profile === "supported-scale"
			? {
					minimumSamples: 10,
					sources: ["http", "websocket", "osc"],
					actions: [
						"command_line_edit",
						"command_execute",
						"playback_go",
						"playback_flash_press",
						"playback_flash_release",
						"playback_master",
					],
					frameRateBands: ["at-or-below-60"],
				}
			: {
					minimumSamples: 2,
					sources: ["http"],
					actions: ["command_line_edit", "command_execute"],
					frameRateBands: ["at-or-below-60"],
				};
	return summarizeProgrammerActionTiming(
		after,
		latestProgrammerActionId(before),
		requirements,
	);
}

function summarizePlaybackOutputCorrelation(
	runtime,
	programmerActionTiming,
	uiActions,
) {
	const exercises = [
		["control", runtime?.controlPlaybackTimingExercise],
		["stage", runtime?.stagePlaybackTimingExercise],
	];
	const measurements = exercises.flatMap(([phase, exercise]) =>
		(Array.isArray(exercise?.measurements) ? exercise.measurements : []).map(
			(measurement) => ({ ...measurement, phase }),
		),
	);
	const actionTimingByRequestId = new Map(
		(programmerActionTiming?.measurements ?? []).map((measurement) => [
			measurement.request_id,
			measurement,
		]),
	);
	const failures = [];
	for (const phase of ["control", "stage"])
		for (const [action, value] of [
			["go", true],
			["flash", true],
			["flash", false],
			["master", 0.75],
		]) {
			const measurement = measurements.find(
				(candidate) =>
					candidate.phase === phase &&
					candidate.action === action &&
					candidate.value === value,
			);
			const label = `${phase} ${action}${action === "flash" ? (value ? " press" : " release") : ""}`;
			if (!measurement) {
				failures.push(`missing ${label} measurement`);
				continue;
			}
			if (measurement.feedback?.arguments?.[0] !== measurement.requestId)
				failures.push(`${label} feedback did not retain its request ID`);
			for (const protocol of ["artnet", "sacn"])
				if (measurement.network?.[protocol]?.changed !== true)
					failures.push(`${label} produced no changed ${protocol} DMX payload`);
			const timing = actionTimingByRequestId.get(measurement.requestId);
			if (!timing)
				failures.push(`${label} has no authenticated server timing record`);
			else if (
				timing.acknowledgement_within_budget !== true ||
				timing.output_within_budget !== true
			)
				failures.push(`${label} exceeded its server output-tick budget`);
		}
	const uiMeasurements = (Array.isArray(uiActions) ? uiActions : []).map(
		(sample) => {
			const timing = actionTimingByRequestId.get(sample.requestId);
			const network = Object.fromEntries(
				["artnet", "sacn"].map((protocol) => {
					const change = runtime?.networkCapture?.after?.[
						protocol
					]?.trackedValueHistory?.find((candidate) => {
						const changedAt = Date.parse(candidate.changedAt);
						return (
							changedAt >= sample.inputEpochMillis &&
							changedAt <= sample.inputEpochMillis + 500
						);
					});
					return [protocol, change ?? null];
				}),
			);
			return { ...sample, timing: timing ?? null, network };
		},
	);
	for (const sample of uiMeasurements) {
		if (!sample.requestId) {
			failures.push(`UI ${sample.action} has no request ID`);
			continue;
		}
		if (!sample.timing)
			failures.push(
				`UI ${sample.action} has no authenticated server timing record`,
			);
		else if (
			sample.timing.acknowledgement_within_budget !== true ||
			sample.timing.output_within_budget !== true
		)
			failures.push(
				`UI ${sample.action} exceeded its server output-tick budget`,
			);
		for (const protocol of ["artnet", "sacn"])
			if (!sample.network[protocol])
				failures.push(
					`UI ${sample.action} produced no changed ${protocol} DMX slot`,
				);
	}
	return {
		samples: measurements.length + uiMeasurements.length,
		measurements,
		uiMeasurements,
		passed: failures.length === 0,
		failures,
	};
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
