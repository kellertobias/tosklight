import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import vm from "node:vm";
import { artifactPaths } from "./artifact-paths.mjs";
import { processTreeResourceDelta } from "./process-tree-resources.mjs";

const {
  CpuDiagnosticRun,
  workloadSummary,
  assertNumericOnly
} = await import(process.env.LIGHT_DESKTOP_CPU_TEST_MODULE ?? "./desktop-performance-cpu-stages.mjs");

await mkdir(artifactPaths.tmp, {
  recursive: true
});

const scratch = await mkdtemp(path.join(artifactPaths.tmp, "cpu-server-observer-test-"));

after(() => rm(scratch, {
  recursive: true,
  force: true
}));

const source = await readFile(new URL("./run-desktop-performance.mjs", import.meta.url), "utf8");

function snapshot(index = 0) {
  const entries = [{
	  pid: 100,
	  parentPid: 1,
	  startTicks: 1,
	  command: "desk"
	}, {
	  pid: 101,
	  parentPid: 100,
	  startTicks: 11,
	  command: "light-headless"
	}, {
	  pid: 103,
	  parentPid: 100,
	  startTicks: 13,
	  command: "viz-renderer"
	}];

	if (index < 4) entries.push({
	  pid: 102,
	  parentPid: 100,
	  startTicks: 12,
	  command: "viz-renderer"
	});
	else if (index >= 7) entries.push({
	  pid: 104,
	  parentPid: 100,
	  startTicks: 14,
	  command: "viz-renderer"
	});

	return entries.map(entry => ({
	  ...entry,
	  cpuTicks: index * 10,
	  residentPages: 2
	}));
}

function stat(entry) {
  const fields = Array(22).fill("0");
	fields[0] = "S";
	fields[1] = entry.parentPid;
	fields[19] = entry.startTicks;
	return `${entry.pid} (${entry.command}) ${fields.join(" ")}`;
}

function fixture(mode = "off") {
  const state = {
	  entries: snapshot(),
	  executable: "/bundle/light-headless",
	  stat: null,
	  lookups: []
	};

	const run = new CpuDiagnosticRun(mode, scratch, scratch, {
	  readlink: async filename => {
		  state.lookups.push(filename);
			return filename === "/proc/101/exe" ? state.executable : "/bundle/viz-renderer";
		},

	  readStat: async filename => {
		  const entry = state.entries.find(value => filename === `/proc/${value.pid}/stat`);

			if (!entry) {
			  const error = new Error("process ended");
				error.code = "ENOENT";
				throw error;
			}

			return state.stat ?? stat(entry);
		}
	});

	run.start(100);

	return {
	  run,
	  state
	};
}

async function measure(observe) {
  let clock = 0, reads = 0, processReads = 0;

	const {
	  run,
	  state
	} = fixture();

	const start = source.indexOf("async function measureWindow(");
	const end = source.indexOf("\nfunction apiClient(", start);

	const execute = vm.runInNewContext(`${source.slice(start, end)}; measureWindow`, {
	  DURATION_SECONDS: 60,

	  performance: {
		  now: () => clock
		},

	  setTimeout: callback => {
		  clock += 1000;
			callback();
		},

	  readLinuxProcessTree: async () => {
		  state.entries = snapshot(processReads++);
			return state.entries;
		},

	  processTreeResourceDelta,

	  LINUX_PROCESS_OPTIONS: {
		  ticksPerSecond: 100,
		  pageSize: 4096
		},

	  average: values => values.reduce((sum, value) => sum + value, 0) / values.length,
	  percentile: values => values[0]
	});

	const samples = [];

	const result = await execute({
	  request: async (method, route) => {
		  assert.equal(method, "GET");
			assert.equal(route, "/api/v2/diagnostics/performance");
			reads++;

			return {
			  output: {
				  frames_sent: clock * 40 / 1000,
				  frame_rate_band_counts: Array(34).fill(reads),
				  frame_rate_band_bounds_hz: Array(34).fill(40)
				}
			};
		}
	}, 100, "unrestricted", samples, observe ? run : undefined);

	return {
	  result,
	  samples,
	  reads,
	  processReads,
	  run,
	  state
	};
}

