import assert from "node:assert/strict";
import { chmod, link, mkdir, mkdtemp, readFile, rm, symlink, truncate, writeFile } from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import vm from "node:vm";
import { artifactPaths } from "./artifact-paths.mjs";
import { processTreeResourceDelta } from "./process-tree-resources.mjs";

const { CpuDiagnosticRun, workloadSummary, assertNumericOnly, validateOwnedBinding } = await import(
  process.env.LIGHT_DESKTOP_CPU_TEST_MODULE ?? "./desktop-performance-cpu-stages.mjs"
);

await mkdir(artifactPaths.tmp, {
  recursive: true,
});

const scratch = await mkdtemp(path.join(artifactPaths.tmp, "cpu-server-observer-test-"));

after(() =>
  rm(scratch, {
    recursive: true,
    force: true,
  }),
);

const source = await readFile(new URL("./run-desktop-performance.mjs", import.meta.url), "utf8");

function snapshot(index = 0) {
  const entries = [
    {
      pid: 100,
      parentPid: 1,
      startTicks: 1,
      command: "desk",
    },
    {
      pid: 101,
      parentPid: 100,
      startTicks: 11,
      command: "light-headless",
    },
    {
      pid: 103,
      parentPid: 100,
      startTicks: 13,
      command: "viz-renderer",
    },
  ];

  if (index < 4)
    entries.push({
      pid: 102,
      parentPid: 100,
      startTicks: 12,
      command: "viz-renderer",
    });
  else if (index >= 7)
    entries.push({
      pid: 104,
      parentPid: 100,
      startTicks: 14,
      command: "viz-renderer",
    });

  return entries.map((entry) => ({
    ...entry,
    cpuTicks: index * 10,
    residentPages: 2,
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
    lookups: [],
  };

  const run = new CpuDiagnosticRun(mode, scratch, scratch, {
    readlink: async (filename) => {
      state.lookups.push(filename);
      return filename === "/proc/101/exe" ? state.executable : "/bundle/viz-renderer";
    },

    readStat: async (filename) => {
      const entry = state.entries.find((value) => filename === `/proc/${value.pid}/stat`);

      if (!entry) {
        const error = new Error("process ended");
        error.code = "ENOENT";
        throw error;
      }

      return state.stat ?? stat(entry);
    },
  });

  run.start(100);

  return {
    run,
    state,
  };
}

async function measure(observe, mode = "off") {
  let clock = 0,
    reads = 0,
    processReads = 0;

  const { run, state } = fixture(mode);

  const start = source.indexOf("async function measureWindow(");
  const end = source.indexOf("\nfunction apiClient(", start);

  const execute = vm.runInNewContext(`${source.slice(start, end)}; measureWindow`, {
    DURATION_SECONDS: 60,

    performance: {
      now: () => clock,
    },

    setTimeout: (callback) => {
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
      pageSize: 4096,
    },

    average: (values) => values.reduce((sum, value) => sum + value, 0) / values.length,
    percentile: (values) => values[0],
  });

  const samples = [];

  const result = await execute(
    {
      request: async (method, route) => {
        assert.equal(method, "GET");
        assert.equal(route, "/api/v2/diagnostics/performance");
        reads++;

        return {
          output: {
            frames_sent: (clock * 40) / 1000,
            frame_rate_band_counts: Array(34).fill(reads),
            frame_rate_band_bounds_hz: Array(34).fill(40),
          },
        };
      },
    },
    100,
    "unrestricted",
    samples,
    observe ? run : undefined,
  );

  return {
    result,
    samples,
    reads,
    processReads,
    run,
    state,
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
  assert.ok(observed.state.lookups.every((filename) => filename === "/proc/101/exe"));
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
    "root-start",
  ]) {
    const { run, state } = fixture();

    await run.observe(state.entries);
    const server = state.entries.find((entry) => entry.pid === 101);

    if (mutation === "missing") state.entries = state.entries.filter((entry) => entry !== server);

    if (mutation === "duplicate")
      state.entries.push({
        ...server,
        pid: 111,
      });

    if (mutation === "pid") server.pid = 111;

    if (mutation === "start") server.startTicks++;

    if (mutation === "parent") {
      server.parentPid = 105;

      state.entries.push({
        pid: 105,
        parentPid: 100,
        startTicks: 15,
        command: "parent",
      });
    }

    if (mutation === "foreign") server.parentPid = 999;

    if (mutation === "executable") state.executable = "/foreign/unowned";

    if (mutation === "stat-start")
      state.stat = stat({
        ...server,
        startTicks: 999,
      });

    if (mutation === "stat-parent")
      state.stat = stat({
        ...server,
        parentPid: 999,
      });

    if (mutation === "root-start") state.entries.find((entry) => entry.pid === 100).startTicks++;

    await assert.rejects(run.observe(state.entries), /Typed CPU diagnostic STOP/);
    assert.ok(run.observerFailureReason > 0);
  }
});

