import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { pathToFileURL } from "node:url";
const ROOT = path.resolve(process.env.LIGHT_REPOSITORY_ROOT ?? process.cwd());
const runnerLocation = new URL("./historical-runner.mjs", import.meta.url);
let runnerSource = await readFile(runnerLocation, "utf8");
runnerSource = runnerSource.replaceAll(
	"import.meta.url",
	JSON.stringify(
		pathToFileURL(path.join(ROOT, "tools/historical-576/historical-runner.mjs"))
			.href,
	),
);
for (const file of [
	"stage-large-scene.mjs",
	"stage-dynamics-scene.mjs",
	"process-tree-resources.mjs",
	"desktop-performance-576-proof-observers.mjs",
	"artifact-paths.mjs",
])
	runnerSource = runnerSource.replace(
		JSON.stringify("../" + file),
		JSON.stringify(pathToFileURL(path.join(ROOT, "tools", file)).href),
	);
for (const file of ["legacy-plan.mjs", "old-benchmark-shim.mjs", "definition-response.mjs", "profile-response.mjs", "owned-process-cleanup.mjs"])
	runnerSource = runnerSource.replace(
		JSON.stringify("./" + file),
		JSON.stringify(new URL("./" + file, import.meta.url).href),
	);
const {
	ARMS,
	REPORT_FIELDS,
	PROVENANCE_FIELDS,
	validateRow,
	validateProvenance,
	provenancePath,
	intervalBounds,
	summarizeCounters,
	sheetEvidence,
	canonicalHeadId,
	targetBijection,
	remapPlan,
	validateManifest,
	validateRuntime,
	caseEnvironment,
	summarizeHeadlessResources,
} = await import(
	"data:text/javascript;base64," + Buffer.from(runnerSource).toString("base64")
);

import {
	oldBenchmarkControlShim,
	CONTROL_PROFILE,
	OLD_REVISION,
	OLD_CONTROL_SHA256,
} from "./old-benchmark-shim.mjs";
const zero = () =>
	Object.assign(Object.fromEntries(REPORT_FIELDS.map((k) => [k, 0])), {
		schema_version: 2,
		arm_code: 1,
	});
