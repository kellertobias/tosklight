#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REQUIRED_UNIVERSES = 32;
const REQUIRED_RATE_HZ = 100;
const BASELINE_FIXTURES_PER_UNIVERSE = 32;
const DOUBLED_FIXTURES_PER_UNIVERSE = BASELINE_FIXTURES_PER_UNIVERSE * 2;

export function classifyPerformance(baseline, doubled) {
  if (!baseline?.report || baseline.report.required_floor_met == null) {
    return {
      status: "unknown",
      summary: "The released benchmark artifact did not produce a valid hard-floor result.",
    };
  }
  if (baseline.report.required_floor_met !== true) {
    return {
      status: "degraded",
      summary:
        `The release missed the ${REQUIRED_UNIVERSES}-universe ${REQUIRED_RATE_HZ} Hz floor ` +
        `at ${BASELINE_FIXTURES_PER_UNIVERSE} fixtures per universe.`,
    };
  }
  if (baseline.report.show_mutation?.gate_met === false) {
    return {
      status: "degraded",
      summary: "The release met the output floor but regressed the large-show mutation budget.",
    };
  }
  if (!baseline.report.patch_mutation) {
    return {
      status: "unknown",
      summary: "The released artifact did not produce the required Patch transaction evidence.",
    };
  }
  if (baseline.report.patch_mutation.gate_met !== true) {
    return {
      status: "degraded",
      summary: "The release met the output floor but regressed the persisted Patch latency budget.",
    };
  }
  if (!doubled?.report || doubled.report.required_floor_met == null) {
    return {
      status: "healthy",
      summary:
        `The release met the required floor with ` +
        `${REQUIRED_UNIVERSES * BASELINE_FIXTURES_PER_UNIVERSE} fixtures; ` +
        "the doubled-density capacity probe was inconclusive.",
    };
  }
  const doubledFixtures = REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE;
  return doubled.report.required_floor_met === true
    ? {
        status: "healthy",
        summary:
          `The release met the required floor and the doubled-density probe with ` +
          `${doubledFixtures} fixtures.`,
      }
    : {
        status: "healthy",
        summary:
          `The release met the required floor; the optional ${doubledFixtures}-fixture ` +
          "capacity probe did not sustain 100 Hz.",
      };
}

function parseArguments(arguments_) {
  const values = {};
  for (let index = 0; index < arguments_.length; index += 2) {
    const option = arguments_[index];
    const value = arguments_[index + 1];
    if (!option?.startsWith("--") || value == null) {
      throw new Error(`invalid argument list near ${option ?? "<end>"}`);
    }
    values[option.slice(2)] = value;
  }
  for (const required of ["binary", "output-dir", "version", "commit", "release-url"]) {
    if (!values[required]) throw new Error(`--${required} is required`);
  }
  return values;
}

function runStage(
  binary,
  outputDirectory,
  label,
  fixturesPerUniverse,
  hardwareLabel,
  mutationGate,
  patchGate,
) {
  const arguments_ = [
    "--profile",
    "hard-floor",
    "--protocol",
    "artnet",
    "--transport",
    "encode-only",
    "--seconds",
    "3",
    "--warmup-seconds",
    "1",
    "--fixtures-per-universe",
    String(fixturesPerUniverse),
    "--hardware-label",
    hardwareLabel,
  ];
  if (mutationGate) arguments_.push("--mutation-gate");
  if (patchGate) arguments_.push("--patch-gate");
  const result = spawnSync(binary, arguments_, {
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  });
  writeFileSync(resolve(outputDirectory, `${label}.stderr.log`), result.stderr ?? "");
  if (result.stdout) writeFileSync(resolve(outputDirectory, `${label}.json`), result.stdout);
  let report;
  try {
    report = JSON.parse(result.stdout);
  } catch {
    report = null;
  }
  return {
    attempted: true,
    fixtures_per_universe: fixturesPerUniverse,
    fixture_count: REQUIRED_UNIVERSES * fixturesPerUniverse,
    exit_code: result.status,
    signal: result.signal,
    error: result.error?.message ?? null,
    report,
  };
}