test("an altered intermediary ancestry cannot be accepted as the same owned server", async () => {
  const { run, state } = fixture();

  state.entries.push({
    pid: 110,
    parentPid: 100,
    startTicks: 110,
    command: "parent",
  });

  state.entries.find((entry) => entry.pid === 101).parentPid = 110;
  await run.observe(state.entries);
  state.entries.find((entry) => entry.pid === 110).parentPid = 999;
  await assert.rejects(run.observe(state.entries), /Typed CPU diagnostic STOP/);
});

test("pre-window Sheet failure exposes bounded phase and observed readiness without private strings or invented gates", () => {
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
        private_path: "/private/scene",
      },

      readiness: {
        active: true,
        configState: "loaded",
        fixtureSheetEnabled: true,
        rowCount: 415,
        fixtureRecords: 415,
        activeValuesLoading: true,
        groupRuntimeLoading: false,
      },

      renderFailure: null,
      lifecycleFailure: null,
      readyAppendFailure: null,
      error: "arbitrary secret",
      pid: 1234,
    },
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
});

test("one opening snapshot is not a zero-delta observation or passing gate; partial positive failures remain failures", () => {
  const opening = {
    phase: "opening",

    diagnostics: {
      output: {
        frames_sent: 10,
        deadline_misses: 2,
        send_errors: 3,
      },
    },
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
        send_errors: 4,
      },
    },
  };

  const partial = workloadSummary(undefined, [opening, second]);
  assert.equal(partial.frames_sent_delta, 40);
  assert.equal(partial.deadline_gate_met, null);
  assert.equal(partial.send_error_gate_met, 0);
});

test("Sheet liveness observations retain explicit failures and reject malformed safe fields", () => {
  const result = workloadSummary(undefined, [], {
    phase: "fixture-sheet-liveness",

    sheet: {
      livenessPassed: false,
      heartbeatCount: 0,
      widestSilenceMillis: 60000,
      toleratedSilenceMillis: 10000,
    },
  });

  assert.equal(result.phase, 5);
  assert.equal(result.fixture_sheet_liveness_gate_met, 0);
  assert.equal(result.fixture_sheet.heartbeat_count, 0);
  assert.equal(result.fixture_sheet.widest_silence_ms, 60000);
  assert.equal(result.fixture_sheet.tolerated_silence_ms, 10000);

  for (const sheet of [
    {
      ready: "secret",
    },
    {
      readiness: {
        rowCount: -1,
      },
    },
    {
      readiness: {
        configState: "private-state",
      },
    },
    {
      surface: {
        kind: "private-kind",
      },
    },
  ]) {
    assert.throws(
      () =>
        workloadSummary(undefined, [], {
          sheet,
        }),
      /STOP/,
    );
  }
});

test("cleanup checks every verified observed birth after renderer replacement, without native attribution", async () => {
  const { run } = await measure(true);

  const checked = [];

  run.readStat = async (filename) => {
    checked.push(filename);
    const error = new Error("gone");
    error.code = "ENOENT";
    throw error;
  };

  await run.requireStopped();
  assert.deepEqual([...new Set(checked)].sort(), [100, 101, 102, 103, 104].map((pid) => `/proc/${pid}/stat`).sort());
  assert.equal(run.bindings.size, 1);
});

