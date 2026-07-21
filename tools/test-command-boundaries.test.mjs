import assert from "node:assert/strict";
import test from "node:test";

import {
  CENTRALIZED_SENDER,
  baselineFor,
  evaluateTestCommandBoundaries,
  scanTestCommandBoundaries,
} from "./test-command-boundaries.mjs";

const EMPTY_BASELINE = {
  version: 2,
  directActionCommands: {},
  directActionFamilies: {},
  compatibilityCommands: {},
  compatibilityFamilies: {},
};

test("scanner ignores the centralized sender's own envelope and family declarations", () => {
  const scan = scanTestCommandBoundaries([
    {
      path: CENTRALIZED_SENDER,
      source: `
        this.command("programmer.execute", { value: command });
        this.command("programmer.command_target", { value: target });
        return { via: "compatibility", family: "cue_navigation" };
      `,
    },
  ]);
  assert.deepEqual(scan.directActionCommands, {});
  assert.deepEqual(scan.directActionFamilies, {});
  assert.deepEqual(scan.compatibilityFamilies, {});
});

test("scanner counts scenario call sites by file and family", () => {
  const scan = scanTestCommandBoundaries([
    {
      path: "tests/09-cue-go-to-load.spec.ts",
      source: `
        await api.executeCompatibilityProgrammerCommand({ family: "cue_navigation", command: "CUE 2" });
        await api.executeCompatibilityProgrammerCommand({ family: "cue_navigation", command: "CUE 3" });
      `,
    },
    {
      path: "tests/pbk006.ts",
      source:
        'await api.executeCompatibilityProgrammerCommand({ family: "speed_group", command: "SPD GRP 1 AT 90" });',
    },
  ]);
  assert.deepEqual(scan.compatibilityCommands, {
    "tests/09-cue-go-to-load.spec.ts": 2,
    "tests/pbk006.ts": 1,
  });
  assert.deepEqual(scan.compatibilityFamilies, { cue_navigation: 2, speed_group: 1 });
});

test("a retired legacy helper call fails regardless of the baseline", () => {
  const scan = scanTestCommandBoundaries([
    { path: "tests/new.spec.ts", source: "await api.executeLegacyCommandLine(\"CUE 2\");" },
  ]);
  const failures = evaluateTestCommandBoundaries(scan, baselineFor(scan));
  assert.equal(failures.length, 1);
  assert.match(failures[0], /executeLegacyCommandLine is retired/u);
});

test("a new direct literal v1 WebSocket action is rejected", () => {
  const scan = scanTestCommandBoundaries([
    {
      path: "tests/new.spec.ts",
      source: 'await api.command("programmer.command_line", { value: "GROUP 1" });',
    },
  ]);
  const failures = evaluateTestCommandBoundaries(scan, EMPTY_BASELINE);
  assert.equal(failures.length, 2);
  assert.match(failures[0], /new direct v1 WebSocket action call site: tests\/new\.spec\.ts/u);
  assert.match(failures[1], /new direct v1 WebSocket action family use: programmer\.command_line/u);
});

test("a generic typed WebSocket command is inventoried and rejected", () => {
  const scan = scanTestCommandBoundaries([
    {
      path: "tests/new.spec.ts",
      source: 'await api.command("selection.set", { fixtures: [] });',
    },
  ]);
  assert.deepEqual(scan.directActionCommands, { "tests/new.spec.ts": 1 });
  assert.deepEqual(scan.directActionFamilies, { "selection.set": 1 });
  assert.equal(evaluateTestCommandBoundaries(scan, EMPTY_BASELINE).length, 2);
});

test("scanner counts typed generic syntax and every literal action family", () => {
  const scan = scanTestCommandBoundaries([
    {
      path: "tests/actions.spec.ts",
      source: `
        await api.command<Result>("programmer.priority", { priority: 10 });
        await api.command <Other> ("preset.apply", { family: "Color", number: 1 });
      `,
    },
  ]);
  assert.deepEqual(scan.directActionCommands, { "tests/actions.spec.ts": 2 });
  assert.deepEqual(scan.directActionFamilies, {
    "preset.apply": 1,
    "programmer.priority": 1,
  });
});

test("a grown compatibility call site is rejected while the baseline count passes", () => {
  const files = [
    {
      path: "tests/cues.spec.ts",
      source: `
        await api.executeCompatibilityProgrammerCommand({ family: "cue_delete", command: "DELETE SET 1 CUE 1" });
        await api.executeCompatibilityProgrammerCommand({ family: "cue_delete", command: "DELETE SET 1 CUE 2" });
      `,
    },
  ];
  const scan = scanTestCommandBoundaries(files);
  assert.deepEqual(evaluateTestCommandBoundaries(scan, baselineFor(scan)), []);

  const baseline = baselineFor(scan);
  baseline.compatibilityCommands["tests/cues.spec.ts"] = 1;
  baseline.compatibilityFamilies.cue_delete = 1;
  const failures = evaluateTestCommandBoundaries(scan, baseline);
  assert.equal(failures.length, 2);
  assert.match(failures[0], /compatibility command call site grew: tests\/cues\.spec\.ts has 2/u);
  assert.match(failures[1], /compatibility command family use grew: cue_delete has 2/u);
});

test("a removed compatibility call site reports a stale baseline entry so the ratchet tightens", () => {
  const failures = evaluateTestCommandBoundaries(
    scanTestCommandBoundaries([{ path: "tests/cues.spec.ts", source: "await api.executeCommandLine(\"GROUP 1 AT 50\");" }]),
    { ...EMPTY_BASELINE, compatibilityCommands: { "tests/cues.spec.ts": 1 } },
  );
  assert.deepEqual(failures, [
    "stale compatibility command call site baseline entry: tests/cues.spec.ts",
  ]);
});

test("a removed direct action family reports a stale baseline entry", () => {
  const failures = evaluateTestCommandBoundaries(scanTestCommandBoundaries([]), {
    ...EMPTY_BASELINE,
    directActionFamilies: { "preset.apply": 1 },
  });
  assert.deepEqual(failures, [
    "stale direct v1 WebSocket action family use baseline entry: preset.apply",
  ]);
});

test("a partially shrunk direct action count requires an exact baseline update", () => {
  const scan = scanTestCommandBoundaries([
    {
      path: "tests/actions.spec.ts",
      source: 'await api.command("programmer.set", { fixture_id: "one" });',
    },
  ]);
  const failures = evaluateTestCommandBoundaries(scan, {
    ...EMPTY_BASELINE,
    directActionCommands: { "tests/actions.spec.ts": 2 },
    directActionFamilies: { "programmer.set": 2 },
  });
  assert.deepEqual(failures, [
    "direct v1 WebSocket action call site shrank: tests/actions.spec.ts has 1 (baseline 2); lower or regenerate the baseline",
    "direct v1 WebSocket action family use shrank: programmer.set has 1 (baseline 2); lower or regenerate the baseline",
  ]);
});

test("an unversioned baseline is rejected", () => {
  const failures = evaluateTestCommandBoundaries(scanTestCommandBoundaries([]), {});
  assert.deepEqual(failures, ["test command boundary baseline version must be 2"]);
});
