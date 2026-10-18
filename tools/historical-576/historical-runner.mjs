// CI-only historical comparison; tools/historical-576 module layout.
// No schema-5 acceptance, server-locked clock, semantic-equivalence or causal claim.
import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { readFile, writeFile, mkdir, lstat } from "node:fs/promises";
import { spawn, execFileSync } from "node:child_process";
import { createServer } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createPerformanceFixtureInputs } from "../stage-large-scene.mjs";
import { createLargeStageDynamicsPlan } from "../stage-dynamics-scene.mjs";
import {
	readLinuxProcessTree,
	processTreeResourceDelta,
} from "../process-tree-resources.mjs";
import { configurePacketProof } from "../desktop-performance-576-proof-observers.mjs";
import { artifactPaths } from "../artifact-paths.mjs";
import { translateLegacyPlan } from "./legacy-plan.mjs";
import { CONTROL_PROFILE } from "./old-benchmark-shim.mjs";
import { installDefinitions } from "./definition-response.mjs";
import { bindInstalledProfiles } from "./profile-response.mjs";
import { stopOwned, applyCleanupResult } from "./owned-process-cleanup.mjs";
export const ARMS = Object.freeze([
	"old-legacy",
	"current-legacy",
	"current-typed",
]);
const ROOT = fileURLToPath(new URL("../../", import.meta.url));
export const INPUT_SHA256 =
	"0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f";
const MAX_JSON = 32 * 1024 * 1024,
	MAX_LOG = 16 * 1024 * 1024;
const uint = (n) => Number.isSafeInteger(n) && n >= 0;
const hex = (s, n) =>
	typeof s === "string" && new RegExp(`^[a-f0-9]{${n}}$`, "u").test(s);
const digest = (b) => createHash("sha256").update(b).digest("hex");
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const closed = (r, keys) =>
	assert.deepEqual(Object.keys(r).sort(), keys.slice().sort());