test("an observed native birth surviving termination stops even if reparented and server gone", async () => {
  const { run, state } = fixture();

  await run.observe(state.entries);

  state.entries = [
    {
      pid: 102,
      parentPid: 1,
      startTicks: 12,
      command: "viz-renderer",
    },
  ];

  await assert.rejects(run.requireStopped(), /still alive after termination/);
  state.entries[0].startTicks = 999;
  await run.requireStopped();
});

test("optional unverified ancestry is skipped for cleanup without changing server validity", async () => {
  const { run, state } = fixture();

  state.entries.push({
    pid: 999,
    parentPid: 888,
    startTicks: 1,
    command: "viz-renderer",
  });

  state.entries.push({
    pid: 777,
    parentPid: 777,
    startTicks: 1,
    command: "viz-renderer",
  });

  await run.observe(state.entries);
  const checked = [];

  run.readStat = async (filename) => {
    checked.push(filename);
    const error = new Error("gone");
    error.code = "ESRCH";
    throw error;
  };

  await run.requireStopped();
  assert.ok(!checked.some((filename) => filename.includes("/999/") || filename.includes("/777/")));
  assert.equal(run.bindings.size, 1);
});

test("ON actual measureWindow binds native generations across2→1→replacement2 without changing polling", async () => {
  const baseline = await measure(false);
  const observed = await measure(true, "on");
  assert.equal(observed.reads, 61);
  assert.equal(observed.processReads, 61);
  assert.deepEqual(JSON.parse(JSON.stringify(observed.result)), JSON.parse(JSON.stringify(baseline.result)));
  assert.equal(observed.run.nativeGenerations.size, 3);
  assert.deepEqual(
    observed.run.receipts.slice(0, 8).map((row) => row.owned.length),
    [3, 3, 3, 3, 2, 2, 2, 3],
  );
});

test("native verification gaps retain stable server evidence and never follow a foreign manifest PID", async () => {
  const { run, state } = fixture("on");
  await run.observe(state.entries);
  const readlink = run.readlink;
  run.readlink = async (filename) => {
    if (filename === "/proc/102/exe") throw Object.assign(new Error("ended"), { code: "ENOENT" });
    return readlink(filename);
  };
  await run.observe(state.entries);
  assert.equal(run.observerFailureReason, 0);
  assert.ok(run.nativeUnverified > 0);
  assert.deepEqual(
    run.receipts.at(-1).owned.map((row) => row.pid),
    [101, 103],
  );
  assert.ok(!state.lookups.some((filename) => filename.includes("/999/")));
});

test("native sample binding cannot use another birth with a reused PID", () => {
  const owned = [{ pid: 102, parent_pid: 100, start_ticks: 14, executable: "/bundle/viz-renderer" }];
  const receipts = [
    { wall_end_ms: 10, owned },
    { wall_start_ms: 50, owned },
  ];
  const manifest = { child_pid: 102, parent_pid: 100, start_ticks: 12, executable: "/bundle/viz-renderer" };
  assert.equal(validateOwnedBinding(manifest, receipts, 30, 1000000), false);
  assert.equal(validateOwnedBinding({ ...manifest, start_ticks: 14 }, receipts, 30, 1000000), true);
});

function nativeSample(anchor, cpu = 5) {
  const meta = {
    schema: 1,
    role: 2,
    ordinal: 1,
    relative_ns: 100,
    sampled_at_millis: anchor,
    source_before: 1,
    source_after: 2,
    presented_before: 0,
    presented_after: 1,
    transport: 1,
    outcome: 1,
    redrawn: 1,
    send_outcome: 1,
  };
  return (
    [
      ...Array.from({ length: 8 }, (_, stage) => ({
        ...meta,
        stage,
        calls: 1,
        wall_ns: 1000,
        thread_cpu_ns: cpu,
        queue_ns: null,
        slices: null,
        counter_read_wall_ns: 10,
      })),
      { schema: 1, role: 2, ordinal: 1, sampled_at_millis: anchor, export_wall_ns: 20, window_wall_ns: 1000000 },
    ]
      .map((row) => JSON.stringify(row))
      .join("\n") + "\n"
  );
}

