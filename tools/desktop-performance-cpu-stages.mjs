import { constants } from "node:fs";
import { chmod, lstat, mkdir, mkdtemp, open, readdir, readFile, readlink, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { parseLinuxProcessStat } from "./process-tree-resources.mjs";
import { artifactPaths } from "./artifact-paths.mjs";
const LIMIT = 1024;
const MAX_BYTES = 64 * 1024 * 1024;
const MAX_NATIVE_ROWS = LIMIT * 9;

const PHASES = [
  "capture_settle",
  "projection_inclusive",
  "speed_lock_acquire",
  "dynamics_lock_acquire",
  "scratch_lock_acquire",
  "family_lock_acquire",
  "family_preparation_inclusive",
  "family_retire",
  "family_finalize",
  "reconciliation"
];

const FAMILY = [
  "initial_static",
  "missing_fixed_bases",
  "repeat_static",
  "bind_and_control",
  "sampling_before_callback",
  "scalar_static",
  "final_geometry",
  "complete_deferred",
  "prepare_families",
  "cohort_setup",
  "position_compose",
  "owner_compose",
  "ordered_finish",
  "native_projection"
];

const TUPLE = ["calls", "wall_ns", "thread_cpu_ns", "runnable_queue_ns", "timeslices", "counter_read_wall_ns"];

const NATIVE_META = [
  "schema",
  "role",
  "ordinal",
  "relative_ns",
  "sampled_at_millis",
  "source_before",
  "source_after",
  "presented_before",
  "presented_after",
  "transport",
  "outcome",
  "redrawn",
  "send_outcome"
];

const NATIVE_COUNTERS = ["calls", "wall_ns", "thread_cpu_ns", "queue_ns", "slices", "counter_read_wall_ns"];
const OVERHEAD = ["schema", "role", "ordinal", "sampled_at_millis", "export_wall_ns", "window_wall_ns"];

const MANIFEST = [
  "private_capture_schema",
  "child_pid",
  "parent_pid",
  "spawn_ordinal",
  "program",
  "arguments",
  "stderr_path"
];

const FLAGS = [
  "LIGHT_OUTPUT_PHASE_DIAGNOSTICS",
  "LIGHT_OUTPUT_TYPED_CPU_DIAGNOSTICS",
  "LIGHT_VIZ_TYPED_CPU_DIAGNOSTICS"
];

function require_(condition, message) {
  if (!condition)
	  throw new Error(`Typed CPU diagnostic STOP: ${message}`);
}

function uint(value, maximum = Number.MAX_SAFE_INTEGER) {
  require_(Number.isSafeInteger(value) && value >= 0 && value <= maximum, "invalid unsigned numeric field");
	return value;
}

function nullable(value) {
  return value === null ? null : uint(value);
}

function exact(value, keys) {
  require_(value !== null && !Array.isArray(value) && typeof value === "object", "expected record");

	require_(
	  Object.keys(value).length === keys.length && Object.keys(value).every(key => keys.includes(key)),
		"unknown or missing record field"
	);
}

function fixedArray(value, length) {
  require_(Array.isArray(value) && value.length === length, "wrong fixed array size");
	return value.map(entry => uint(entry));
}

function sample(value) {
  uint(value, LIMIT);
	require_(value > 0, "sample ordinal must be positive");
	return value;
}

function add(a, b) {
  return uint(a + b);
}

function tuple(value, native = false) {
  const keys = native ? NATIVE_COUNTERS : TUPLE;
	exact(value, keys);

	const result = {
	  calls: uint(value.calls),
	  wall_ns: uint(value.wall_ns),
	  thread_cpu_ns: nullable(value.thread_cpu_ns),
	  runnable_queue_ns: nullable(value[native ? "queue_ns" : "runnable_queue_ns"]),
	  timeslices: nullable(value[native ? "slices" : "timeslices"]),
	  counter_read_wall_ns: uint(value.counter_read_wall_ns)
	};

	if (result.calls === 0) require_(
	  result.wall_ns === 0 && result.counter_read_wall_ns === 0 && result.thread_cpu_ns === null && result.runnable_queue_ns === null && result.timeslices === null,
		"not-run tuple contains observations"
	);

	return result;
}

function pick(value, keys) {
  return Object.fromEntries(keys.map(key => [key, value[key]]));
}

export function cpuEnvironment(mode, directory) {
  require_(["off", "on"].includes(mode), "mode must be off or on");

	if (mode === "on")
	  require_(typeof directory === "string" && path.isAbsolute(directory), "capture directory must be absolute");

	return {
	  ...Object.fromEntries(FLAGS.map(key => [key, mode === "on" ? "1" : "0"])),
	  LIGHT_VIZ_TYPED_CPU_CAPTURE_DIR: mode === "on" ? directory : undefined
	};
}

export function parseNativeCapture(text) {
  require_(Buffer.byteLength(text) <= MAX_BYTES, "native capture byte cap exceeded");
	const groups = new Map();
	let records = 0;

	for (const line of text.split("\n")) {
	  if (!line.trim())
		  continue;

		if (!line.trimStart().startsWith("{"))
		  continue; // Unrelated private stderr is never exported.

		require_(++records <= MAX_NATIVE_ROWS, "native record cap exceeded");
		const row = JSON.parse(line);
		require_(row.schema === 1 && row.role === 2, "unknown native schema or role");
		const ordinal = sample(row.ordinal);

		const group = groups.get(ordinal) ?? {
		  stages: new Map(),
		  overhead: null
		};

		if (Object.hasOwn(row, "stage")) {
		  exact(row, [...NATIVE_META, "stage", ...NATIVE_COUNTERS]);
			uint(row.stage, 7);

			for (const key of NATIVE_META.filter(key => key !== "sampled_at_millis"))
			  uint(row[key]);

			nullable(row.sampled_at_millis);
			require_([1, 2].includes(row.transport), "unknown native transport");
			uint(row.outcome, 3);
			uint(row.redrawn, 1);
			uint(row.send_outcome, 2);
			require_(!group.stages.has(row.stage), "duplicate native stage");

			group.stages.set(row.stage, {
			  meta: pick(row, NATIVE_META),
			  tuple: tuple(pick(row, NATIVE_COUNTERS), true)
			});
		} else {
		  exact(row, OVERHEAD);
			nullable(row.sampled_at_millis);
			uint(row.export_wall_ns);
			uint(row.window_wall_ns);
			require_(group.overhead === null, "duplicate native overhead");
			group.overhead = row;
		}

		groups.set(ordinal, group);
	}

	require_(groups.size <= LIMIT, "native sample cap exceeded");

	for (const group of groups.values()) {
	  require_(group.stages.size === 8 && group.overhead !== null, "incomplete native sample");
		const meta = group.stages.get(0).meta;

		for (const entry of group.stages.values())
		  require_(JSON.stringify(entry.meta) === JSON.stringify(meta), "conflicting native sample metadata");

		require_(group.overhead.sampled_at_millis === meta.sampled_at_millis, "conflicting native anchor");
	}

	return groups;
}

export function parseServerCapture(text) {
  require_(Buffer.byteLength(text) <= MAX_BYTES, "server capture byte cap exceeded");
	const groups = new Map();
	let rows = 0;

	for (const raw of text.split("\n")) {
	  const line = raw.replace(/\x1b\[[0-9;]*m/gu, "");

		if (!line.includes("typed_cpu_payload="))
		  continue;

		require_(!/[\x00-\x1f\x7f]/u.test(line.trimEnd()), "unsupported server control escape");
		require_(++rows <= LIMIT * 2, "server record cap exceeded");
		const pieces = line.split("typed_cpu_payload=");

		require_(
		  pieces.length === 2 && /bounded (?:typed output major CPU|live output typed nested CPU) sample/u.test(pieces[0]),
			"invalid server diagnostic envelope"
		);

		const row = JSON.parse(pieces[1].trim());
		require_(row.schema === 1 && row.role === 1 && [0, 1].includes(row.kind), "unknown server schema/role/kind");
		const expectedEvent = row.kind === 0 ? "bounded typed output major CPU sample" : "bounded live output typed nested CPU sample";
		require_(pieces[0].trimEnd().endsWith(expectedEvent), "server event does not match kind");
		sample(row.sample);
		nullable(row.sampled_at_millis);
		const group = groups.get(row.sample) ?? {};
		require_(!group[row.kind], "duplicate server sample kind");
		require_(row.worker_cpu_coverage === 0 && row.runqueue_availability === 0, "unsupported CPU attribution");

		if (row.kind === 0) {
		  exact(row, [
			  "schema",
			  "role",
			  "kind",
			  "sample",
			  "sampled_at_millis",
			  "major",
			  "async_cpu_coverage",
			  "worker_cpu_coverage",
			  "runqueue_availability"
			]);

			require_(row.async_cpu_coverage === 0, "async CPU attribution is unavailable");
			exact(row.major, ["tick", "parts"]);
			exact(row.major.parts, ["dynamic", "engine", "publish", "send"]);

			group[0] = {
			  anchor: row.sampled_at_millis,

			  tuples: [
				  tuple(row.major.tick),
				  ...["dynamic", "engine", "publish", "send"].map(name => tuple(row.major.parts[name]))
				]
			};

			for (const entry of [group[0].tuples[0], group[0].tuples[4]]) require_(
			  entry.thread_cpu_ns === null && entry.runnable_queue_ns === null && entry.timeslices === null,
				"async tuple claims CPU"
			);
		} else {
		  exact(row, [
			  "schema",
			  "role",
			  "kind",
			  "sample",
			  "generation",
			  "show_revision",
			  "sampled_at_millis",
			  "accepted",
			  "phases",
			  "family_stages",
			  "family_stage_calls",
			  "family_counts",
			  "worker_cpu_coverage",
			  "runqueue_availability"
			]);

			nullable(row.generation);
			nullable(row.show_revision);
			uint(row.accepted, 1);
			exact(row.phases, PHASES);
			exact(row.family_stages, FAMILY);
			const calls = fixedArray(row.family_stage_calls, 14);
			const counts = fixedArray(row.family_counts, 22);
			const family = FAMILY.map(name => tuple(row.family_stages[name]));
			family.forEach((value, index) => require_(value.calls <= calls[index], "family call count mismatch"));

			group[1] = {
			  anchor: row.sampled_at_millis,
			  accepted: row.accepted,
			  counts,
			  calls,
			  tuples: [...PHASES.map(name => tuple(row.phases[name])), ...family]
			};
		}

		groups.set(row.sample, group);
	}

	require_(groups.size <= LIMIT, "server sample cap exceeded");
	return groups;
}

export function qualifyWindow(requests) {
  if (requests.length !== 61) return {
	  available: 0,
	  opening_start_ms: null,
	  opening_end_ms: null,
	  closing_start_ms: null,
	  closing_end_ms: null,
	  clock_step_detected: 0,
	  opening_monotonic_start_ns: null,
	  opening_monotonic_end_ns: null,
	  closing_monotonic_start_ns: null,
	  closing_monotonic_end_ns: null
	};

	let step = false;
	let previous;
	let initialOffset;

	for (const request of requests) {
	  exact(request, ["wall_start_ms", "wall_end_ms", "mono_start_ns", "mono_end_ns"]);

		for (const value of Object.values(request))
		  uint(value);

		const anchors = [
		  [request.wall_start_ms, request.mono_start_ns / 1e6],
		  [request.wall_end_ms, request.mono_end_ns / 1e6]
		];

		for (const anchor of anchors) {
		  initialOffset ??= anchor[0] - anchor[1];

			if (Math.abs(anchor[0] - anchor[1] - initialOffset) > 5)
			  step = true;

			if (previous && (anchor[0] < previous[0] || anchor[1] < previous[1] || Math.abs(anchor[0] - previous[0] - (anchor[1] - previous[1])) > 5))
			  step = true;

			previous = anchor;
		}
	}

	const first = requests[0], last = requests.at(-1);

	return {
	  available: step ? 0 : 1,
	  opening_start_ms: uint(first.wall_start_ms),
	  opening_end_ms: uint(first.wall_end_ms),
	  closing_start_ms: uint(last.wall_start_ms),
	  closing_end_ms: uint(last.wall_end_ms),
	  clock_step_detected: step ? 1 : 0,
	  opening_monotonic_start_ns: uint(first.mono_start_ns),
	  opening_monotonic_end_ns: uint(first.mono_end_ns),
	  closing_monotonic_start_ns: uint(last.mono_start_ns),
	  closing_monotonic_end_ns: uint(last.mono_end_ns)
	};
}

// Class 0 inside, 1 outside, 2 boundary-ambiguous, 3 missing anchor, 4 clock/window unbound.
export function windowClass(anchor, durationNs, window) {
  nullable(anchor);
	uint(durationNs);

	if (anchor === null)
	  return 3;

	if (!window.available)
	  return 4;

	const end = add(anchor, Math.ceil(durationNs / 1e6));

	if (end < window.opening_start_ms || anchor > window.closing_end_ms)
	  return 1;

	return anchor - 6 > window.opening_end_ms && end + 6 < window.closing_start_ms ? 0 : 2;
}

export function validateOwnedBinding(manifest, receipts, timestamp, durationNs) {
  const end = timestamp === null ? null : timestamp + Math.ceil(durationNs / 1e6);

	if (timestamp === null)
	  return false;

	for (let index = 0; index + 1 < receipts.length; index++) {
	  const before = receipts[index], after = receipts[index + 1];

		if (!(before.wall_end_ms < timestamp - 6 && end + 6 < after.wall_start_ms))
		  continue;

		const a = before.owned.find(entry => entry.pid === manifest.child_pid);
		const b = after.owned.find(entry => entry.pid === manifest.child_pid);

		if (a && b && a.parent_pid === manifest.parent_pid && b.parent_pid === manifest.parent_pid && a.start_ticks === b.start_ticks && a.executable === b.executable && a.executable === manifest.executable)
		  return true;
	}

	return false;
}

function summary(rows) {
  const groups = new Map();

	for (const row of rows) {
	  const key = `${row.role}:${row.child}:${row.stage}`;

		const value = groups.get(key) ?? {
		  role: row.role,
		  child: row.child,
		  stage: row.stage,
		  rows: 0,
		  calls: 0,
		  wall_ns: 0,
		  valid_cpu_rows: 0,
		  missing_cpu_rows: 0,
		  thread_cpu_ns: null,
		  counter_read_wall_ns: 0
		};

		value.rows++;
		value.calls = add(value.calls, row.calls);
		value.wall_ns = add(value.wall_ns, row.wall_ns);
		value.counter_read_wall_ns = add(value.counter_read_wall_ns, row.counter_read_wall_ns);

		if (row.calls > 0) {
		  if (row.thread_cpu_ns === null)
			  value.missing_cpu_rows++;
			else {
			  value.valid_cpu_rows++;
				value.thread_cpu_ns = add(value.thread_cpu_ns ?? 0, row.thread_cpu_ns);
			}
		}

		groups.set(key, value);
	}

	return [...groups.values()].map(entry => ({
	  ...entry,
	  thread_cpu_ns: entry.missing_cpu_rows ? null : entry.thread_cpu_ns
	}));
}

function scaled(value, factor = 1000000) {
  if (value === undefined || value === null)
	  return null;

	require_(typeof value === "number" && Number.isFinite(value) && value >= 0, "invalid workload measurement");
	return uint(Math.floor(value * factor));
}

export function workloadSummary(result, samples = [], state = {}) {
  const complete = result === undefined ? 0 : 1;

	if (complete) require_(
	  result.case_id === "sixteen_universe" && result.execution_mode === "unrestricted" && result.fixture_count === 576 && result.requested_rate_hz === 40 && result.measurement_seconds === 60,
		"workload differs from existing576 case"
	);

	const first = samples[0]?.diagnostics?.output;
	const last = samples.at(-1)?.diagnostics?.output;

	const delta = key => {
	  if (first?.[key] === undefined || last?.[key] === undefined)
		  return null;

		const before = uint(first[key]), after = uint(last[key]);
		return after < before ? null : after - before;
	};

	const minimum = scaled(result?.minimum_one_second_completed_hz);
	const deadline = delta("deadline_misses"), errors = delta("send_errors");
	const resources = result?.resources;
	const bounds = result?.frame_rate_band_bounds_hz ?? [];
	const bands = result?.frame_rate_band_counts ?? [];

	require_(
	  Array.isArray(bounds) && Array.isArray(bands) && bounds.length === bands.length && [0, 34].includes(bounds.length),
		"invalid workload rate bands"
	);

	const component = (role, source) => ({
	  role,
	  cpu_average_percent_microunits: scaled(source?.cpu_average_percent),
	  cpu_max_percent_microunits: scaled(source?.cpu_max_percent),
	  peak_resident_bytes: source?.peak_resident_bytes === undefined ? null : uint(source.peak_resident_bytes)
	});

	return {
	  case: 1,
	  execution_mode: 0,
	  expected_fixture_records: 576,
	  observed_fixture_records: result?.fixture_count === undefined ? null : uint(result.fixture_count),
	  physical_instance_count: result?.physical_instance_count === undefined ? null : uint(result.physical_instance_count),
	  parameter_count: result?.parameter_count === undefined ? null : uint(result.parameter_count),
	  universe_count: result?.universes === undefined ? null : uint(result.universes),
	  animated_attribute_count: result?.animated_attribute_count === undefined ? null : uint(result.animated_attribute_count),
	  master_lane_count: result?.master_lane_count === undefined ? null : uint(result.master_lane_count),
	  requested_rate_microhz: 40000000,
	  minimum_rate_gate_microhz: 38000000,
	  configured_poll_iterations: 60,
	  observed_poll_iterations: samples.filter(entry => entry.phase === "measurement").length,
	  workload_complete: complete,
	  backend_ready: state.backendReady === true ? 1 : 0,
	  show_prepared: state.prepared === true ? 1 : 0,
	  fixture_sheet_liveness_gate_met: complete ? 1 : null,
	  minimum_one_second_completed_microhz: minimum,
	  average_completed_microhz: scaled(result?.average_completed_hz),
	  p95_one_second_completed_microhz: scaled(result?.p95_one_second_completed_hz),
	  maximum_one_second_completed_microhz: scaled(result?.maximum_one_second_completed_hz),
	  frame_rate_band_bounds_microhz: bounds.map(value => scaled(value)),
	  frame_rate_band_counts: bands.map(value => uint(value)),
	  windows_below_38hz: result?.windows_below_target === undefined ? null : uint(result.windows_below_target, 60),
	  rate_gate_met: minimum === null ? null : minimum >= 38000000 ? 1 : 0,
	  frames_sent_delta: delta("frames_sent"),
	  output_deadline_misses_delta: deadline,
	  output_send_errors_delta: errors,
	  deadline_gate_met: deadline === null ? null : deadline === 0 ? 1 : 0,
	  send_error_gate_met: errors === null ? null : errors === 0 ? 1 : 0,
	  mutation_measurement_available: 0,
	  mutation_budget_ns: 5000000,
	  mutation_p95_ns: null,
	  mutation_gate_met: null,
	  application_cpu_average_percent_microunits: scaled(resources?.application_cpu_average_percent),
	  application_cpu_p95_percent_microunits: scaled(resources?.application_cpu_p95_percent),
	  application_cpu_max_percent_microunits: scaled(resources?.application_cpu_max_percent),
	  application_peak_resident_bytes: resources?.application_peak_resident_bytes === undefined ? null : uint(resources.application_peak_resident_bytes),

	  process_resources: [
		  component(1, resources?.server),
		  component(2, resources?.native_renderer),
		  component(3, resources?.desktop_webview),
		  component(4, resources?.desktop_host)
		]
	};
}

export function numericReport(
  {
	  mode,
	  requests,
	  native,
	  server,
	  receipts,
	  complete = 1,
	  observerWallNs = 0,
	  workloadResult,
	  workloadSamples,
	  workloadState
	}
) {
  require_(["on", "off"].includes(mode), "invalid diagnostic mode");
	uint(complete, 1);
	const window = qualifyWindow(requests), records = [], overhead = [], excluded = [0, 0, 0, 0, 0, 0];

	const append = (role, child, ordinal, stage, anchor, entry, accepted = null) => records.push({
	  role,
	  child,
	  sample: ordinal,
	  stage,
	  sampled_at_millis: anchor,
	  accepted,
	  ...entry
	});

	if (mode === "off")
	  require_(native.length === 0 && server.size === 0, "OFF unexpectedly emitted CPU records");

	for (const [index, capture] of native.entries()) {
	  for (const [ordinal, group] of capture.groups) {
		  const meta = group.stages.get(0).meta;
			let classification = windowClass(meta.sampled_at_millis, group.overhead.window_wall_ns, window);

			if (classification === 0 && !validateOwnedBinding(capture.binding, receipts, meta.sampled_at_millis, group.overhead.window_wall_ns))
			  classification = 5;

			if (classification !== 0) {
			  excluded[classification]++;
				continue;
			}

			for (const [stage, value] of group.stages) append(2, index + 1, ordinal, stage, meta.sampled_at_millis, {
			  ...value.tuple,
			  transport: meta.transport,
			  outcome: meta.outcome,
			  redrawn: meta.redrawn,
			  send_outcome: meta.send_outcome,
			  source_before: meta.source_before,
			  source_after: meta.source_after,
			  presented_before: meta.presented_before,
			  presented_after: meta.presented_after,
			  relative_ns: meta.relative_ns
			});

			overhead.push({
			  role: 2,
			  child: index + 1,
			  sample: ordinal,
			  sampled_at_millis: meta.sampled_at_millis,
			  export_wall_ns: group.overhead.export_wall_ns,
			  window_wall_ns: group.overhead.window_wall_ns
			});
		}
	}

	for (const [ordinal, group] of server) {
	  if (!group[0] || !group[1]) {
		  excluded[5]++;
			continue;
		}

		const major = group[0], nested = group[1];
		let classification = windowClass(major.anchor, major.tuples[0].wall_ns, window);

		if (nested.anchor === null)
		  classification = 3;
		else if (major.anchor === null || nested.anchor < major.anchor - 1 || nested.anchor > major.anchor + Math.ceil(major.tuples[0].wall_ns / 1e6) + 1)
		  classification = 5;

		const binding = receipts.find(receipt => receipt.server)?.server;

		if (classification === 0 && (!binding || !validateOwnedBinding(binding, receipts, major.anchor, major.tuples[0].wall_ns)))
		  classification = 5;

		if (classification !== 0) {
		  excluded[classification]++;
			continue;
		}

		major.tuples.forEach((entry, stage) => append(1, 0, ordinal, stage, major.anchor, entry));
		nested.tuples.forEach((entry, stage) => append(1, 0, ordinal, stage + 5, nested.anchor, entry, nested.accepted));
	}

	const report = {
	  schema: 1,
	  mode: mode === "on" ? 1 : 0,
	  complete,
	  expected_role_count: 3,
	  pane_binding_available: 0,
	  accepted_frame_binding_available: 0,
	  worker_cpu_coverage: 0,
	  runqueue_availability: 0,
	  clock_tolerance_ms: 5,
	  boundary_margin_ms: 6,
	  window,
	  observer_wall_ns: uint(observerWallNs),
	  workload: workloadSummary(workloadResult, workloadSamples, workloadState),
	  coordinator_final_export_overhead_available: 0,
	  observer_measured_within_window_only: 0,
	  records,
	  native_export_overhead: overhead,
	  excluded_outside: excluded[1],
	  excluded_boundary: excluded[2],
	  excluded_null_anchor: excluded[3],
	  excluded_clock_unbound: excluded[4],
	  excluded_identity_or_join: excluded[5],
	  summaries: summary(records)
	};

	assertNumericOnly(report);
	return report;
}

export function assertNumericOnly(value) {
  if (value === null)
	  return;

	if (typeof value === "number") {
	  uint(value);
		return;
	}

	require_(typeof value === "object" && value !== null, "public export contains a nonnumeric value");

	for (const child of Object.values(value))
	  assertNumericOnly(child);
}

async function privateText(filename, maximum = MAX_BYTES, permissions = 0o600) {
  const metadata = await lstat(filename);

	require_(
	  metadata.isFile() && !metadata.isSymbolicLink() && metadata.uid === process.getuid() && (metadata.mode & 0o7777) === permissions && metadata.nlink === 1 && metadata.size <= maximum,
		"unsafe private capture file"
	);

	const file = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW);

	try {
	  const opened = await file.stat();

		require_(
		  opened.ino === metadata.ino && opened.dev === metadata.dev && opened.size <= maximum,
			"private file changed identity"
		);

		return await file.readFile("utf8");
	} finally {
	  await file.close();
	}
}

async function drainedPrivateText(filename) {
  const before = await lstat(filename);
	await new Promise(resolve => setTimeout(resolve, 100));
	const after = await lstat(filename);

	require_(
	  before.size === after.size && before.mtimeMs === after.mtimeMs && before.ino === after.ino,
		"private stream has not drained"
	);

	return privateText(filename);
}

async function manifests(directory, pending = false) {
  const metadata = await lstat(directory);

	require_(
	  metadata.isDirectory() && !metadata.isSymbolicLink() && metadata.uid === process.getuid() && (metadata.mode & 0o7777) === 0o700,
		"unsafe private capture directory"
	);

	const names = await readdir(directory);

	require_(
	  names.length <= 4 && names.every(name => /^viz-cpu-\d+-\d+\.(?:json|stderr)$/u.test(name)),
		"unexpected capture files or helper count"
	);

	const result = [];

	for (const name of names.filter(name => name.endsWith(".json"))) {
	  const text = await privateText(path.join(directory, name), 32768);

		if (pending && !text)
		  continue;

		const value = JSON.parse(text);
		exact(value, MANIFEST);
		require_(value.private_capture_schema === 1, "unknown private capture schema");
		uint(value.child_pid);
		uint(value.parent_pid);
		sample(value.spawn_ordinal);

		require_(
		  typeof value.program === "string" && path.isAbsolute(value.program) && Array.isArray(value.arguments) && value.arguments.every(entry => typeof entry === "string"),
			"invalid private program identity"
		);

		const stem = `viz-cpu-${value.parent_pid}-${value.spawn_ordinal}`;

		require_(
		  name === `${stem}.json` && value.stderr_path === path.join(directory, `${stem}.stderr`) && names.includes(`${stem}.stderr`),
			"capture identity/path mismatch"
		);

		result.push(value);
	}

	require_(
	  new Set(result.map(entry => entry.spawn_ordinal)).size === result.length && new Set(result.map(entry => entry.child_pid)).size === result.length,
		"duplicate child or spawn ordinal"
	);

	return result.sort((a, b) => a.spawn_ordinal - b.spawn_ordinal);
}

function ancestry(entries, entry, rootPid) {
  const seen = new Set();
	let current = entry;

	while (current?.pid !== rootPid) {
	  require_(current && !seen.has(current.pid), "owned child ancestry is unavailable");
		seen.add(current.pid);
		current = entries.find(candidate => candidate.pid === current.parentPid);
	}

	require_(current, "owned root missing");
}

export class CpuDiagnosticRun {
  constructor(mode, root, outputDirectory, io = {}) {
	  this.mode = mode;
		this.root = root;
		this.outputDirectory = outputDirectory;
		this.directory = path.join(root, "native-cpu-private");
		this.environment = cpuEnvironment(mode, this.directory);
		this.requests = [];
		this.receipts = [];
		this.bindings = new Map();
		this.verifiedManifests = new Map();
		this.observerWallNs = 0;
		this.readStat = io.readStat ?? (filename => readFile(filename, "utf8"));
		this.readlink = io.readlink ?? readlink;
		this.realpath = io.realpath ?? realpath;
	}

	requestStart() {
	  return {
		  wall_start_ms: Date.now(),
		  mono_start_ns: Math.floor(performance.now() * 1e6)
		};
	}

	requestEnd(start) {
	  this.requests.push({
		  ...start,
		  wall_end_ms: Date.now(),
		  mono_end_ns: Math.floor(performance.now() * 1e6)
		});
	}

	start(rootPid) {
	  this.rootPid = uint(rootPid);
	}

	async observe(processes) {
	  const started = performance.now(), wallStart = Date.now(), owned = [];
		const candidates = processes.filter(entry => /^(?:viz-renderer|light-headless)/u.test(entry.command));

		require_(
		  candidates.filter(entry => entry.command.startsWith("viz-renderer")).length === 2 && candidates.filter(entry => entry.command.startsWith("light-headless")).length === 1,
			"expected one server and two owned renderers"
		);

		for (const entry of candidates) {
		  ancestry(processes, entry, this.rootPid);
			const executable = await this.readlink(`/proc/${entry.pid}/exe`);

			require_(
			  path.isAbsolute(executable) && !executable.endsWith(" (deleted)") && path.basename(executable).startsWith(entry.command.startsWith("viz-renderer") ? "viz-renderer" : "light-headless"),
				"actual executable identity mismatch"
			);

			const confirmed = parseLinuxProcessStat(await this.readStat(`/proc/${entry.pid}/stat`));

			require_(
			  confirmed.pid === entry.pid && confirmed.parentPid === entry.parentPid && confirmed.startTicks === entry.startTicks,
				"process changed during executable verification"
			);

			const record = {
			  pid: uint(entry.pid),
			  parent_pid: uint(entry.parentPid),
			  start_ticks: uint(entry.startTicks),
			  executable
			};

			const previous = this.bindings.get(entry.pid);
			require_(!previous || JSON.stringify(previous) === JSON.stringify(record), "owned process identity changed");
			this.bindings.set(entry.pid, record);
			owned.push(record);
		}

		require_(this.bindings.size === 3, "owned role set changed during measurement");
		const privateBindings = this.mode === "on" ? await manifests(this.directory, true) : [];

		for (const manifest of privateBindings) {
		  const child = owned.find(entry => entry.pid === manifest.child_pid);

			require_(
			  child && child.parent_pid === manifest.parent_pid && child.executable === (await this.realpath(manifest.program)),
				"manifest differs from actual owned executable/parent"
			);

			this.verifiedManifests.set(manifest.spawn_ordinal, {
			  manifest: JSON.stringify(manifest),
			  executable: child.executable
			});
		}

		const server = owned.find(entry => path.basename(entry.executable).startsWith("light-headless"));

		this.receipts.push({
		  wall_start_ms: wallStart,
		  wall_end_ms: Date.now(),
		  owned,

		  server: {
			  child_pid: server.pid,
			  parent_pid: server.parent_pid,
			  executable: server.executable
			}
		});

		this.observerWallNs = add(this.observerWallNs, Math.ceil((performance.now() - started) * 1e6));
	}

	setWorkload(result, state) {
	  this.workload = {
		  result,
		  state
		};
	}

	async retain() {
	  await writeFile(path.join(this.root, "typed-cpu-private-window.json"), JSON.stringify({
		  root_pid: this.rootPid,
		  requests: this.requests,
		  receipts: this.receipts,
		  manifest_verification: [...this.verifiedManifests]
		}), {
		  flag: "wx",
		  mode: 0o600
		});
	}

	async requireStopped() {
	  for (const identity of this.bindings.values()) {
		  try {
			  const current = parseLinuxProcessStat(await this.readStat(`/proc/${identity.pid}/stat`));
				require_(current.startTicks !== identity.start_ticks, "owned child still alive after termination");
			} catch (error) {
			  if (error.code !== "ENOENT" && error.code !== "ESRCH")
				  throw error;
			}
		}
	}

	async finish(evidence, failure) {
	  await this.requireStopped();
		const started = performance.now();
		const bindings = this.mode === "on" ? await manifests(this.directory) : [];
		require_(this.mode === "off" || bindings.length === 2, "two bound native children required");
		const native = [];

		for (const binding of bindings) {
		  const observed = this.bindings.get(binding.child_pid);
			const verified = this.verifiedManifests.get(binding.spawn_ordinal);

			require_(
			  observed && verified?.manifest === JSON.stringify(binding) && verified.executable === observed.executable,
				"capture child was not independently observed"
			);

			native.push({
			  binding: {
				  ...binding,
				  executable: observed.executable
				},

			  groups: parseNativeCapture(await drainedPrivateText(binding.stderr_path))
			});
		}

		const serverMetadata = await lstat(evidence.sidecarLogPath);

		require_(
		  serverMetadata.isFile() && !serverMetadata.isSymbolicLink() && serverMetadata.uid === process.getuid(),
			"server log is not owned regular evidence"
		);

		await chmod(evidence.sidecarLogPath, 0o600);
		const server = parseServerCapture(await drainedPrivateText(evidence.sidecarLogPath));

		require_(
		  this.mode === "off" || server.size > 0 && native.every(capture => capture.groups.size > 0),
			"diagnostic counter rows are missing"
		);

		const report = numericReport({
		  mode: this.mode,
		  requests: this.requests,
		  native,
		  server,
		  receipts: this.receipts,
		  complete: failure ? 0 : 1,
		  observerWallNs: add(this.observerWallNs, Math.ceil((performance.now() - started) * 1e6)),
		  workloadResult: this.workload?.result,
		  workloadSamples: evidence.samples,
		  workloadState: this.workload?.state
		});

		await writeFile(path.join(this.outputDirectory, "typed-cpu-numeric.json"), `${JSON.stringify(report)}\n`, {
		  flag: "wx",
		  mode: 0o600
		});
	}
}

export async function createCpuDiagnosticRun(mode, outputDirectory) {
  require_(["off", "on"].includes(mode), "mode must be off or on");

	await mkdir(artifactPaths.tmp, {
	  recursive: true
	});

	const root = await mkdtemp(path.join(artifactPaths.tmp, "desktop-typed-cpu-"));
	const publicRoot = await realpath(path.resolve(outputDirectory));
	const privateRoot = await realpath(root);

	require_(
	  !privateRoot.startsWith(`${publicRoot}${path.sep}`) && !publicRoot.startsWith(`${privateRoot}${path.sep}`) && privateRoot !== publicRoot,
		"private scratch overlaps public output"
	);

	await chmod(root, 0o700);
	const run = new CpuDiagnosticRun(mode, root, path.resolve(outputDirectory));

	if (mode === "on") await mkdir(run.directory, {
	  mode: 0o700
	});

	return run;
}