const throws = (fn) => assert.throws(fn);
test("closed numeric row rejects privacy, fractional counters and false success", () => {
	assert.equal(validateRow(zero()).available, 0);
	for (const change of [
		{ pid: 123 },
		{ setup_checkpoint_code: 10 },
		{ setup_checkpoint_code: 0.5 },
		{ schema_version: 1 },
		{ arm_code: 0 },
		{ completed_frames: 0.5 },
		{ available: 1 },
		{ compatibility_build: 1 },
		{ server_locked_clock: 1 },
		{ packet_count: 65537 },
		{ completed_hz_lower: 2, completed_hz_upper: 1 },
	])
		throws(() => validateRow({ ...zero(), ...change }));
});
test("HEX-only seal and exact companion filename", () => {
	const p = Object.fromEntries(
		PROVENANCE_FIELDS.map((k) => [
			k,
			k.endsWith("_sha256") ? "0".repeat(64) : 0,
		]),
	);
	Object.assign(p, {
		schema_version: 3,
		arm_code: 1,
		revision_sha: "0".repeat(40),
	});
	validateProvenance(p);
	throws(() => validateProvenance({ ...p, path: "/private" }));
	throws(() => validateProvenance({ ...p, available: 1 }));
	throws(() => validateProvenance({ ...p, runner_sha256: "symbol" }));
	assert.equal(
		provenancePath("/private/arm.json"),
		"/private/arm-provenance.json",
	);
	throws(() => provenancePath("/private/arm"));
});
const sample = (i, frames = i * 40) => ({
	begin: i * 1020,
	end: i * 1020 + 20,
	output: { frames_sent: frames, deadline_misses: i, send_errors: 0 },
});
test("request bounds enclose actual sample instant and midpoint is qualified", () => {
	const b = intervalBounds(sample(0), sample(1));
	assert.equal(b.lower_hz, 40 / 1.04);
	assert.equal(b.upper_hz, 40);
	assert(b.lower_hz < b.midpoint_hz && b.midpoint_hz < b.upper_hz);
	throws(() => intervalBounds(sample(0), { ...sample(1), begin: 10 }));
	throws(() => intervalBounds(sample(1), sample(2, 0)));
});
test("61 external observations retain all 60 counters and reject reset", () => {
	const rows = Array.from({ length: 61 }, (_, i) => sample(i));
	const r = summarizeCounters(rows);
	assert.equal(r.completed_frames, 2400);
	assert.equal(r.deadline_misses, 60);
	assert.equal(r.interval_count, 60);
	assert.equal(r.window_lower_millis, 61180);
	assert.equal(r.window_upper_millis, 61220);
	throws(() => summarizeCounters(rows.slice(1)));
	rows[30].output.deadline_misses = 0;
	throws(() => summarizeCounters(rows));
	rows[30].output.deadline_misses = 30;
	rows[40].output.send_errors = 2;
	throws(() => summarizeCounters(rows));
});
const rec = (kind, t) => ({
	kind,
	recordedAt: new Date(t).toISOString(),
	schemaVersion: 1,
	measurementSurface: "packaged-tauri-webview-fixture-sheet",
	profile: CONTROL_PROFILE,
	fixtureRecords: 576,
	rowCount: 800,
});
test("Sheet evidence requires bracketed active heartbeats, no Stage start", () => {
	const rows = [
		rec("fixture-sheet-ready", 1000),
		...Array.from({ length: 65 }, (_, i) =>
			rec("fixture-sheet-heartbeat", 1000 + i * 1000),
		),
	];
	assert.equal(sheetEvidence(rows, 2000, 62000).sheet_available, 1);
	throws(() =>
		sheetEvidence([...rows, rec("stage-started", 5000)], 2000, 62000),
	);
	throws(() =>
		sheetEvidence(
			rows.filter((r) => r.kind !== "fixture-sheet-ready"),
			2000,
			62000,
		),
	);
	throws(() => sheetEvidence(rows.slice(0, 5), 2000, 62000));
	throws(() =>
		sheetEvidence(
			rows.filter(
				(r) =>
					Date.parse(r.recordedAt) < 20000 || Date.parse(r.recordedAt) > 30000,
			),
			2000,
			62000,
		),
	);
});
function patch() {
	const profile = {
		id: "p",
		modes: [{ id: "m", heads: [{ id: "h", master_shared: false }] }],
	};
	return {
		profile_revisions: [
			{ profile_id: "p", profile_revision: 1, profile_snapshot: profile },
		],
		fixtures: [
			{
				fixture_id: "root",
				profile_id: "p",
				profile_revision: 1,
				mode_id: "m",
				fixture_number: 1,
				split_patches: [],
				multipatch: [],
				logical_heads: [
					{
						profile_head_id: "h",
						head_index: 0,
						fixture_id: canonicalHeadId("root", "h"),
					},
				],
			},
		],
	};
}
test("canonical target partitions stay frozen before server-ID remap", () => {
	const canonical = patch(),
		actual = structuredClone(canonical);
	actual.fixtures[0].logical_heads[0].fixture_id = "actual-head";
	const map = targetBijection(canonical, actual);
	const plan = {
		activations: [
			{
				targets: [canonical.fixtures[0].logical_heads[0].fixture_id, "root"],
				definition: { id: "d" },
			},
		],
		targetDescriptors: [{ id: "canonical" }],
	};
	const out = remapPlan(plan, map);
	assert.deepEqual(out.activations[0].targets, ["actual-head", "root"]);
	assert.deepEqual(out.targetDescriptors, plan.targetDescriptors);
	assert.notDeepEqual(out, plan);
	actual.fixtures[0].logical_heads.push({
		...actual.fixtures[0].logical_heads[0],
	});
	throws(() => targetBijection(canonical, actual));
	throws(() => remapPlan({ activations: [{ targets: ["missing"] }] }, map));
});
test("common manifest binds exactly three arms and compatibility label", () => {
	const m = {
		schema_version: 1,
		input: { path: "/private/input", sha256: "a".repeat(64) },
		arms: ARMS.map((id, i) => ({
			id,
			revision_sha: "a".repeat(40),
			application_sha256: "b".repeat(64),
			source_manifest_sha256: "c".repeat(64),
			overlay_sha256: "d".repeat(64),
			compatibility_build: Number(i === 1),
		})),
	};
	assert.equal(validateManifest(m, "current-legacy").compatibility_build, 1);
	throws(() => validateManifest({ ...m, extra: 1 }, "old-legacy"));
	m.arms[0].compatibility_build = 1;
	throws(() => validateManifest(m, "old-legacy"));
});
test("old shim refuses arbitrary source before any mutation", () => {
	throws(() => oldBenchmarkControlShim("unrecognized source"));
});