function observedServerText(anchor = 20500) {
  const tuple = {
    calls: 1,
    wall_ns: 1000,
    thread_cpu_ns: 5,
    runnable_queue_ns: null,
    timeslices: null,
    counter_read_wall_ns: 10,
    process_cpu_ns: null,
    process_cpu_tick_hz: null,
    process_cpu_resolution_ns: null,
  };
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
    "reconciliation",
  ];
  const families = [
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
    "native_projection",
  ];
  const asyncTuple = { ...tuple, thread_cpu_ns: null };
  const rows = [
    {
      schema: 1,
      role: 1,
      kind: 0,
      sample: 1,
      sampled_at_millis: anchor,
      major: {
        tick: { ...asyncTuple, wall_ns: 5000000 },
        parts: {
          dynamic: tuple,
          engine: tuple,
          publish: tuple,
          send: asyncTuple,
        },
      },
      async_cpu_coverage: 0,
      worker_cpu_coverage: 0,
      runqueue_availability: 0,
    },
    {
      schema: 1,
      role: 1,
      kind: 1,
      sample: 1,
      generation: 2,
      show_revision: 3,
      sampled_at_millis: anchor + 1,
      accepted: 1,
      phases: Object.fromEntries(phases.map((name) => [name, tuple])),
      family_stages: Object.fromEntries(families.map((name) => [name, tuple])),
      family_stage_calls: Array(14).fill(1),
      family_counts: Array(22).fill(0),
      worker_cpu_coverage: 0,
      runqueue_availability: 0,
    },
  ];
  return (
    rows
      .map(
        (row) =>
          `INFO ${row.kind === 0 ? "bounded typed output major CPU sample" : "bounded live output typed nested CPU sample"} typed_cpu_payload=${JSON.stringify(row)}`,
      )
      .join("\n") + "\n"
  );
}

async function nativeRun() {
  const { run, state } = fixture("on");
  run.root = await mkdtemp(path.join(scratch, "generation-"));
  await chmod(run.root, 0o700);
  run.directory = path.join(run.root, "native-cpu-private");
  run.outputDirectory = path.join(run.root, "public");
  await mkdir(run.directory, { mode: 0o700 });
  await mkdir(run.outputDirectory);
  for (const index of [0, 4, 7]) {
    state.entries = snapshot(index);
    await run.observe(state.entries);
  }
  const server = run.bindings.get(101);
  run.requests = Array.from({ length: 61 }, (_, index) => ({
    wall_start_ms: 1000 + index * 1000,
    wall_end_ms: 1010 + index * 1000,
    mono_start_ns: index * 1000000000,
    mono_end_ns: index * 1000000000 + 10000000,
  }));
  run.receipts = run.requests.map((request, index) => ({
    wall_start_ms: request.wall_start_ms,
    wall_end_ms: request.wall_start_ms + 5,
    owned: [
      server,
      ...snapshot(index)
        .filter((entry) => entry.command === "viz-renderer")
        .map((entry) => run.nativeGenerations.get(`${entry.pid}:${entry.startTicks}`)),
    ],
    server: { child_pid: 101, parent_pid: 100, executable: server.executable },
  }));
  const manifests = [];
  for (const [pid, anchor] of [
    [102, 2500],
    [103, 20500],
    [104, 8500],
  ]) {
    const stem = `viz-cpu-100-${pid - 101}`;
    const filename = path.join(run.directory, `${stem}.json`);
    const manifest = {
      private_capture_schema: 1,
      child_pid: pid,
      parent_pid: 100,
      spawn_ordinal: pid - 101,
      program: "/bundle/viz-renderer",
      arguments: ["--embed"],
      stderr_path: path.join(run.directory, `${stem}.stderr`),
    };
    await writeFile(filename, JSON.stringify(manifest), { mode: 0o600 });
    await writeFile(manifest.stderr_path, nativeSample(anchor), { mode: 0o600 });
    manifests.push({ filename, manifest });
  }
  run.setWorkload(
    {
      case_id: "sixteen_universe",
      execution_mode: "unrestricted",
      fixture_count: 576,
      requested_rate_hz: 40,
      measurement_seconds: 60,
      minimum_one_second_completed_hz: 37,
      windows_below_target: 1,
    },
    { phase: "complete", backendReady: true, prepared: true },
  );
  const log = path.join(run.root, "server.log");
  await writeFile(log, observedServerText(), { mode: 0o600 });
  run.readStat = async () => {
    throw Object.assign(new Error("old birth gone"), { code: "ENOENT" });
  };
  run.readlink = async () => {
    throw new Error("must not follow postmortem manifest PIDs");
  };
  return { run, manifests, evidence: { sidecarLogPath: log } };
}