test("actual measureWindow preserves61 reads and60 polls across native2→1→replacement2", async () => {
  const baseline = await measure(false);
	const observed = await measure(true);
	assert.equal(observed.reads, 61);
	assert.equal(observed.processReads, 61);
	assert.equal(observed.samples.length, 61);
	assert.equal(observed.run.receipts.length, 61);
	assert.equal(observed.run.bindings.size, 1);
	assert.deepEqual(JSON.parse(JSON.stringify(observed.result)), JSON.parse(JSON.stringify(baseline.result)));
	assert.equal(observed.result.average_completed_hz, 40);
	assert.equal(observed.result.minimum_one_second_completed_hz, 40);
	assert.equal(observed.result.windows_below_target, 0);
	assert.ok(observed.state.lookups.every(filename => filename === "/proc/101/exe"));
});

test("missing, duplicate, foreign or changed headless identity always stops observation", async () => {
  for (const mutation of [
	  "missing",
	  "duplicate",
	  "pid",
	  "start",
	  "parent",
	  "foreign",
	  "executable",
	  "stat-start",
	  "stat-parent",
	  "root-start"
	]) {
	  const {
		  run,
		  state
		} = fixture();

		await run.observe(state.entries);
		const server = state.entries.find(entry => entry.pid === 101);

		if (mutation === "missing")
		  state.entries = state.entries.filter(entry => entry !== server);

		if (mutation === "duplicate") state.entries.push({
		  ...server,
		  pid: 111
		});

		if (mutation === "pid")
		  server.pid = 111;

		if (mutation === "start")
		  server.startTicks++;

		if (mutation === "parent") {
		  server.parentPid = 105;

			state.entries.push({
			  pid: 105,
			  parentPid: 100,
			  startTicks: 15,
			  command: "parent"
			});
		}

		if (mutation === "foreign")
		  server.parentPid = 999;

		if (mutation === "executable")
		  state.executable = "/foreign/unowned";

		if (mutation === "stat-start") state.stat = stat({
		  ...server,
		  startTicks: 999
		});

		if (mutation === "stat-parent") state.stat = stat({
		  ...server,
		  parentPid: 999
		});

		if (mutation === "root-start")
		  state.entries.find(entry => entry.pid === 100).startTicks++;

		await assert.rejects(run.observe(state.entries), /Typed CPU diagnostic STOP/);
		assert.ok(run.observerFailureReason > 0);
	}
});

test("an altered intermediary ancestry cannot be accepted as the same owned server", async () => {
  const {
	  run,
	  state
	} = fixture();

	state.entries.push({
	  pid: 110,
	  parentPid: 100,
	  startTicks: 110,
	  command: "parent"
	});

	state.entries.find(entry => entry.pid === 101).parentPid = 110;
	await run.observe(state.entries);
	state.entries.find(entry => entry.pid === 110).parentPid = 999;
	await assert.rejects(run.observe(state.entries), /Typed CPU diagnostic STOP/);
});

test(
  "pre-window Sheet failure exposes bounded phase and observed readiness without private strings or invented gates",
	() => {
	  const state = {
		  phase: "fixture-sheet-readiness",
		  backendReady: true,
		  prepared: true,

		  sheet: {
			  ready: false,
			  readinessTimedOut: true,
			  reportPresent: true,
			  expectedFixtureRecords: 576,
			  readyFixtureRecords: null,

			  surface: {
				  kind: "fixture-sheet-container-mounted",
				  private_path: "/private/scene"
				},

			  readiness: {
				  active: true,
				  configState: "loaded",
				  fixtureSheetEnabled: true,
				  rowCount: 415,
				  fixtureRecords: 415,
				  activeValuesLoading: true,
				  groupRuntimeLoading: false
				},

			  renderFailure: null,
			  lifecycleFailure: null,
			  readyAppendFailure: null,
			  error: "arbitrary secret",
			  pid: 1234
			}
		};

		const summary = workloadSummary(undefined, [], state);
		assert.equal(summary.phase, 3);
		assert.equal(summary.fixture_sheet.ready, 0);
		assert.equal(summary.fixture_sheet.readiness_timed_out, 1);
		assert.equal(summary.fixture_sheet.reported_fixture_records, 415);
		assert.equal(summary.fixture_sheet.ready_fixture_records, null);
		assert.equal(summary.fixture_sheet.surface_mounted, 1);
		assert.equal(summary.fixture_sheet.active_values_loading, 1);
		assert.equal(summary.fixture_sheet.group_runtime_loading, 0);
		assert.equal(summary.fixture_sheet_liveness_gate_met, null);
		assert.equal(summary.rate_gate_met, null);
		assert.equal(summary.deadline_gate_met, null);
		assert.equal(summary.send_error_gate_met, null);
		assertNumericOnly(summary);
		assert.ok(!JSON.stringify(summary).includes("private"));
		assert.ok(!JSON.stringify(summary).includes("secret"));
	}
);

