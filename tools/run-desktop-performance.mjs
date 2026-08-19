#!/usr/bin/env node

import { spawn, spawnSync } from "node:child_process";
import crypto from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import {
	processTreeResourceDelta,
	readLinuxProcessTree,
} from "./process-tree-resources.mjs";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";
import { createPerformanceFixtureInputs } from "./stage-large-scene.mjs";

const DURATION_SECONDS = 15;
const LINUX_PROCESS_OPTIONS = {
	ticksPerSecond: linuxConfiguration("CLK_TCK", 100),
	pageSize: linuxConfiguration("PAGESIZE", 4096),
};
const CASES = [
	{ caseId: "demo", fixtureRecords: 264, demo: true },
	{ caseId: "sixteen_universe", fixtureRecords: 576 },
	{ caseId: "required_1024", fixtureRecords: 1_024 },
	{ caseId: "maximum", fixtureRecords: 2_000 },
	{ caseId: "doubled_2048", fixtureRecords: 2_048 },
];

const options = parseArguments(process.argv.slice(2));
if (process.platform !== "linux")
	throw new Error(
		"The released Desk performance harness requires Linux /proc and Xvfb",
	);
await mkdir(path.resolve(options["output-dir"]), { recursive: true });
const results = [];
for (const executionMode of ["one_core", "unrestricted"])
	for (const performanceCase of CASES)
		results.push(await runCase(performanceCase, executionMode));
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
			LIGHT_STAGE_PACKAGED_BENCH_FIXTURE_SHEET: "1",
			LIGHT_STAGE_PACKAGED_BENCH_EXPECTED_FIXTURE_RECORDS: String(
				performanceCase.fixtureRecords,
			),
		},
		stdio: ["ignore", "pipe", "pipe"],
	});
	const output = [];
	for (const stream of [child.stdout, child.stderr])
		stream.on("data", (chunk) => {
			output.push(chunk.toString());
			if (output.length > 100) output.shift();
		});
	try {
		await waitForReadiness(child, port);
		const api = apiClient(port);
		// A desk admits the operators it was seeded with and no one else, so the measurement
		// signs in as one of them rather than inventing a name.
		const session = await api.request("POST", "/api/v2/sessions", {
			username: await enabledUser(api),
			desk_id: null,
		});
		api.session = session;
		await configureScheduler(api);
		const prepared = performanceCase.demo
			? await prepareDemo(api)
			: await prepareMixed(api, performanceCase.fixtureRecords);
		// The show exists now, so let the benchmark surface begin.
		await writeFile(preparedPath, `${JSON.stringify(prepared)}\n`);
		await waitForFixtureSheet(
			reportPath,
			performanceCase.fixtureRecords,
			child,
		);
		const measured = await measureWindow(api, child.pid, executionMode);
		await requireFixtureSheetActive(reportPath, performanceCase.fixtureRecords);
		return {
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
			requested_rate_hz: 60,
			below_target_hz: 44,
			measurement_seconds: DURATION_SECONDS,
			measurement_surface: "released-tauri-desk-fixture-sheet",
			...measured,
		};
	} catch (error) {
		throw new Error(
			`${performanceCase.caseId}/${executionMode}: ${error instanceof Error ? error.message : String(error)}${output.length ? `\n${output.join("")}` : ""}`,
		);
	} finally {
		await terminateProcessTree(child.pid);
		await rm(temporaryRoot, { recursive: true, force: true });
	}
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

async function enabledUser(api) {
	const bootstrap = await api.request("GET", "/api/v2/bootstrap");
	const user = (bootstrap.users ?? []).find((candidate) => candidate.enabled);
	if (!user) throw new Error("the released Desk has no enabled user to measure as");
	return user.name;
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
		const mode = fixture.definition.profile_snapshot?.modes?.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
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
		patch: { frame_rate_hz: 60 },
	});
}

async function measureWindow(api, rootPid, executionMode) {
	const rates = [];
	const resources = [];
	let diagnostics = await api.request("GET", "/api/v2/diagnostics/performance");
	let processes = await readLinuxProcessTree(rootPid);
	for (let second = 0; second < DURATION_SECONDS; second++) {
		const started = performance.now();
		await new Promise((resolve) => setTimeout(resolve, 1_000));
		const elapsed = (performance.now() - started) / 1_000;
		const nextDiagnostics = await api.request(
			"GET",
			"/api/v2/diagnostics/performance",
		);
		const nextProcesses = await readLinuxProcessTree(rootPid);
		rates.push(
			(nextDiagnostics.output.frames_sent - diagnostics.output.frames_sent) /
				elapsed,
		);
		const server = (entries) =>
			entries.filter((entry) => entry.command.includes("light-headless"));
		const ui = (entries) =>
			entries.filter((entry) => !entry.command.includes("light-headless"));
		const resourceDelta = (before, after) =>
			processTreeResourceDelta(before, after, elapsed, LINUX_PROCESS_OPTIONS);
		resources.push({
			application: resourceDelta(processes, nextProcesses),
			server: processTreeResourceDelta(
				server(processes),
				server(nextProcesses),
				elapsed,
				LINUX_PROCESS_OPTIONS,
			),
			ui: resourceDelta(ui(processes), ui(nextProcesses)),
		});
		diagnostics = nextDiagnostics;
		processes = nextProcesses;
	}
	const sorted = [...rates].sort((left, right) => left - right);
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
	return {
		minimum_one_second_completed_hz: sorted[0],
		average_completed_hz: average(rates),
		p95_one_second_completed_hz: percentile(sorted, 95),
		maximum_one_second_completed_hz: sorted.at(-1),
		windows_below_target: rates.filter((rate) => rate < 44).length,
		resources: {
			application_cpu_average_percent: average(cpu),
			application_cpu_max_percent: Math.max(...cpu),
			application_peak_resident_bytes: Math.max(
				...resources.map((sample) => sample.application.residentBytes),
			),
			server: component("server"),
			desktop_webview: component("ui"),
			playwright: { launched: false },
			measurement:
				"Released Tauri Desk process tree: desktop host, WebKit Fixture Sheet, and bundled light-headless sidecar; excludes the Node coordinator and Xvfb",
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

async function waitForFixtureSheet(reportPath, expected, child) {
	const deadline = Date.now() + 60_000;
	while (Date.now() < deadline) {
		assertRunning(child);
		const records = await readFile(reportPath, "utf8").catch(() => "");
		const ready = records
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
			.find(
				(record) =>
					record.kind === "fixture-sheet-ready" &&
					record.fixtureRecords === expected,
			);
		if (ready) return;
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(
		`Fixture Sheet did not converge to ${expected} fixture records`,
	);
}

async function requireFixtureSheetActive(reportPath, expected) {
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
	const heartbeat = records
		.filter(
			(record) =>
				record.kind === "fixture-sheet-heartbeat" &&
				record.fixtureRecords === expected,
		)
		.at(-1);
	if (!heartbeat || Date.now() - Date.parse(heartbeat.recordedAt) > 2_500)
		throw new Error(
			"Fixture Sheet did not remain active through the timed measurement window",
		);
}

function assertRunning(child) {
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