export function stable(value) {
	if (Array.isArray(value)) return "[" + value.map(stable).join(",") + "]";
	if (value && typeof value === "object")
		return (
			"{" +
			Object.keys(value)
				.sort()
				.map((k) => JSON.stringify(k) + ":" + stable(value[k]))
				.join(",") +
			"}"
		);
	return JSON.stringify(value);
}
export function canonicalHeadId(root, head) {
	const h = digest(`tl775-mixed576:${root}:${head}`);
	return `${h.slice(0, 8)}-${h.slice(8, 12)}-4${h.slice(13, 16)}-8${h.slice(17, 20)}-${h.slice(20, 32)}`;
}
function headFor(rows, head, index) {
	const matches = rows.filter(
		(r) =>
			r.profile_head_id === head ||
			(r.profile_head_id === null && r.head_index === index),
	);
	assert.equal(matches.length, 1, "ambiguous logical head");
	return matches[0];
}
// Build the canonical plan FIRST. Actual IDs can only replace already-ordered activation IDs.
export function targetBijection(canonical, actual) {
	assert.equal(canonical.fixtures.length, actual.fixtures.length);
	const actualRoots = new Map(actual.fixtures.map((f) => [f.fixture_id, f]));
	assert.equal(actualRoots.size, actual.fixtures.length);
	const profiles = new Map(
		canonical.profile_revisions.map((p) => [
			`${p.profile_id}:${p.profile_revision}`,
			p.profile_snapshot,
		]),
	);
	const forward = new Map(),
		reverse = new Map();
	const add = (from, to) => {
		assert(typeof to === "string");
		assert(!forward.has(from) || forward.get(from) === to);
		assert(!reverse.has(to) || reverse.get(to) === from);
		forward.set(from, to);
		reverse.set(to, from);
	};
	for (const expected of canonical.fixtures) {
		const observed = actualRoots.get(expected.fixture_id);
		assert(observed, "missing fixture root");
		for (const key of [
			"profile_id",
			"profile_revision",
			"mode_id",
			"fixture_number",
			"split_patches",
			"multipatch",
		])
			assert.equal(
				stable(observed[key]),
				stable(expected[key]),
				`patch field ${key}`,
			);
		add(expected.fixture_id, observed.fixture_id);
		const mode = profiles
			.get(`${expected.profile_id}:${expected.profile_revision}`)
			?.modes.find((m) => m.id === expected.mode_id);
		assert(mode);
		const heads = mode.heads
			.map((head, index) => ({ head, index }))
			.filter(({ head }) => !head.master_shared);
		assert.equal(expected.logical_heads.length, heads.length);
		assert.equal(observed.logical_heads.length, heads.length);
		for (const { head, index } of heads) {
			const before = headFor(expected.logical_heads, head.id, index),
				after = headFor(observed.logical_heads, head.id, index);
			assert.equal(
				before.fixture_id,
				canonicalHeadId(expected.fixture_id, head.id),
			);
			add(before.fixture_id, after.fixture_id);
		}
	}
	return forward;
}
export function remapPlan(plan, bijection) {
	const result = structuredClone(plan);
	for (const activation of result.activations)
		activation.targets = activation.targets.map((target) => {
			assert(bijection.has(target));
			return bijection.get(target);
		});
	// Metadata descriptors remain canonical: they are never submitted as runtime target IDs.
	return result;
}
export function intervalBounds(a, b) {
	for (const row of [a, b]) {
		assert(
			Number.isFinite(row.begin) && row.begin >= 0 && row.end >= row.begin,
		);
		for (const key of ["frames_sent", "deadline_misses", "send_errors"])
			assert(uint(row.output[key]));
	}
	assert(b.begin > a.end, "overlapping counter brackets");
	for (const k of ["frames_sent", "deadline_misses", "send_errors"])
		assert(b.output[k] >= a.output[k], "counter reset");
	const frames = b.output.frames_sent - a.output.frames_sent;
	assert(uint(frames));
	const widest = (b.end - a.begin) / 1000,
		narrowest = (b.begin - a.end) / 1000;
	return {
		frames,
		lower_hz: frames / widest,
		upper_hz: frames / narrowest,
		midpoint_hz: frames / ((b.begin + b.end - (a.begin + a.end)) / 2000),
	};
}
export function summarizeCounters(rows) {
	assert.equal(rows.length, 61);
	const intervals = rows.slice(1).map((b, i) => intervalBounds(rows[i], b));
	const total = intervalBounds(rows[0], rows.at(-1));
	const misses =
			rows.at(-1).output.deadline_misses - rows[0].output.deadline_misses,
		errors = rows.at(-1).output.send_errors - rows[0].output.send_errors;
	assert(uint(misses) && uint(errors));
	return {
		counter_observations: 61,
		interval_count: 60,
		window_lower_millis: rows.at(-1).begin - rows[0].end,
		window_upper_millis: rows.at(-1).end - rows[0].begin,
		completed_frames: total.frames,
		completed_hz_lower: total.lower_hz,
		completed_hz_midpoint: total.midpoint_hz,
		completed_hz_upper: total.upper_hz,
		minimum_interval_hz_lower: Math.min(...intervals.map((x) => x.lower_hz)),
		minimum_interval_hz_upper: Math.min(...intervals.map((x) => x.upper_hz)),
		intervals_definitely_below_38: intervals.filter((x) => x.upper_hz < 38)
			.length,
		intervals_possibly_below_38: intervals.filter((x) => x.lower_hz < 38)
			.length,
		deadline_misses: misses,
		send_errors: errors,
	};
}
export function sheetEvidence(records, start, end) {
	assert(end >= start);
	assert(!records.some((r) => r.kind === "stage-started"));
	const ready = records.filter(
		(r) =>
			r.kind === "fixture-sheet-ready" &&
			r.fixtureRecords === 576 &&
			r.profile === CONTROL_PROFILE,
	);
	assert(ready.some((r) => Date.parse(r.recordedAt) <= start));
	const beats = records.filter(
		(r) =>
			r.kind === "fixture-sheet-heartbeat" && r.profile === CONTROL_PROFILE,
	);
	assert(
		beats.length > 0 &&
			[...ready, ...beats].every(
				(r) =>
					r.schemaVersion === 1 &&
					r.measurementSurface === "packaged-tauri-webview-fixture-sheet",
			) &&
			beats.every(
				(r) =>
					r.fixtureRecords === 576 && uint(r.rowCount) && r.rowCount >= 576,
			),
	);
	const times = beats.map((r) => Date.parse(r.recordedAt));
	assert(times.every(Number.isFinite));
	assert(times.every((t, i) => i === 0 || t >= times[i - 1]));
	const before = times.filter((t) => t <= start).at(-1),
		after = times.find((t) => t >= end);
	assert(before !== undefined && after !== undefined);
	const span = times.filter((t) => t >= before && t <= after);
	const gap = Math.max(0, ...span.slice(1).map((t, i) => t - span[i]));
	assert(gap <= 5000);
	return {
		sheet_available: 1,
		sheet_records: 576,
		sheet_heartbeat_count: span.length,
		sheet_max_gap_millis: gap,
	};
}
// Common old/current runtime projection, not the internal portable checkpoint.
// Counts below describe admitted runtime identities/targets, not observed waveform values.
// Existing census samples include all process threads via /proc/PID/stat CPU ticks.
// This singleton view remains available when unrelated UI descendants turn over.
export function summarizeHeadlessResources(samples, options) {
	const unavailable = {
		headless_cpu_available: 0,
		headless_cpu_mean_percent: 0,
		headless_peak_resident_bytes: 0,
	};
	assert(samples.length === 61);
	let identity = null,
		previous = null,
		previousTime = null,
		weighted = 0,
		elapsed = 0,
		peak = 0;
	for (const sample of samples) {
		const rows = sample.tree.filter((p) =>
			p.command.includes("light-headless"),
		);
		if (rows.length !== 1) return unavailable;
		const row = rows[0];
		const key = `${row.pid}:${row.startTicks}`;
		if (identity !== null && identity !== key) return unavailable;
		identity = key;
		if (!uint(row.cpuTicks) || !uint(row.residentPages)) return unavailable;
		peak = Math.max(peak, row.residentPages * options.pageSize);
		const time = (sample.resource_begin + sample.resource_end) / 2;
		if (previous) {
			const seconds = (time - previousTime) / 1000;
			if (
				!(Number.isFinite(seconds) && seconds > 0) ||
				row.cpuTicks < previous.cpuTicks
			)
				return unavailable;
			const delta = processTreeResourceDelta(
				[previous],
				[row],
				seconds,
				options,
			);
			weighted += delta.cpuPercent * seconds;
			elapsed += seconds;
		}
		previous = row;
		previousTime = time;
	}
	assert(elapsed > 0 && Number.isFinite(weighted) && uint(peak));
	return {
		headless_cpu_available: 1,
		headless_cpu_mean_percent: weighted / elapsed,
		headless_peak_resident_bytes: peak,
	};
}
export function validateRuntime(snapshot, plan, starts) {
	assert.equal(snapshot.global_paused, false);
	assert.equal(snapshot.instances.length, 20);
	assert.equal(plan.activations.length, 20);
	assert.equal(starts.length, 20);
	const instances = new Map(snapshot.instances.map((i) => [i.dynamic_id, i]));
	assert.equal(instances.size, 20);
	assert.equal(new Set(snapshot.instances.map((i) => i.instance_id)).size, 20);
	const definitions = new Map(
		snapshot.definitions.map((d) => [d.dynamic_id, d]),
	);
	assert.equal(definitions.size, snapshot.definitions.length);
	let assignments = 0,
		controllers = 0;
	for (const [index, activation] of plan.activations.entries()) {
		const definition = activation.definition,
			instance = instances.get(definition.id),
			status = definitions.get(definition.id),
			started = starts[index];
		assert(instance && status && started);
		assert.equal(started.dynamic_id, definition.id);
		assert.equal(started.instance_id, instance.instance_id);
		assert.equal(started.started, true);
		assert.deepEqual(started.targets, activation.targets);
		assert.deepEqual(instance.targets, activation.targets);
		assert.equal(status.lane_count, definition.lanes.length);
		assert(uint(status.lane_count) && status.lane_count > 0);
		assert.equal(instance.pending, false);
		assert.equal(instance.paused, false);
		assert.equal(instance.phase_advancing, true);
		assert.equal(instance.speed_source, "Fixed");
		assert.equal(definition.speed.type, "fixed");
		assert.deepEqual(definition.overall_speed_multiplier, {
			numerator: 1,
			denominator: 1,
		});
		assert.equal(
			instance.effective_cycle_millis,
			definition.speed.duration_millis,
		);
		assert.equal(instance.controllers.length, 1);
		const c = instance.controllers[0];
		assert.equal(c.controller_id, started.controller_id);
		assert.equal(c.size, 1);
		assert.equal(c.speed_multiplier, 1);
		assert.equal(c.phase_offset_degrees, 0);
		assert.equal(c.paused, false);
		assert.equal(c.winning, true);
		assert.equal(c.releasing, false);
		assert.equal(c.activation_mix, 1);
		controllers++;
		assignments += instance.targets.length * status.lane_count;
		assert(uint(assignments));
	}
	assert.equal(
		new Set(
			snapshot.instances.flatMap((i) =>
				i.controllers.map((c) => c.controller_id),
			),
		).size,
		20,
	);
	assert.equal(controllers, 20);
	assert.equal(assignments, 5408);
	return { instances: 20, controllers: 20, assignments: 5408, checked: 1 };
}
async function bounded(file, cap = MAX_JSON) {
	const st = await lstat(file);
	assert(st.isFile() && !st.isSymbolicLink() && st.size <= cap);
	const b = await readFile(file);
	assert(b.length <= cap);
	return b;
}
async function writePrivate(file, value) {
	await writeFile(file, JSON.stringify(value, null, 2) + "\n", {
		flag: "wx",
		mode: 0o600,
	});
}
async function readRecords(file) {
	const b = await bounded(file, MAX_LOG);
	const text = b.toString("utf8");
	assert(text.endsWith("\n"));
	const rows = text.trim().split("\n").filter(Boolean);
	assert(rows.length <= 20000);
	return rows.map((s) => JSON.parse(s));
}
function apiClient(port, deadline) {
	return {
		session: null,
		showId: null,
		async request(method, route, body, revision) {
			assert(performance.now() < deadline, "overall deadline");
			assert(route.startsWith("/api/v2/"));
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
				signal: AbortSignal.timeout(10000),
			});
			assert(response.ok, "HTTP action failed");
			if (response.status === 204) return undefined;
			const chunks = [];
			let bytes = 0;
			for await (const chunk of response.body) {
				bytes += chunk.length;
				assert(bytes <= MAX_JSON);
				chunks.push(chunk);
			}
			return JSON.parse(Buffer.concat(chunks).toString("utf8"));
		},
	};
}
async function freePort() {
	const server = createServer();
	await new Promise((resolve, reject) => {
		server.once("error", reject);
		server.listen(0, "127.0.0.1", resolve);
	});
	const port = server.address().port;
	await new Promise((resolve) => server.close(resolve));
	return port;
}
async function waitFor(test, child, budget) {
	const deadline = performance.now() + budget;
	while (performance.now() < deadline) {
		assert(
			child.exitCode === null && child.signalCode === null,
			"owned app exited",
		);
		if (await test()) return;
		await sleep(250);
	}
	throw Error("bounded readiness deadline");
}
function assertTree(tree, rootIdentity) {
	assert(tree.length > 0 && tree.length <= 256);
	const root = tree.find((p) => p.pid === rootIdentity.pid);
	assert(root && root.startTicks === rootIdentity.startTicks);
	assert(tree.every((p) => uint(p.pid) && uint(p.startTicks)));
	assert.equal(
		tree.filter((p) => p.command.includes("light-headless")).length,
		1,
	);
	assert(
		!tree.some((p) => p.command.includes("viz-renderer")),
		"native renderer unexpectedly running",
	);
}
export function validateManifest(m, arm) {
	closed(m, ["schema_version", "input", "arms"]);
	assert.equal(m.schema_version, 1);
	closed(m.input, ["path", "sha256"]);
	assert(path.isAbsolute(m.input.path) && hex(m.input.sha256, 64));
	assert.equal(m.arms.length, 3);
	assert.deepEqual(m.arms.map((a) => a.id).sort(), ARMS.slice().sort());
	for (const a of m.arms) {
		closed(a, [
			"id",
			"revision_sha",
			"application_sha256",
			"source_manifest_sha256",
			"overlay_sha256",
			"compatibility_build",
		]);
		for (const k of ["revision_sha"]) assert(hex(a[k], 40));
		for (const k of [
			"application_sha256",
			"source_manifest_sha256",
			"overlay_sha256",
		])
			assert(hex(a[k], 64));
		assert.equal(a.compatibility_build, Number(a.id === "current-legacy"));
	}
	return m.arms.find((a) => a.id === arm);
}
export const REPORT_FIELDS = Object.freeze([
	"schema_version",
	"arm_code",
	"available",
	"complete",
	"failure_stage_code",
	"setup_checkpoint_code",
	"cleanup_verified",
	"compatibility_build",
	"schema5_acceptance_claim",
	"semantic_equivalence_claim",
	"server_locked_clock",
	"exact_start_alignment",
	"fixtures",
	"occupied_slots",
	"universes",
	"animated_assignments",
	"dynamic_instances",
	"counter_observations",
	"interval_count",
	"window_lower_millis",
	"window_upper_millis",
	"completed_frames",
	"completed_hz_lower",
	"completed_hz_midpoint",
	"completed_hz_upper",
	"minimum_interval_hz_lower",
	"minimum_interval_hz_upper",
	"intervals_definitely_below_38",
	"intervals_possibly_below_38",
	"deadline_misses",
	"send_errors",
	"sheet_available",
	"sheet_records",
	"sheet_heartbeat_count",
	"sheet_max_gap_millis",
	"stage_absence_available",
	"stage_absence_scope_code",
	"process_observations",
	"resource_available",
	"resource_partial",
	"application_cpu_mean_percent",
	"application_peak_resident_bytes",
	"packet_inventory_available",
	"packet_count",
	"packet_universes",
	"packet_sequence_discontinuities",
	"packet_zero_sequences",
	"decoded_ring_loss_observed",
	"decoded_ring_loss_checked",
	"static_fade_commands",
	"static_fades_passed",
	"elapsed_millis",
	"runtime_before_checked",
	"runtime_before_instances",
	"runtime_before_controllers",
	"runtime_before_assignments",
	"runtime_after_checked",
	"runtime_after_instances",
	"runtime_after_controllers",
	"runtime_after_assignments",
	"headless_cpu_available",
	"headless_cpu_mean_percent",
	"headless_peak_resident_bytes",
]);
export function validateReport(r) {
	closed(r, REPORT_FIELDS);
	for (const n of Object.values(r))
		assert(typeof n === "number" && Number.isFinite(n) && n >= 0);
	const fractional = new Set([
		"window_lower_millis",
		"window_upper_millis",
		"completed_hz_lower",
		"completed_hz_midpoint",
		"completed_hz_upper",
		"minimum_interval_hz_lower",
		"minimum_interval_hz_upper",
		"application_cpu_mean_percent",
		"elapsed_millis",
		"headless_cpu_mean_percent",
	]);
	for (const k of REPORT_FIELDS) if (!fractional.has(k)) assert(uint(r[k]));
	assert(
		r.counter_observations <= 61 &&
			r.interval_count <= 60 &&
			r.process_observations <= 62 &&
			r.packet_count <= 65536 &&
			r.packet_universes <= 16 &&
			r.sheet_heartbeat_count <= 20000 &&
			r.static_fade_commands <= 2 &&
			r.static_fades_passed <= 2,
	);
	assert(
		r.intervals_definitely_below_38 <= r.intervals_possibly_below_38 &&
			r.intervals_possibly_below_38 <= 60,
	);
	assert(r.window_lower_millis <= r.window_upper_millis);
	assert(
		r.completed_hz_lower <= r.completed_hz_midpoint &&
			r.completed_hz_midpoint <= r.completed_hz_upper,
	);
	assert.equal(r.compatibility_build, Number(r.arm_code === 2));
	assert(r.stage_absence_scope_code <= 1);
	assert(r.setup_checkpoint_code <= 9);
	if (r.resource_available) assert.equal(r.resource_partial, 0);
	if (!r.headless_cpu_available) {
		assert.equal(r.headless_cpu_mean_percent, 0);
		assert.equal(r.headless_peak_resident_bytes, 0);
	}
	for (const prefix of ["runtime_before", "runtime_after"]) {
		assert(
			r[`${prefix}_instances`] <= 20 &&
				r[`${prefix}_controllers`] <= 20 &&
				r[`${prefix}_assignments`] <= 5408,
		);
		if (r[`${prefix}_checked`]) {
			assert.equal(r[`${prefix}_instances`], 20);
			assert.equal(r[`${prefix}_controllers`], 20);
			assert.equal(r[`${prefix}_assignments`], 5408);
		}
	}
	assert.equal(r.schema_version, 2);
	assert(uint(r.arm_code) && r.arm_code >= 1 && r.arm_code <= 3);
	assert(uint(r.failure_stage_code) && r.failure_stage_code <= 12);
	for (const k of [
		"available",
		"complete",
		"cleanup_verified",
		"compatibility_build",
		"resource_available",
		"resource_partial",
		"packet_inventory_available",
		"sheet_available",
		"stage_absence_available",
		"decoded_ring_loss_checked",
		"runtime_before_checked",
		"runtime_after_checked",
		"headless_cpu_available",
	])
		assert(r[k] === 0 || r[k] === 1);
	for (const k of [
		"schema5_acceptance_claim",
		"semantic_equivalence_claim",
		"server_locked_clock",
		"exact_start_alignment",
	])
		assert.equal(r[k], 0);
	if (r.available) {
		assert.equal(r.setup_checkpoint_code, 9);
		assert(r.complete && r.cleanup_verified && r.failure_stage_code === 0);
		assert.equal(r.fixtures, 576);
		assert.equal(r.occupied_slots, 8064);
		assert.equal(r.universes, 16);
		assert.equal(r.animated_assignments, 5408);
		assert.equal(r.dynamic_instances, 20);
		assert.equal(r.counter_observations, 61);
		assert.equal(r.interval_count, 60);
		assert(
			r.sheet_available &&
				r.stage_absence_available &&
				r.packet_inventory_available,
		);
		assert.equal(r.packet_universes, 16);
		assert.equal(r.static_fades_passed, 2);
		assert.equal(r.static_fade_commands, 2);
		assert.equal(r.stage_absence_scope_code, 1);
		assert.equal(r.sheet_records, 576);
		assert(r.sheet_heartbeat_count >= 2 && r.sheet_max_gap_millis <= 5000);
		assert.equal(r.decoded_ring_loss_observed, 0);
		assert.equal(r.decoded_ring_loss_checked, 1);
		assert(r.completed_frames > 0 && r.window_lower_millis > 0);
		assert.equal(r.process_observations, 61);
		for (const prefix of ["runtime_before", "runtime_after"]) {
			assert.equal(r[`${prefix}_checked`], 1);
			assert.equal(r[`${prefix}_instances`], 20);
			assert.equal(r[`${prefix}_controllers`], 20);
			assert.equal(r[`${prefix}_assignments`], 5408);
		}
	}
	return r;
}
export const validateRow = validateReport;
export const PROVENANCE_FIELDS = Object.freeze([
	"schema_version",
	"arm_code",
	"available",
	"revision_sha",
	"application_sha256",
	"source_manifest_sha256",
	"overlay_sha256",
	"common_manifest_sha256",
	"input_sha256",
	"canonical_plan_sha256",
	"selected_plan_sha256",
	"target_bijection_sha256",
	"definition_bijection_sha256",
	"profile_bijection_sha256",
	"runner_sha256",
	"translator_sha256",
]);
export function validateProvenance(p) {
	closed(p, PROVENANCE_FIELDS);
	assert.equal(p.schema_version, 3);
	assert(uint(p.arm_code) && p.arm_code >= 1 && p.arm_code <= 3);
	assert(p.available === 0 || p.available === 1);
	for (const k of PROVENANCE_FIELDS.filter((k) => k.endsWith("_sha256")))
		assert(hex(p[k], 64));
	assert(hex(p.revision_sha, 40));
	if (p.available)
		assert(
			PROVENANCE_FIELDS.filter((k) => k.endsWith("_sha256")).every(
				(k) => p[k] !== "0".repeat(64),
			),
		);
	return p;
}
export function provenancePath(numericReport) {
	assert(numericReport.endsWith(".json"));
	return numericReport.slice(0, -5) + "-provenance.json";
}
// Each case owns fresh Tauri/WebKit preferences; HOME/runtime/D-Bus stay unchanged.
export function caseEnvironment(output, base = process.env) {
	assert(path.isAbsolute(output));
	return {
		...base,
		XDG_CONFIG_HOME: path.join(output, "xdg-config"),
		XDG_DATA_HOME: path.join(output, "xdg-data"),
		XDG_CACHE_HOME: path.join(output, "xdg-cache"),
	};
}
export async function runArm(options) {
	const arm = ARMS.indexOf(options.arm);
	assert(arm >= 0);
	const r = Object.fromEntries(REPORT_FIELDS.map((k) => [k, 0]));
	Object.assign(r, {
		schema_version: 2,
		arm_code: arm + 1,
		compatibility_build: Number(arm === 1),
	});
	const started = performance.now();
	let stage = 1,
		child,
		packets,
		provenance = Object.fromEntries(
			PROVENANCE_FIELDS.map((k) => [
				k,
				k.endsWith("_sha256") ? "0".repeat(64) : 0,
			]),
		);
	Object.assign(provenance, {
		schema_version: 3,
		arm_code: arm + 1,
		revision_sha: "0".repeat(40),
	});
	const raw = { samples: [], errors: [] };
	const output = path.resolve(options["output-dir"]);
	assert(output.startsWith(path.resolve(artifactPaths.tmp) + path.sep));
	await mkdir(output, { recursive: false, mode: 0o700 });
	const publicFile = path.resolve(options["numeric-report"]);
	assert(
		publicFile.startsWith(path.resolve(artifactPaths.results) + path.sep) ||
			publicFile.startsWith(path.resolve(artifactPaths.tmp) + path.sep),
	);
	assert(!publicFile.startsWith(output + path.sep) && publicFile !== output);
	provenancePath(publicFile);
	let signalFailure = false;
	const interrupted = () => {
		signalFailure = true;
		void stopOwned(child);
	};
	process.once("SIGTERM", interrupted);
	process.once("SIGINT", interrupted);
	const lifetimeTimer = setTimeout(interrupted, 420000);
	try {
		assert(options.manifest, "common manifest required");
		for (const k of [
			"LIGHT_OUTPUT_WORKERS",
			"LIGHT_OUTPUT_PHASE_DIAGNOSTICS",
			"LIGHT_HEADLESS_STACK_AUDIT",
			"LIGHT_HEADLESS_PROFILE",
		])
			assert(!process.env[k], `unexpected instrumentation override ${k}`);
		const manifestBytes = await bounded(options.manifest);
		const manifest = JSON.parse(manifestBytes);
		const a = validateManifest(manifest, options.arm);
		assert.equal(
			digest(await bounded(options.application, 1024 * 1024 * 1024)),
			a.application_sha256,
		);
		const inputBytes = await bounded(manifest.input.path);
		assert.equal(digest(inputBytes), manifest.input.sha256);
		assert.equal(manifest.input.sha256, INPUT_SHA256);
		const input = JSON.parse(inputBytes);
		closed(input, [
			"schema",
			"kind",
			"scene",
			"patch",
			"plan",
			"package_bindings",
			"source_bindings",
			"qualification",
		]);
		assert.equal(input.schema, 1);
		assert.equal(input.kind, "private.production-js-mixed576-live-input/v1");
		assert.deepEqual(input.source_bindings.map((b) => b.file).sort(), [
			"tools/mixed576-live-probe-input.mjs",
			"tools/stage-dynamics-scene.mjs",
			"tools/stage-large-scene.mjs",
		]);
		for (const b of input.package_bindings) {
			closed(b, ["file", "sha256", "profile_id"]);
			assert(hex(b.sha256, 64));
		}
		assert.equal(input.scene.fixtures.length, 576);
		assert.equal(input.scene.patch.occupiedSlots, 8064);
		assert.equal(input.scene.patch.universeCount, 16);
		assert.equal(input.package_bindings.length, 9);
		// The shared input was produced by createPerformanceFixtureInputs; bind that exact generator source.
		assert.equal(typeof createPerformanceFixtureInputs, "function");
		for (const b of input.source_bindings) {
			closed(b, ["file", "sha256"]);
			assert(
				typeof b.file === "string" &&
					!path.isAbsolute(b.file) &&
					!b.file.split("/").includes("..") &&
					hex(b.sha256, 64),
			);
			assert.equal(digest(await bounded(path.join(ROOT, b.file))), b.sha256);
		}
		assert.equal(
			stable(createLargeStageDynamicsPlan(input.patch, input.scene)),
			stable(input.plan),
		);
		assert.equal(input.plan.dynamicTargetCount, 5408);
		assert.equal(input.plan.activations.length, 20);
		const canonicalPlan =
			arm === 2 ? input.plan : translateLegacyPlan(input.plan);
		const canonicalHash = digest(stable(input.plan)),
			selectedHash = digest(stable(canonicalPlan));
		Object.assign(r, {
			fixtures: 576,
			occupied_slots: 8064,
			universes: 16,
			animated_assignments: 5408,
			dynamic_instances: 20,
		});
		const port = await freePort(),
			reportFile = path.join(output, "surface.jsonl"),
			prepared = path.join(output, "prepared");
		let bytes = 0;
		const appEnvironment = caseEnvironment(output);
		for (const key of ["XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"])
			await mkdir(appEnvironment[key], { mode: 0o700 });
		stage = 2;
		child = spawn(options.application, [], {
			cwd: ROOT,
			detached: true,
			env: {
				...appEnvironment,
				APPIMAGE_EXTRACT_AND_RUN: "1",
				LIGHT_DESKTOP_TEST_BIND: `127.0.0.1:${port}`,
				LIGHT_DESKTOP_TEST_DATA_DIR: path.join(output, "data"),
				LIGHT_STAGE_PACKAGED_BENCH_PREPARED: prepared,
				LIGHT_STAGE_PACKAGED_BENCH_REPORT: reportFile,
				LIGHT_STAGE_PACKAGED_BENCH_PROFILE: CONTROL_PROFILE,
				LIGHT_STAGE_PACKAGED_BENCH_STAGE: "0",
				LIGHT_STAGE_PACKAGED_BENCH_ADDITIONAL_STAGE_WINDOW: "0",
				LIGHT_STAGE_PACKAGED_BENCH_FIXTURE_SHEET: "1",
				LIGHT_STAGE_PACKAGED_BENCH_EXPECTED_FIXTURE_RECORDS: "576",
				LIGHT_STAGE_PACKAGED_BENCH_DURATION_SECONDS: "420",
				LIGHT_STAGE_PACKAGED_BENCH_CONTROL_DURATION_SECONDS: "900",
			},
			stdio: ["ignore", "pipe", "pipe"],
		});
		const logs = [];
		let spawnError = false;
		child.once("error", () => {
			spawnError = true;
		});
		for (const stream of [child.stdout, child.stderr])
			stream.on("data", (b) => {
				bytes += b.length;
				if (bytes <= MAX_LOG) logs.push(b);
				else interrupted();
			});
		raw.launch_logs = logs;
		const api = apiClient(port, started + 400000);
		await waitFor(
			async () => {
				if (spawnError) throw Error("spawn failed");
				try {
					return Boolean(await api.request("GET", "/api/v2/readiness"));
				} catch {
					return false;
				}
			},
			child,
			90000,
		);
		stage = 3;
		r.setup_checkpoint_code = 1;
		api.session = await api.request("POST", "/api/v2/sessions", {
			desk_id: null,
		});
		r.setup_checkpoint_code = 2;
		const bootstrap = await api.request("GET", "/api/v2/bootstrap");
		api.showId = bootstrap.active_show?.id;
		assert(api.showId);
		r.setup_checkpoint_code = 3;
		await api.request("POST", "/api/v2/configuration/update", {
			request_id: randomUUID(),
			patch: { frame_rate_hz: 40 },
		});
		r.setup_checkpoint_code = 4;
		const before = await api.request("GET", "/api/v2/patch"),
			library = await api.request("GET", "/api/v2/fixture-library/profiles");
		r.setup_checkpoint_code = 5;
		const { runtimeScene, runtimePatch, profileBindings } =
			bindInstalledProfiles(input, library.profiles, arm + 1);
		raw.profile_bindings = profileBindings;
		r.setup_checkpoint_code = 6;
		await api.request(
			"POST",
			"/api/v2/patch/fixtures",
			{
				request_id: randomUUID(),
				fixtures: runtimeScene.fixtures,
				remove_fixture_ids: before.fixtures.map((f) => f.fixture_id),
				placements: [],
			},
			before.patch_revision,
		);
		r.setup_checkpoint_code = 7;
		const actual = await api.request("GET", "/api/v2/patch"),
			mapping = targetBijection(runtimePatch, actual),
			selectedPlan = remapPlan(canonicalPlan, mapping);
		r.setup_checkpoint_code = 8;
		const { runtimePlan: plan, definitionBindings, starts } =
			await installDefinitions(api, selectedPlan, randomUUID);
		raw.definition_bindings = definitionBindings;
		provenance = {
			schema_version: 3,
			arm_code: arm + 1,
			available: 1,
			revision_sha: a.revision_sha,
			application_sha256: a.application_sha256,
			source_manifest_sha256: a.source_manifest_sha256,
			overlay_sha256: a.overlay_sha256,
			common_manifest_sha256: digest(manifestBytes),
			input_sha256: manifest.input.sha256,
			canonical_plan_sha256: canonicalHash,
			selected_plan_sha256: selectedHash,
			definition_bijection_sha256: digest(stable(definitionBindings)),
			profile_bijection_sha256: digest(stable(profileBindings)),
			target_bijection_sha256: digest(
				stable([...mapping].sort((a, b) => a[0].localeCompare(b[0]))),
			),
			runner_sha256: digest(await bounded(fileURLToPath(import.meta.url))),
			translator_sha256: digest(
				await bounded(
					fileURLToPath(new URL("./legacy-plan.mjs", import.meta.url)),
				),
			),
		};
		r.setup_checkpoint_code = 9;
		stage = 4;
		packets = await configurePacketProof(api, port);
		await writeFile(prepared, "ready\n", { flag: "wx", mode: 0o600 });
		stage = 5;
		await waitFor(
			async () => {
				try {
					const records = await readRecords(reportFile);
					return records.some(
						(x) =>
							x.kind === "fixture-sheet-ready" &&
							x.fixtureRecords === 576 &&
							x.profile === CONTROL_PROFILE,
					);
				} catch {
					return false;
				}
			},
			child,
			90000,
		);
		await sleep(20000);
		let firstTree = await readLinuxProcessTree(child.pid);
		const rootIdentity = firstTree.find((p) => p.pid === child.pid);
		assert(rootIdentity);
		assertTree(firstTree, rootIdentity);
		const ticksPerSecond = Number(
				execFileSync("getconf", ["CLK_TCK"], { encoding: "utf8" }).trim(),
			),
			pageSize = Number(
				execFileSync("getconf", ["PAGESIZE"], { encoding: "utf8" }).trim(),
			);
		assert(
			uint(ticksPerSecond) &&
				ticksPerSecond > 0 &&
				uint(pageSize) &&
				pageSize > 0,
		);
		stage = 6;
		const runtimeBefore = await api.request("GET", "/api/v2/dynamics/runtime");
		const beforeAdmission = validateRuntime(runtimeBefore, plan, starts);
		raw.runtime_before = runtimeBefore;
		for (const [key, value] of Object.entries(beforeAdmission))
			r[`runtime_before_${key}`] = value;
		packets.start();
		const openingWall = Date.now(),
			openingMono = performance.now();
		const resources = [];
		let previousTree, previousResourceTime;
		for (let i = 0; i <= 60; i++) {
			if (i) await sleep(1000);
			assert(!signalFailure && bytes <= MAX_LOG);
			assert(performance.now() - started < 400000);
			assert(performance.now() - openingMono < 95000);
			const begin = performance.now(),
				diagnostics = await api.request(
					"GET",
					"/api/v2/diagnostics/performance",
				),
				end = performance.now();
			const wall = Date.now();
			assert(
				Math.abs(wall - openingWall - (end - openingMono)) <= 100,
				"wall clock drift",
			);
			assert.equal(diagnostics.output.frame_hz, 40);
			const outputCounters = Object.fromEntries(
				["frames_sent", "deadline_misses", "send_errors"].map((k) => [
					k,
					diagnostics.output[k],
				]),
			);
			assert(Object.values(outputCounters).every(uint));
			const rb = performance.now(),
				tree = await readLinuxProcessTree(child.pid),
				re = performance.now();
			assertTree(tree, rootIdentity);
			const midpoint = (rb + re) / 2;
			if (previousTree) {
				resources.push(
					processTreeResourceDelta(
						previousTree,
						tree,
						(midpoint - previousResourceTime) / 1000,
						{ ticksPerSecond, pageSize },
					),
				);
				if (
					stable(previousTree.map((p) => [p.pid, p.startTicks]).sort()) !==
					stable(tree.map((p) => [p.pid, p.startTicks]).sort())
				)
					r.resource_partial = 1;
			}
			previousTree = tree;
			previousResourceTime = midpoint;
			raw.samples.push({
				begin,
				end,
				wall,
				output: outputCounters,
				tree,
				resource_begin: rb,
				resource_end: re,
			});
		}
		const runtimeAfter = await api.request("GET", "/api/v2/dynamics/runtime");
		const afterAdmission = validateRuntime(runtimeAfter, plan, starts);
		raw.runtime_after = runtimeAfter;
		for (const [key, value] of Object.entries(afterAdmission))
			r[`runtime_after_${key}`] = value;
		Object.assign(r, summarizeCounters(raw.samples));
		Object.assign(
			r,
			summarizeHeadlessResources(raw.samples, { ticksPerSecond, pageSize }),
		);
		r.process_observations = 61;
		r.resource_available = Number(!r.resource_partial);
		r.application_cpu_mean_percent =
			resources.reduce((sum, x) => sum + x.cpuPercent, 0) / resources.length;
		r.application_peak_resident_bytes = Math.max(
			...resources.map((x) => x.residentBytes),
		);
		const closingWall = raw.samples.at(-1).wall;
		stage = 7;
		const inventory = packets.snapshot({
			startedAt: openingWall,
			endedAt: closingWall,
		});
		r.packet_count = inventory.packet_count;
		r.packet_universes = inventory.rows.filter((x) => x.packets > 0).length;
		r.packet_sequence_discontinuities = inventory.rows.reduce(
			(n, x) => n + x.sequence_discontinuities,
			0,
		);
		r.packet_zero_sequences = inventory.rows.reduce(
			(n, x) => n + x.zero_sequence_packets,
			0,
		);
		r.packet_inventory_available = 1;
		r.decoded_ring_loss_checked = 1;
		stage = 8;
		const interaction = await packets.interact();
		r.static_fade_commands = interaction.accepted_commands;
		r.static_fades_passed = interaction.ramps.length;
		r.decoded_ring_loss_checked = 1;
		raw.packets = packets.privateState();
		await packets.close();
		packets = null;
		stage = 9;
		await sleep(1500);
		const records = await readRecords(reportFile);
		Object.assign(r, sheetEvidence(records, openingWall, closingWall));
		assert(
			records.some(
				(x) => x.kind === "started" && x.profile === CONTROL_PROFILE,
			),
		);
		const tail = await readLinuxProcessTree(child.pid);
		assertTree(tail, rootIdentity);
		r.stage_absence_available = 1;
		r.stage_absence_scope_code = 1;
		stage = 10;
		assert.equal(
			digest(await bounded(options.application, 1024 * 1024 * 1024)),
			a.application_sha256,
		);
		assert.equal(
			digest(await bounded(manifest.input.path)),
			manifest.input.sha256,
		);
		r.complete = 1;
	} catch (error) {
		r.failure_stage_code = stage;
		raw.errors.push({ stage, name: error?.name ?? "Error" });
	} finally {
		try {
			if (packets) {
				raw.packets = packets.privateState();
				await packets.close();
			}
		} catch {
			r.complete = 0;
			r.failure_stage_code ||= 11;
		}
		let cleanupVerified = 0;
		try {
			cleanupVerified = await stopOwned(child);
		} catch {
			// An unexpected cleanup failure never permits the next case to launch.
		}
		applyCleanupResult(r, cleanupVerified);
		clearTimeout(lifetimeTimer);
		process.removeListener("SIGTERM", interrupted);
		process.removeListener("SIGINT", interrupted);
		if (raw.launch_logs) {
			await writeFile(
				path.join(output, "application.log"),
				Buffer.concat(raw.launch_logs),
				{ flag: "wx", mode: 0o600 },
			);
			delete raw.launch_logs;
		}
		await writePrivate(path.join(output, "observations.json"), raw);
	}
	r.elapsed_millis = performance.now() - started;
	r.available = Number(
		r.complete && r.cleanup_verified && !r.failure_stage_code,
	);
	validateReport(r);
	await mkdir(path.dirname(publicFile), { recursive: true });
	await writePrivate(publicFile, r);
	validateProvenance(provenance);
	await writePrivate(provenancePath(publicFile), provenance);
	return r;
}
if (
	process.argv[1] &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const args = process.argv.slice(2),
		options = {};
	assert(args.length % 2 === 0);
	for (let i = 0; i < args.length; i += 2) {
		assert(
			args[i].startsWith("--") && !Object.hasOwn(options, args[i].slice(2)),
		);
		options[args[i].slice(2)] = args[i + 1];
	}
	assert(
		Object.keys(options).every((k) =>
			[
				"arm",
				"application",
				"output-dir",
				"numeric-report",
				"manifest",
			].includes(k),
		),
	);
	for (const k of ["arm", "application", "output-dir", "numeric-report"])
		assert(options[k]);
	for (const k of ["application", "output-dir", "numeric-report"])
		assert(path.isAbsolute(options[k]));
	const r = await runArm(options);
	console.log(JSON.stringify(r));
	process.exitCode = r.available ? 0 : 1;
}