async function finishNative(fixture) {
  // Original workload failure stays with the caller; diagnostic finalization never replaces it.
  await fixture.run.finish(fixture.evidence, new Error("original38Hzfailure"));
  const report = JSON.parse(await readFile(path.join(fixture.run.outputDirectory, "typed-cpu-numeric.json")));
  assertNumericOnly(report);
  assert.equal(report.complete, 0);
  assert.equal(report.capture_partial, 1);
  assert.equal(report.native_capture_validated, 0);
  assert.equal(report.server_capture_validated, 1);
  assert.equal(report.native_lifetime_coverage_available, 0);
  assert.equal(report.native_thread_cpu_ns, null);
  assert.equal(report.workload.rate_gate_met, 0);
  for (const secret of ["/bundle", "private", "child_pid", "start_ticks", "arguments", "stderr_path"])
    assert.ok(!JSON.stringify(report).includes(secret));
  return report;
}

test("retired and replacement native generations export only validated samples, with unknown lifetime still partial", async () => {
  const value = await nativeRun();
  const report = await finishNative(value);
  assert.equal(report.native_capture_available, 1);
  assert.equal(report.native_samples_validated, 3);
  assert.equal(report.native_generation_count, 3);
  assert.equal(report.native_invalid_captures, 0);
  assert.equal(report.records.filter((row) => row.role === 2).length, 24);
  assert.ok(report.summaries.filter((row) => row.role === 2).every((row) => row.thread_cpu_ns === 5));
});

test("native sample cannot bridge an intervening absence or relabel an unobserved manifest birth", async () => {
  const value = await nativeRun();
  value.run.receipts[19].owned = value.run.receipts[19].owned.filter((entry) => entry.pid !== 103);
  value.manifests[2].manifest.child_pid = 999;
  await writeFile(value.manifests[2].filename, JSON.stringify(value.manifests[2].manifest));
  const report = await finishNative(value);
  assert.equal(report.native_samples_validated, 1);
  assert.equal(report.native_invalid_captures, 1);
  assert.ok(report.excluded_identity_or_join > 0);
});

test("same PID with multiple observed births cannot be assigned postmortem using argv or ordinal", async () => {
  const value = await nativeRun();
  const old = value.run.nativeGenerations.get("102:12");
  value.run.nativeGenerations.set("102:99", { ...old, start_ticks: 99 });
  const report = await finishNative(value);
  assert.equal(report.native_invalid_captures, 1);
  assert.equal(report.native_samples_validated, 2);
});

test("unknown lifetime outside-only samples are excluded without claiming complete native coverage", async () => {
  const value = await nativeRun();
  for (const { manifest } of value.manifests) await writeFile(manifest.stderr_path, nativeSample(100));
  const report = await finishNative(value);
  assert.equal(report.native_capture_available, 0);
  assert.equal(report.native_samples_validated, 0);
  assert.equal(report.excluded_outside, 3);
});

test("private native path, role, fields, links, and malformed stages never become attributed records", async () => {
  for (const kind of ["escape", "role", "field", "symlink", "hardlink", "stages"]) {
    const value = await nativeRun();
    const { filename, manifest } = value.manifests[0];
    if (kind === "escape") manifest.stderr_path = "/outside/private";
    if (kind === "role") manifest.arguments = ["--desk", "private-scene"];
    if (kind === "field") manifest.private_secret = "must-not-export";
    await writeFile(filename, JSON.stringify(manifest));
    if (kind === "symlink") {
      await rm(manifest.stderr_path);
      await symlink("/outside/private", manifest.stderr_path);
    }
    if (kind === "hardlink") await link(manifest.stderr_path, path.join(value.run.root, "other"));
    if (kind === "stages") await writeFile(manifest.stderr_path, nativeSample(2500).split("\n").slice(1).join("\n"));
    const report = await finishNative(value);
    assert.equal(report.native_invalid_captures, 1, kind);
    assert.equal(report.native_samples_validated, 2, kind);
  }
});

