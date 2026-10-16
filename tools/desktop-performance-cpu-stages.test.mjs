import assert from "node:assert/strict";
import { chmod, link, lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import { artifactPaths } from "./artifact-paths.mjs";

import {
  CpuDiagnosticRun,
	assertNumericOnly,
	cpuEnvironment,
	createCpuDiagnosticRun,
	numericReport,
	parseNativeCapture,
	parseServerCapture,
	qualifyWindow,
	validateOwnedBinding,
	windowClass,
	workloadSummary,
} from "./desktop-performance-cpu-stages.mjs";

await mkdir(artifactPaths.tmp, {
  recursive: true
});

const scratch = await mkdtemp(path.join(artifactPaths.tmp, "cpu-collector-test-"));

after(() => rm(scratch, {
  recursive: true,
  force: true
}));

const phases = [
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

const family = [
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

const observation = (cpu = 5, calls = 1) => ({
  calls,
  wall_ns: calls ? 1000 : 0,
  thread_cpu_ns: calls ? cpu : null,
  runnable_queue_ns: calls ? 0 : null,
  timeslices: calls ? 0 : null,
  counter_read_wall_ns: calls ? 10 : 0
});

function nativeRows(ordinal = 1, anchor = 20500) {
  const meta = {
	  schema: 1,
	  role: 2,
	  ordinal,
	  relative_ns: 100,
	  sampled_at_millis: anchor,
	  source_before: 7,
	  source_after: 8,
	  presented_before: 2,
	  presented_after: 3,
	  transport: 1,
	  outcome: 1,
	  redrawn: 1,
	  send_outcome: 1
	};

	return [...Array.from({
	  length: 8
	}, (_, stage) => ({
	  ...meta,
	  stage,
	  calls: 1,
	  wall_ns: 1000,
	  thread_cpu_ns: 5,
	  queue_ns: null,
	  slices: null,
	  counter_read_wall_ns: 10
	})), {
	  schema: 1,
	  role: 2,
	  ordinal,
	  sampled_at_millis: anchor,
	  export_wall_ns: 20,
	  window_wall_ns: 1000000
	}];
}

const lines = rows => rows.map(row => JSON.stringify(row)).join("\n") + "\n";

function serverRows(sample = 1, anchor = 20500) {
  return [{
	  schema: 1,
	  role: 1,
	  kind: 0,
	  sample,
	  sampled_at_millis: anchor,

	  major: {
		  tick: {
			  ...observation(null),
			  wall_ns: 5000000,
			  runnable_queue_ns: null,
			  timeslices: null
			},

		  parts: {
			  dynamic: observation(),
			  engine: observation(),
			  publish: observation(),

			  send: {
				  ...observation(null),
				  runnable_queue_ns: null,
				  timeslices: null
				}
			}
		},

	  async_cpu_coverage: 0,
	  worker_cpu_coverage: 0,
	  runqueue_availability: 0
	}, {
	  schema: 1,
	  role: 1,
	  kind: 1,
	  sample,
	  generation: 2,
	  show_revision: 3,
	  sampled_at_millis: anchor === null ? null : anchor + 1,
	  accepted: 1,
	  phases: Object.fromEntries(phases.map(name => [name, observation()])),
	  family_stages: Object.fromEntries(family.map(name => [name, observation()])),
	  family_stage_calls: Array(14).fill(1),
	  family_counts: Array(22).fill(0),
	  worker_cpu_coverage: 0,
	  runqueue_availability: 0
	}];
}

function serverText(rows = serverRows()) {
  return rows.map(
	  row => `2026-10-16T13:00:00Z INFO ${row.kind === 0 ? "bounded typed output major CPU sample" : "bounded live output typed nested CPU sample"} typed_cpu_payload=${JSON.stringify(row)}`
	).join("\n") + "\n";
}

function requests() {
  return Array.from({
	  length: 61
	}, (_, index) => ({
	  wall_start_ms: 1000 + index * 1000,
	  wall_end_ms: 1010 + index * 1000,
	  mono_start_ns: index * 1000000000,
	  mono_end_ns: index * 1000000000 + 10000000
	}));
}

function receipts() {
  return Array.from({
	  length: 61
	}, (_, index) => ({
	  wall_start_ms: 1000 + index * 1000,
	  wall_end_ms: 1005 + index * 1000,

	  owned: [101, 102, 103].map(pid => ({
		  pid,
		  parent_pid: 100,
		  start_ticks: pid,
		  executable: `/bundle/${pid === 101 ? "light-headless" : "viz-renderer"}`
		})),

	  server: {
		  child_pid: 101,
		  parent_pid: 100,
		  executable: "/bundle/light-headless"
		}
	}));
}

const binding = (pid, ordinal = pid - 101) => ({
  private_capture_schema: 1,
  child_pid: pid,
  parent_pid: 100,
  spawn_ordinal: ordinal,
  program: "/bundle/viz-renderer",
  arguments: ["--embedded", "private-scene"],
  stderr_path: "/private/raw.stderr",
  executable: "/bundle/viz-renderer"
});

function reportOptions() {
  return {
	  mode: "on",
	  requests: requests(),
	  receipts: receipts(),

	  native: [102, 103].map(pid => ({
		  binding: binding(pid),
		  groups: parseNativeCapture(lines(nativeRows()))
		})),

	  server: parseServerCapture(serverText())
	};
}

test("two owned helpers with identical sample/frame numbers remain separate numeric children", () => {
  const report = numericReport(reportOptions());
	assert.equal(report.records.filter(row => row.role === 2).length, 16);
	assert.deepEqual([...new Set(report.records.filter(row => row.role === 2).map(row => row.child))], [1, 2]);
	assert.equal(report.records.filter(row => row.role === 1).length, 29);
	assert.equal(report.pane_binding_available, 0);
	assert.equal(report.accepted_frame_binding_available, 0);
	assert.equal(report.native_export_overhead.length, 2);
	assertNumericOnly(report);
	const serialized = JSON.stringify(report);

	for (const privateText of ["private-scene", "/bundle", "pid", "arguments", "stderr_path", "executable"])
	  assert.ok(!serialized.includes(privateText));
});

test("OFF explicitly scrubs inherited switches/directory and emits no CPU records", () => {
  const inherited = {
	  LIGHT_OUTPUT_PHASE_DIAGNOSTICS: "true",
	  LIGHT_OUTPUT_TYPED_CPU_DIAGNOSTICS: "1",
	  LIGHT_VIZ_TYPED_CPU_DIAGNOSTICS: "1",
	  LIGHT_VIZ_TYPED_CPU_CAPTURE_DIR: "/old/private"
	};

	const off = {
	  ...inherited,
	  ...cpuEnvironment("off")
	};

	assert.equal(off.LIGHT_OUTPUT_PHASE_DIAGNOSTICS, "0");
	assert.equal(off.LIGHT_OUTPUT_TYPED_CPU_DIAGNOSTICS, "0");
	assert.equal(off.LIGHT_VIZ_TYPED_CPU_DIAGNOSTICS, "0");
	assert.equal(off.LIGHT_VIZ_TYPED_CPU_CAPTURE_DIR, undefined);
	assert.throws(() => cpuEnvironment("on", "relative"), /absolute/);
	assert.throws(() => cpuEnvironment("maybe"), /mode/);

	const report = numericReport({
	  ...reportOptions(),
	  mode: "off",
	  native: [],
	  server: new Map()
	});

	assert.deepEqual(report.records, []);
	assert.equal(report.mode, 0);

	assert.throws(() => numericReport({
	  ...reportOptions(),
	  mode: "off"
	}), /OFF unexpectedly/);
});

test("null and not-run CPU stay absent while measured CPU zero is counted", () => {
  const options = reportOptions();
	const rows = nativeRows();
	rows[0].thread_cpu_ns = 0;
	rows[1].thread_cpu_ns = null;

	Object.assign(rows[2], {
	  calls: 0,
	  wall_ns: 0,
	  thread_cpu_ns: null,
	  queue_ns: null,
	  slices: null,
	  counter_read_wall_ns: 0
	});

	options.native[0].groups = parseNativeCapture(lines(rows));
	const report = numericReport(options), summaries = report.summaries.filter(row => row.role === 2 && row.child === 1);
	assert.equal(summaries[0].thread_cpu_ns, 0);
	assert.equal(summaries[0].valid_cpu_rows, 1);
	assert.equal(summaries[1].thread_cpu_ns, null);
	assert.equal(summaries[1].missing_cpu_rows, 1);
	assert.equal(summaries[2].thread_cpu_ns, null);
	assert.equal(summaries[2].valid_cpu_rows, 0);
	assert.equal(summaries[2].missing_cpu_rows, 0);
});

test("actual 61 request bounds retain elapsed latency and classify boundaries conservatively", () => {
  const actual = requests().map((row, index) => Object.fromEntries(
	  Object.entries(row).map(([key, value]) => [key, value + index * 13 * (key.includes("mono") ? 1e6 : 1)])
	));

	const window = qualifyWindow(actual);
	assert.equal(window.available, 1);
	assert.equal(window.closing_end_ms - window.opening_start_ms, 60790);
	assert.equal(windowClass(20500, 1000000, window), 0);
	assert.equal(windowClass(window.opening_end_ms, 0, window), 2);
	assert.equal(windowClass(window.closing_start_ms - 1, 1000000, window), 2);
	assert.equal(windowClass(1, 0, window), 1);
	assert.equal(windowClass(null, 0, window), 3);
	assert.equal(qualifyWindow(actual.slice(1)).available, 0);
});

test("clock step or null anchor produces qualified unbound records instead of fitted window", () => {
  const options = reportOptions();
	options.requests[30].wall_start_ms += 1000;
	const stepped = numericReport(options);
	assert.equal(stepped.window.available, 0);
	assert.equal(stepped.window.clock_step_detected, 1);
	assert.equal(stepped.records.length, 0);
	assert.equal(stepped.excluded_clock_unbound, 3);
	const missing = reportOptions();
	missing.native[0].groups = parseNativeCapture(lines(nativeRows(1, null)));
	const result = numericReport(missing);
	assert.equal(result.excluded_null_anchor, 1);
	assert.equal(result.records.filter(row => row.child === 1 && row.role === 2).length, 0);
});

test(
  "same owned PID requires matching executable, parent, start identity and bracketing receipts",
	() => {
	  const expected = binding(102);
		const observed = receipts();
		assert.equal(validateOwnedBinding(expected, observed, 20500, 1000000), true);

		for (const property of ["start_ticks", "parent_pid", "executable"]) {
		  const changed = structuredClone(observed);
			const entry = changed[20].owned.find(row => row.pid === 102);
			entry[property] = property === "executable" ? "/unowned/renderer" : 9999;
			assert.equal(validateOwnedBinding(expected, changed, 20500, 1000000), false);
		}

		assert.equal(validateOwnedBinding(expected, observed, 20005, 1000), false);
		assert.equal(validateOwnedBinding(expected, observed, null, 0), false);
	}
);

test(
  "native fixed schema rejects private fields, invalid enums/numerics and duplicate/conflicting/incomplete rows",
	() => {
	  for (const [key, value] of [
		  ["pid", 999],
		  ["path", "/private"],
		  ["thread_cpu_ns", -1],
		  ["thread_cpu_ns", 0.5],
		  ["thread_cpu_ns", "5"],
		  ["stage", 8],
		  ["ordinal", 0],
		  ["ordinal", 1025],
		  ["transport", 3],
		  ["outcome", true],
		  ["send_outcome", 3]
		]) {
		  const rows = nativeRows();
			rows[0][key] = value;
			assert.throws(() => parseNativeCapture(lines(rows)), /STOP|JSON/);
		}

		const duplicate = nativeRows();
		duplicate.push(duplicate[0]);
		assert.throws(() => parseNativeCapture(lines(duplicate)), /duplicate/);
		const conflict = nativeRows();
		conflict[1].source_after = 999;
		assert.throws(() => parseNativeCapture(lines(conflict)), /conflicting/);
		assert.throws(() => parseNativeCapture(lines(nativeRows().slice(1))), /incomplete/);
		const falseZero = nativeRows();
		falseZero[0].calls = 0;
		assert.throws(() => parseNativeCapture(lines(falseZero)), /not-run/);

		assert.throws(() => assertNumericOnly({
		  pid: "secret"
		}), /nonnumeric/);
	}
);

test("native exact sampler cap is accepted and excess record is rejected", () => {
  const rows = Array.from({
	  length: 1024
	}, (_, index) => nativeRows(index + 1)).flat();

	assert.equal(parseNativeCapture(lines(rows)).size, 1024);
	assert.throws(() => parseNativeCapture(lines([...rows, rows[0]])), /cap exceeded/);
});

test("server consumes Display JSON only, exact named scopes and single sample joins", () => {
  assert.equal(parseServerCapture(serverText()).size, 1);
	assert.throws(() => parseServerCapture(serverText().replace("{\"schema\"", "Object {\"schema\"")), /JSON/);

	for (const mutate of [rows => {
	  rows[0].pid = 123;
	}, rows => {
	  rows[1].phases.wait = observation();
	}, rows => {
	  rows[0].major.parts.send.thread_cpu_ns = 0;
	}, rows => {
	  rows[1].family_counts.push(0);
	}, rows => {
	  rows[1].accepted = true;
	}]) {
	  const rows = serverRows();
		mutate(rows);
		assert.throws(() => parseServerCapture(serverText(rows)), /STOP/);
	}

	assert.throws(() => parseServerCapture(serverText([serverRows()[0], serverRows()[0]])), /duplicate/);
	const options = reportOptions();
	options.server = parseServerCapture(serverText([serverRows()[0]]));
	assert.equal(numericReport(options).excluded_identity_or_join, 1);
	const nullAnchor = reportOptions();
	nullAnchor.server = parseServerCapture(serverText(serverRows(1, null)));
	assert.equal(numericReport(nullAnchor).records.filter(row => row.role === 1).length, 0);
});

function markStopped(run) {
  run.readStat = async () => {
	  const error = new Error("owned process ended");
		error.code = "ENOENT";
		throw error;
	};
}

function processes() {
  return [{
	  pid: 100,
	  parentPid: 1,
	  startTicks: 1,
	  command: "desk"
	}, {
	  pid: 101,
	  parentPid: 100,
	  startTicks: 101,
	  command: "light-headless"
	}, {
	  pid: 102,
	  parentPid: 100,
	  startTicks: 102,
	  command: "viz-renderer"
	}, {
	  pid: 103,
	  parentPid: 100,
	  startTicks: 103,
	  command: "viz-renderer"
	}];
}

function stat(entry) {
  const fields = Array(22).fill("0");
	fields[0] = "S";
	fields[1] = entry.parentPid;
	fields[19] = entry.startTicks;
	return `${entry.pid} (${entry.command}) ${fields.join(" ")}`;
}

function fakeIo(entries = processes()) {
  return {
	  readlink: async filename => `/bundle/${filename.includes("/101/") ? "light-headless" : "viz-renderer"}`,
	  realpath: async filename => filename,
	  readStat: async filename => stat(entries.find(entry => filename.includes(`/${entry.pid}/`)))
	};
}

async function fixtureRun(mode = "on") {
  const root = await mkdtemp(path.join(scratch, "run-"));
	await chmod(root, 0o700);
	const output = await mkdtemp(path.join(scratch, "public-"));
	const run = new CpuDiagnosticRun(mode, root, output, fakeIo());
	run.start(100);

	if (mode === "on") {
	  await mkdir(run.directory, {
		  mode: 0o700
		});

		for (const pid of [102, 103]) {
		  const value = binding(pid);
			delete value.executable;
			value.stderr_path = path.join(run.directory, `viz-cpu-100-${value.spawn_ordinal}.stderr`);

			await writeFile(value.stderr_path, lines(nativeRows()), {
			  mode: 0o600
			});

			await writeFile(path.join(run.directory, `viz-cpu-100-${value.spawn_ordinal}.json`), JSON.stringify(value), {
			  mode: 0o600
			});
		}
	}

	return run;
}

test(
  "capture-time observer cross-checks actual process receipts rather than trusting manifest argv",
	async () => {
	  const run = await fixtureRun();
		await run.observe(processes());
		assert.equal(run.verifiedManifests.size, 2);
		run.realpath = async () => "/other/viz-renderer";
		await assert.rejects(run.observe(processes()), /actual owned executable/);
		const outside = await fixtureRun();
		const filename = path.join(outside.directory, "viz-cpu-100-1.json");
		const raw = JSON.parse(await readFile(filename));
		raw.child_pid = 999;
		await writeFile(filename, JSON.stringify(raw));
		const lookedUp = [];

		outside.readlink = async file => {
		  lookedUp.push(file);
			return `/bundle/${file.includes("/101/") ? "light-headless" : "viz-renderer"}`;
		};

		await assert.rejects(outside.observe(processes()), /actual owned executable/);
		assert.ok(!lookedUp.some(file => file.includes("/999/")));
	}
);

test(
  "observer rejects changed parent/start identity, missing role, duplicate ordinal and unsafe capture file",
	async () => {
	  const changed = await fixtureRun();

		changed.readStat = async file => {
		  const entry = processes().find(row => file.includes(`/${row.pid}/`));

			return stat({
			  ...entry,
			  startTicks: 999
			});
		};

		await assert.rejects(changed.observe(processes()), /changed during/);
		const missing = await fixtureRun();
		await assert.rejects(missing.observe(processes().slice(0, 3)), /two owned renderers/);

		for (const kind of ["symlink", "hardlink", "mode", "private-field"]) {
		  const run = await fixtureRun();
			const filename = path.join(run.directory, "viz-cpu-100-1.json");

			if (kind === "mode")
			  await chmod(filename, 0o644);

			if (kind === "private-field") {
			  const raw = JSON.parse(await readFile(filename));
				raw.secret = "private";
				await writeFile(filename, JSON.stringify(raw));
			}

			if (kind === "symlink") {
			  const saved = path.join(run.root, "saved");

				await writeFile(saved, await readFile(filename), {
				  mode: 0o600
				});

				await rm(filename);
				await symlink(saved, filename);
			}

			if (kind === "hardlink")
			  await link(filename, path.join(run.root, "other"));

			await assert.rejects(run.observe(processes()), /unsafe|unknown/);
		}
	}
);

test(
  "final export consumes validated private streams after source executable disappears; no raw public files",
	async () => {
	  const run = await fixtureRun();
		await run.observe(processes());
		markStopped(run);

		run.realpath = async () => {
		  throw new Error("AppImage already unmounted");
		};

		run.requests = requests();
		run.receipts = receipts();
		const sidecarLogPath = path.join(run.root, "server.log");
		await writeFile(sidecarLogPath, serverText());

		await run.finish({
		  sidecarLogPath
		}, undefined);

		assert.deepEqual(await readdir(run.outputDirectory), ["typed-cpu-numeric.json"]);
		const report = JSON.parse(await readFile(path.join(run.outputDirectory, "typed-cpu-numeric.json")));
		assert.equal(report.records.length, 45);
		assert.equal(report.complete, 1);
		assertNumericOnly(report);

		await assert.rejects(run.finish({
		  sidecarLogPath
		}, undefined), /EEXIST/);
	}
);

test(
  "failed workload exports complete0 and invalid capture never creates a successful numeric report",
	async () => {
	  const run = await fixtureRun();
		await run.observe(processes());
		markStopped(run);
		run.requests = requests();
		run.receipts = receipts();
		const sidecarLogPath = path.join(run.root, "server.log");
		await writeFile(sidecarLogPath, serverText());

		await run.finish({
		  sidecarLogPath
		}, new Error("original gate failure"));

		assert.equal(JSON.parse(await readFile(path.join(run.outputDirectory, "typed-cpu-numeric.json"))).complete, 0);
		const invalid = await fixtureRun();
		await invalid.observe(processes());
		markStopped(invalid);
		const filename = path.join(invalid.directory, "viz-cpu-100-1.stderr");
		await writeFile(filename, lines(nativeRows().slice(1)));

		await assert.rejects(invalid.finish({
		  sidecarLogPath
		}, undefined), /incomplete/);

		assert.deepEqual(await readdir(invalid.outputDirectory), []);
	}
);

test(
  "private run root and capture directory remain outside public output; OFF creates no native capture",
	async () => {
	  const out = await mkdtemp(path.join(scratch, "output-"));

		for (const mode of ["off", "on"]) {
		  const run = await createCpuDiagnosticRun(mode, out);

			try {
			  assert.ok(!run.root.startsWith(`${out}${path.sep}`));
				assert.equal((await lstat(run.root)).mode & 0o7777, 0o700);
				assert.equal((await readdir(run.root)).includes("native-cpu-private"), mode === "on");
			} finally {
			  await rm(run.root, {
				  recursive: true,
				  force: true
				});
			}
		}
	}
);

test("export refuses an observed child that survived owned termination", async () => {
  const run = await fixtureRun();
	await run.observe(processes());
	await assert.rejects(run.finish({}, undefined), /still alive after termination/);
	assert.deepEqual(await readdir(run.outputDirectory), []);
});

test(
  "known ANSI SGR production logger envelope parses JSON; other control protocols are refused",
	() => {
	  const text = serverText().replaceAll("INFO", "\u001b[32mINFO\u001b[0m").replaceAll("typed_cpu_payload=", "\u001b[3mtyped_cpu_payload\u001b[0m\u001b[2m=\u001b[0m");
		assert.equal(parseServerCapture(text).size, 1);
		assert.throws(() => parseServerCapture("\u001b[2J" + text), /control escape/);
	}
);

test(
  "public workload summary preserves original scaled rates and counter deltas without claiming mutation evidence",
	() => {
	  const result = {
		  case_id: "sixteen_universe",
		  execution_mode: "unrestricted",
		  fixture_count: 576,
		  requested_rate_hz: 40,
		  measurement_seconds: 60,
		  minimum_one_second_completed_hz: 37.9999,
		  average_completed_hz: 39.8,
		  p95_one_second_completed_hz: 38.5,
		  maximum_one_second_completed_hz: 40.1,
		  windows_below_target: 1,
		  ignored_private_error: "secret"
		};

		const samples = [{
		  phase: "opening",

		  diagnostics: {
			  output: {
				  frames_sent: 100,
				  deadline_misses: 8,
				  send_errors: 4
				}
			}
		}, {
		  phase: "measurement",

		  diagnostics: {
			  output: {
				  frames_sent: 2480,
				  deadline_misses: 9,
				  send_errors: 4
				}
			}
		}];

		const summary = workloadSummary(result, samples, {
		  backendReady: true,
		  prepared: true
		});

		assert.equal(summary.workload_complete, 1);
		assert.equal(summary.rate_gate_met, 0);
		assert.equal(summary.minimum_one_second_completed_microhz, 37999900);
		assert.equal(summary.average_completed_microhz, 39800000);
		assert.equal(summary.frames_sent_delta, 2380);
		assert.equal(summary.output_deadline_misses_delta, 1);
		assert.equal(summary.output_send_errors_delta, 0);
		assert.equal(summary.deadline_gate_met, 0);
		assert.equal(summary.send_error_gate_met, 1);
		assert.equal(summary.mutation_measurement_available, 0);
		assert.equal(summary.mutation_p95_ns, null);
		assert.equal(summary.mutation_gate_met, null);
		assert.equal(summary.mutation_budget_ns, 5000000);
		assert.ok(!JSON.stringify(summary).includes("secret"));
		assertNumericOnly(summary);

		assert.throws(() => workloadSummary({
		  ...result,
		  fixture_count: 575
		}, samples), /existing576/);

		assert.throws(() => workloadSummary({
		  ...result,
		  average_completed_hz: Infinity
		}, samples), /invalid workload/);

		assert.equal(workloadSummary(undefined).rate_gate_met, null);
		const reset = structuredClone(samples);
		reset[1].diagnostics.output.send_errors = 0;
		assert.equal(workloadSummary(result, reset).output_send_errors_delta, null);
	}
);

test("cumulative small clock drift is unbound and public monotonic bounds are exact integers", () => {
  const clocks = requests();

	clocks.forEach((clock, index) => {
	  clock.wall_start_ms += index;
		clock.wall_end_ms += index;
	});

	assert.equal(qualifyWindow(clocks).available, 0);
	const stable = qualifyWindow(requests());
	assert.equal(stable.opening_monotonic_start_ns, 0);
	assert.equal(stable.closing_monotonic_end_ns, 60010000000);
});

test("server known event message must match the exact payload kind", () => {
  const major = serverText([serverRows()[0]]).replace("bounded typed output major CPU sample", "bounded live output typed nested CPU sample");
	const nested = serverText([serverRows()[1]]).replace("bounded live output typed nested CPU sample", "bounded typed output major CPU sample");
	assert.throws(() => parseServerCapture(major), /does not match kind/);
	assert.throws(() => parseServerCapture(nested), /does not match kind/);
});

test("public workload keeps original resource maxima, throughput histogram and show dimensions", () => {
  const component = {
	  cpu_average_percent: 12.5,
	  cpu_max_percent: 17,
	  peak_resident_bytes: 4096
	};

	const result = {
	  case_id: "sixteen_universe",
	  execution_mode: "unrestricted",
	  fixture_count: 576,
	  requested_rate_hz: 40,
	  measurement_seconds: 60,
	  physical_instance_count: 576,
	  parameter_count: 2304,
	  universes: 16,
	  animated_attribute_count: 1152,
	  master_lane_count: 1,
	  frame_rate_band_bounds_hz: Array(34).fill(40),
	  frame_rate_band_counts: Array(34).fill(7),

	  resources: {
		  application_cpu_average_percent: 80,
		  application_cpu_max_percent: 100,
		  application_cpu_p95_percent: 90,
		  application_peak_resident_bytes: 16384,
		  server: component,
		  native_renderer: component,
		  desktop_webview: component,
		  desktop_host: component
		}
	};

	const summary = workloadSummary(result);
	assert.equal(summary.application_cpu_max_percent_microunits, 100000000);
	assert.equal(summary.application_cpu_average_percent_microunits, 80000000);
	assert.equal(summary.application_cpu_p95_percent_microunits, 90000000);
	assert.equal(summary.application_peak_resident_bytes, 16384);
	assert.equal(summary.physical_instance_count, 576);
	assert.equal(summary.parameter_count, 2304);
	assert.equal(summary.universe_count, 16);
	assert.equal(summary.animated_attribute_count, 1152);
	assert.equal(summary.master_lane_count, 1);
	assert.deepEqual(summary.frame_rate_band_bounds_microhz, Array(34).fill(40000000));
	assert.deepEqual(summary.frame_rate_band_counts, Array(34).fill(7));

	for (const row of summary.process_resources) {
	  assert.equal(row.cpu_average_percent_microunits, 12500000);
		assert.equal(row.cpu_max_percent_microunits, 17000000);
		assert.equal(row.peak_resident_bytes, 4096);
	}

	assert.throws(() => workloadSummary({
	  ...result,
	  frame_rate_band_counts: [1]
	}), /rate bands/);

	assert.throws(() => workloadSummary({
	  ...result,
	  frame_rate_band_counts: Array(34).fill(-1)
	}), /unsigned/);
});

test("bounded drain rejects streams still changing and retains private evidence", async () => {
  const run = await fixtureRun();
	await run.observe(processes());
	markStopped(run);
	const sidecarLogPath = path.join(run.root, "server.log");
	await writeFile(sidecarLogPath, serverText());
	const filename = path.join(run.directory, "viz-cpu-100-1.stderr");

	const timer = setInterval(() => {
	  writeFile(filename, lines(nativeRows()) + "growing private stderr\n").catch(() => {});
	}, 5);

	try {
	  await assert.rejects(run.finish({
		  sidecarLogPath
		}, undefined), /has not drained/);
	} finally {
	  clearInterval(timer);
	}

	assert.deepEqual(await readdir(run.outputDirectory), []);
	assert.ok((await readFile(filename)).length > 0);
});