function statusDocument(options, baseline, doubled) {
  const classification = classifyPerformance(baseline, doubled);
  return {
    schema_version: 2,
    status: classification.status,
    summary: classification.summary,
    generated_at: new Date().toISOString(),
    release: {
      version: options.version,
      commit: options.commit,
      url: options["release-url"],
    },
    required_floor: {
      universes: REQUIRED_UNIVERSES,
      rate_hz: REQUIRED_RATE_HZ,
      fixtures_per_universe: BASELINE_FIXTURES_PER_UNIVERSE,
      fixture_count: REQUIRED_UNIVERSES * BASELINE_FIXTURES_PER_UNIVERSE,
      met: baseline?.report?.required_floor_met ?? null,
    },
    mutation_gate_met: baseline?.report?.show_mutation?.gate_met ?? null,
    patch: baseline?.report?.patch_mutation
      ? {
          server: {
            single_fixture: {
              p50_microseconds:
                baseline.report.patch_mutation.single_fixture.total_server.p50_microseconds,
              p95_microseconds:
                baseline.report.patch_mutation.single_fixture.total_server.p95_microseconds,
              gate_p95_microseconds:
                baseline.report.patch_mutation.single_fixture.gate_p95_microseconds,
              gate_met: baseline.report.patch_mutation.single_fixture.gate_met,
            },
            hundred_fixtures: {
              p50_microseconds:
                baseline.report.patch_mutation.hundred_fixtures.total_server.p50_microseconds,
              p95_microseconds:
                baseline.report.patch_mutation.hundred_fixtures.total_server.p95_microseconds,
              gate_p95_microseconds:
                baseline.report.patch_mutation.hundred_fixtures.gate_p95_microseconds,
              gate_met: baseline.report.patch_mutation.hundred_fixtures.gate_met,
            },
            gate_met: baseline.report.patch_mutation.gate_met,
          },
          ui: {
            gate_enforced: false,
            status: "informational",
            p50_microseconds: null,
            p95_microseconds: null,
            note: "Action-to-visible Patch paint is measured by the focused browser acceptance run.",
          },
        }
      : null,
    doubled_density: doubled
      ? {
          attempted: true,
          fixtures_per_universe: DOUBLED_FIXTURES_PER_UNIVERSE,
          fixture_count: REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE,
          met: doubled.report?.required_floor_met ?? null,
        }
      : {
          attempted: false,
          fixtures_per_universe: DOUBLED_FIXTURES_PER_UNIVERSE,
          fixture_count: REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE,
          met: null,
        },
  };
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const outputDirectory = resolve(options["output-dir"]);
  mkdirSync(outputDirectory, { recursive: true });
  const hardwareLabel = options["hardware-label"] ?? "GitHub Actions ubuntu-22.04";
  const baseline = runStage(
    resolve(options.binary),
    outputDirectory,
    "hard-floor",
    BASELINE_FIXTURES_PER_UNIVERSE,
    hardwareLabel,
    true,
    true,
  );
  const baselinePassed =
    baseline.report?.required_floor_met === true &&
    baseline.report?.show_mutation?.gate_met !== false &&
    baseline.report?.patch_mutation?.gate_met === true;
  const doubled = baselinePassed
    ? runStage(
        resolve(options.binary),
        outputDirectory,
        "doubled-density",
        DOUBLED_FIXTURES_PER_UNIVERSE,
        hardwareLabel,
        false,
        false,
      )
    : null;
  const status = statusDocument(options, baseline, doubled);
  writeFileSync(resolve(outputDirectory, "status.json"), `${JSON.stringify(status, null, 2)}\n`);
  writeFileSync(
    resolve(outputDirectory, "summary.md"),
    [
      "## Release performance",
      "",
      `**${status.status.toUpperCase()}** — ${status.summary}`,
      "",
      `- Required floor: ${status.required_floor.fixture_count} fixtures across ` +
        `${status.required_floor.universes} full universes at ${status.required_floor.rate_hz} Hz`,
      `- Doubled density: ${status.doubled_density.attempted ? status.doubled_density.met : "skipped"}`,
      `- Patch server p95 (1 fixture): ${status.patch?.server.single_fixture.p95_microseconds ?? "missing"} µs`,
      `- Patch server p95 (100 fixtures): ${status.patch?.server.hundred_fixtures.p95_microseconds ?? "missing"} µs`,
      "- Patch UI action-to-visible: informational browser evidence (not a release gate)",
      `- Release: ${status.release.url}`,
      "",
    ].join("\n"),
  );
  console.log(`${status.status}: ${status.summary}`);
}

const isMain = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