test("sampled native zero and unavailable CPU remain distinct and malformed private streams preserve original diagnostic failure", async () => {
  for (const cpu of [0, null]) {
    const value = await nativeRun();
    for (const { manifest } of value.manifests) await writeFile(manifest.stderr_path, nativeSample(20500, cpu));
    const report = await finishNative(value);
    const summaries = report.summaries.filter((row) => row.role === 2);
    assert.ok(summaries.length > 0);
    assert.ok(summaries.every((row) => row.thread_cpu_ns === cpu));
  }
  const value = await nativeRun();
  for (const { manifest } of value.manifests) await writeFile(manifest.stderr_path, "{malformed}");
  await assert.rejects(value.run.finish(value.evidence), /partial/);
  const report = JSON.parse(await readFile(path.join(value.run.outputDirectory, "typed-cpu-numeric.json")));
  assert.equal(report.server_capture_validated, 1);
  assert.equal(report.native_invalid_captures, 3);
  assert.equal(report.native_capture_available, 0);
});

test("actual observed PID reuse creates distinct birth receipts and never silently reuses an earlier generation", async () => {
  const { run, state } = fixture("on");
  await run.observe(state.entries);
  state.entries.find((entry) => entry.pid === 102).startTicks = 99;
  await run.observe(state.entries);
  assert.equal(run.nativeGenerations.size, 3);
  assert.equal(run.receipts[0].owned.find((entry) => entry.pid === 102).start_ticks, 12);
  assert.equal(run.receipts[1].owned.find((entry) => entry.pid === 102).start_ticks, 99);
});

test("native capture count and aggregate-byte caps preserve usable server evidence without reading oversized streams", async () => {
  for (const kind of ["count", "bytes"]) {
    const value = await nativeRun();
    if (kind === "count") {
      for (let ordinal = 4; ordinal <= 65; ordinal++) {
        await writeFile(path.join(value.run.directory, `viz-cpu-100-${ordinal}.json`), "{}", { mode: 0o600 });
        await writeFile(path.join(value.run.directory, `viz-cpu-100-${ordinal}.stderr`), "", { mode: 0o600 });
      }
    } else {
      await truncate(value.manifests[0].manifest.stderr_path, 64 * 1024 * 1024 + 1);
    }
    const report = await finishNative(value);
    assert.equal(report.server_capture_validated, 1);
    assert.equal(report.native_invalid_captures, 1);
    assert.equal(report.native_samples_validated, 0);
  }
});

test("duplicate native manifests invalidate the whole birth rather than selecting the first stream", async () => {
  const value = await nativeRun();
  const manifest = {
    ...value.manifests[0].manifest,
    spawn_ordinal: 99,
    stderr_path: path.join(value.run.directory, "viz-cpu-100-99.stderr"),
  };
  await writeFile(path.join(value.run.directory, "viz-cpu-100-99.json"), JSON.stringify(manifest), { mode: 0o600 });
  await writeFile(manifest.stderr_path, nativeSample(2500, 99), { mode: 0o600 });
  const report = await finishNative(value);
  assert.equal(report.native_invalid_captures, 1);
  assert.equal(report.native_samples_validated, 2);
  assert.ok(!report.records.some((row) => row.role === 2 && row.sampled_at_millis === 2500));
});

test("duplicate JSON manifest keys cannot bypass the fixed producer manifest shape", async () => {
  const value = await nativeRun();
  const { filename, manifest } = value.manifests[0];
  await writeFile(filename, '{"child_pid":102,' + JSON.stringify(manifest).slice(1));
  const report = await finishNative(value);
  assert.equal(report.native_invalid_captures, 1);
  assert.equal(report.native_samples_validated, 2);
});