test("actual frozen old frontend shim preserves Sheet geometry and guards only benchmark Stage", async () => {
	const root = ROOT;
	const old = execFileSync(
		"git",
		[
			"show",
			`${OLD_REVISION}:apps/light-desktop/src/NativePackagedStageBenchmarkApp.tsx`,
		],
		{ cwd: root, encoding: "utf8", maxBuffer: 65536 },
	);
	const patched = oldBenchmarkControlShim(old);
	assert.equal(
		createHash("sha256").update(patched).digest("hex"),
		OLD_CONTROL_SHA256,
	);
	assert(patched.includes("active={controlOnly ? prepared : stageEnabled}"));
	assert(patched.includes("if (controlOnly) return;"));
	const oldSheet = execFileSync(
		"git",
		[
			"show",
			`${OLD_REVISION}:apps/light-desktop/src/windows/FixtureSheetWindow.tsx`,
		],
		{ cwd: root, encoding: "utf8", maxBuffer: 262144 },
	);
	const currentSheet = await readFile(
		path.join(ROOT, "apps/light-desktop/src/windows/FixtureSheetWindow.tsx"),
		"utf8",
	);
	for (const source of [oldSheet, currentSheet])
		for (const literal of [
			'"fixture-sheet-ready"',
			'"fixture-sheet-heartbeat"',
			'"packaged-tauri-webview-fixture-sheet"',
			"schemaVersion: 1",
			"recordedAt: new Date().toISOString()",
		])
			assert(source.includes(literal));
});