test(
  "one opening snapshot is not a zero-delta observation or passing gate; partial positive failures remain failures",
	() => {
	  const opening = {
		  phase: "opening",

		  diagnostics: {
			  output: {
				  frames_sent: 10,
				  deadline_misses: 2,
				  send_errors: 3
				}
			}
		};

		const first = workloadSummary(undefined, [opening]);
		assert.equal(first.frames_sent_delta, null);
		assert.equal(first.output_deadline_misses_delta, null);
		assert.equal(first.deadline_gate_met, null);

		const second = {
		  phase: "measurement",

		  diagnostics: {
			  output: {
				  frames_sent: 50,
				  deadline_misses: 2,
				  send_errors: 4
				}
			}
		};

		const partial = workloadSummary(undefined, [opening, second]);
		assert.equal(partial.frames_sent_delta, 40);
		assert.equal(partial.deadline_gate_met, null);
		assert.equal(partial.send_error_gate_met, 0);
	}
);

test("Sheet liveness observations retain explicit failures and reject malformed safe fields", () => {
  const result = workloadSummary(undefined, [], {
	  phase: "fixture-sheet-liveness",

	  sheet: {
		  livenessPassed: false,
		  heartbeatCount: 0,
		  widestSilenceMillis: 60000,
		  toleratedSilenceMillis: 10000
		}
	});

	assert.equal(result.phase, 5);
	assert.equal(result.fixture_sheet_liveness_gate_met, 0);
	assert.equal(result.fixture_sheet.heartbeat_count, 0);
	assert.equal(result.fixture_sheet.widest_silence_ms, 60000);
	assert.equal(result.fixture_sheet.tolerated_silence_ms, 10000);

	for (const sheet of [{
	  ready: "secret"
	}, {
	  readiness: {
		  rowCount: -1
		}
	}, {
	  readiness: {
		  configState: "private-state"
		}
	}, {
	  surface: {
		  kind: "private-kind"
		}
	}]) {
	  assert.throws(() => workloadSummary(undefined, [], {
		  sheet
		}), /STOP/);
	}
});

test(
  "cleanup checks every verified observed birth after renderer replacement, without native attribution",
	async () => {
	  const {
		  run
		} = await measure(true);

		const checked = [];

		run.readStat = async filename => {
		  checked.push(filename);
			const error = new Error("gone");
			error.code = "ENOENT";
			throw error;
		};

		await run.requireStopped();
		assert.deepEqual([...new Set(checked)].sort(), [100, 101, 102, 103, 104].map(pid => `/proc/${pid}/stat`).sort());
		assert.equal(run.bindings.size, 1);
	}
);

test(
  "an observed native birth surviving termination stops even if reparented and server gone",
	async () => {
	  const {
		  run,
		  state
		} = fixture();

		await run.observe(state.entries);

		state.entries = [{
		  pid: 102,
		  parentPid: 1,
		  startTicks: 12,
		  command: "viz-renderer"
		}];

		await assert.rejects(run.requireStopped(), /still alive after termination/);
		state.entries[0].startTicks = 999;
		await run.requireStopped();
	}
);

test(
  "optional unverified ancestry is skipped for cleanup without changing server validity",
	async () => {
	  const {
		  run,
		  state
		} = fixture();

		state.entries.push({
		  pid: 999,
		  parentPid: 888,
		  startTicks: 1,
		  command: "viz-renderer"
		});

		state.entries.push({
		  pid: 777,
		  parentPid: 777,
		  startTicks: 1,
		  command: "viz-renderer"
		});

		await run.observe(state.entries);
		const checked = [];

		run.readStat = async filename => {
		  checked.push(filename);
			const error = new Error("gone");
			error.code = "ESRCH";
			throw error;
		};

		await run.requireStopped();
		assert.ok(!checked.some(filename => filename.includes("/999/") || filename.includes("/777/")));
		assert.equal(run.bindings.size, 1);
	}
);