test("actual runtime admission requires every live instance/controller, ordered target and lane count", async () => {
	assert(
		process.env.LIGHT_HISTORICAL_INPUT &&
			path.isAbsolute(process.env.LIGHT_HISTORICAL_INPUT),
	);
	const inputBytes = await readFile(process.env.LIGHT_HISTORICAL_INPUT);
	assert.equal(
		createHash("sha256").update(inputBytes).digest("hex"),
		"0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f",
	);
	const { plan } = JSON.parse(
		await readFile(process.env.LIGHT_HISTORICAL_INPUT, "utf8"),
	);
	const starts = plan.activations.map((a, i) => ({
		dynamic_id: a.definition.id,
		instance_id: `instance-${i}`,
		controller_id: `controller-${i}`,
		started: true,
		targets: a.targets,
	}));
	const snapshot = {
		global_paused: false,
		definitions: plan.activations.map((a) => ({
			dynamic_id: a.definition.id,
			lane_count: a.definition.lanes.length,
		})),
		instances: plan.activations.map((a, i) => ({
			instance_id: starts[i].instance_id,
			dynamic_id: a.definition.id,
			targets: a.targets,
			pending: false,
			paused: false,
			phase_advancing: true,
			speed_source: "Fixed",
			effective_cycle_millis: a.definition.speed.duration_millis,
			controllers: [
				{
					controller_id: starts[i].controller_id,
					size: 1,
					speed_multiplier: 1,
					phase_offset_degrees: 0,
					paused: false,
					winning: true,
					releasing: false,
					activation_mix: 1,
				},
			],
		})),
	};
	assert.deepEqual(validateRuntime(snapshot, plan, starts), {
		instances: 20,
		controllers: 20,
		assignments: 5408,
		checked: 1,
	});
	const mutations = [
		(s) => s.instances.pop(),
		(s) => (s.global_paused = true),
		(s) => (s.instances[0].paused = true),
		(s) => (s.instances[0].controllers[0].paused = true),
		(s) => (s.instances[0].targets = s.instances[0].targets.slice().reverse()),
		(s) => s.definitions[0].lane_count++,
		(s) => (s.instances[0].controllers[0].size = 0.5),
		(s) => (s.instances[0].controllers[0].speed_multiplier = 2),
		(s) => s.instances[0].effective_cycle_millis++,
		(s) => (s.instances[0].controllers[0].winning = false),
		(s) => (s.instances[0].controllers[0].activation_mix = 0.5),
		(s) => (s.instances[0].dynamic_id = "unknown"),
	];
	for (const mutate of mutations) {
		const bad = structuredClone(snapshot);
		mutate(bad);
		throws(() => validateRuntime(bad, plan, starts));
	}
	const badStarts = structuredClone(starts);
	badStarts[0].started = false;
	throws(() => validateRuntime(snapshot, plan, badStarts));
});

test("case XDG roots isolate preferences without repurposing HOME/runtime/D-Bus", () => {
	const original = {
		HOME: "/operator",
		XDG_RUNTIME_DIR: "/run/user/1000",
		DBUS_SESSION_BUS_ADDRESS: "unix:operator",
		XDG_CONFIG_HOME: "/old/config",
		XDG_DATA_HOME: "/old/data",
		XDG_CACHE_HOME: "/old/cache",
	};
	const a = caseEnvironment("/private/case-a", original),
		b = caseEnvironment("/private/case-b", original);
	for (const key of ["HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"]) {
		assert.equal(a[key], original[key]);
		assert.equal(b[key], original[key]);
	}
	for (const key of ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"]) {
		assert(a[key].startsWith("/private/case-a/"));
		assert(b[key].startsWith("/private/case-b/"));
		assert.notEqual(a[key], b[key]);
	}
	assert.equal(original.XDG_DATA_HOME, "/old/data");
	throws(() => caseEnvironment("relative", original));
});

test("headless CPU uses weighted existing samples despite UI turnover; replacement is unavailable", () => {
	const rows = Array.from({ length: 61 }, (_, i) => {
		const time = i * i * 1000;
		return {
			resource_begin: time,
			resource_end: time + 10,
			tree: [
				{
					pid: 100,
					startTicks: 10,
					command: "light-headless",
					cpuTicks: i * 100,
					residentPages: 100 + i,
				},
				{
					pid: 200 + i,
					startTicks: i,
					command: "WebKit",
					cpuTicks: 0,
					residentPages: 1,
				},
			],
		};
	});
	const r = summarizeHeadlessResources(rows, {
		ticksPerSecond: 100,
		pageSize: 4096,
	});
	assert.equal(r.headless_cpu_available, 1);
	assert(Math.abs(r.headless_cpu_mean_percent - 100 / 60) < 1e-12);
	assert.equal(r.headless_peak_resident_bytes, 160 * 4096);
	const replaced = structuredClone(rows);
	replaced[30].tree[0].pid = 101;
	assert.deepEqual(
		summarizeHeadlessResources(replaced, {
			ticksPerSecond: 100,
			pageSize: 4096,
		}),
		{
			headless_cpu_available: 0,
			headless_cpu_mean_percent: 0,
			headless_peak_resident_bytes: 0,
		},
	);
	const forged = zero();
	forged.headless_cpu_mean_percent = 1;
	throws(() => validateRow(forged));
});
