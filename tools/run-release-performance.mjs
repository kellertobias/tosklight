#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REQUIRED_UNIVERSES = 32;
const REQUIRED_RATE_HZ = 100;
const BASELINE_FIXTURES_PER_UNIVERSE = 32;
const DOUBLED_FIXTURES_PER_UNIVERSE = BASELINE_FIXTURES_PER_UNIVERSE * 2;
const BENCHMARK_IDENTITY = "tosklight_render_to_protocol_encoding_pipeline";

const finite = (value) => typeof value === "number" && Number.isFinite(value);

function observedScenario(report) {
	return (
		report?.scenarios?.find((scenario) => scenario.release_blocking === true) ??
		report?.scenarios?.[0]
	);
}

function validDistribution(distribution) {
	return (
		finite(distribution?.p50_microseconds) &&
		finite(distribution?.p95_microseconds)
	);
}

function validateStage(stage, { mutations }) {
	if (stage?.error || stage?.signal || !Number.isInteger(stage?.exit_code)) {
		return {
			valid: false,
			reason: stage?.error ?? "The benchmark process did not exit normally.",
		};
	}
	const report = stage.report;
	const scenario = observedScenario(report);
	if (
		!report ||
		report.schema_version !== 6 ||
		report.benchmark !== BENCHMARK_IDENTITY ||
		typeof report.required_floor_met !== "boolean" ||
		typeof report.reference?.hardware_label !== "string" ||
		!Number.isInteger(report.reference?.logical_cpus) ||
		!scenario ||
		scenario.profile !== "hard_floor" ||
		scenario.expectation !== "required_floor" ||
		scenario.universes !== REQUIRED_UNIVERSES ||
		scenario.configured_rate_hz !== REQUIRED_RATE_HZ ||
		!Number.isInteger(scenario.fixtures_per_universe) ||
		!Number.isInteger(scenario.fixture_count) ||
		scenario.fixtures_per_universe !== stage.fixtures_per_universe ||
		scenario.fixture_count !== stage.fixture_count ||
		!finite(scenario.achieved_ticks_per_second) ||
		!finite(scenario.frame_rate?.minimum_one_second_completed_hz) ||
		!Number.isInteger(scenario.deadline?.deadline_misses) ||
		!Number.isInteger(scenario.deadline?.dropped_ticks) ||
		!Number.isInteger(scenario.deadline?.deferred_ticks)
	) {
		return {
			valid: false,
			reason:
				"The benchmark JSON is missing required measured output evidence.",
		};
	}
	if (
		mutations &&
		(typeof report.show_mutation?.gate_met !== "boolean" ||
			!validDistribution(report.show_mutation?.small) ||
			!validDistribution(report.show_mutation?.large) ||
			typeof report.patch_mutation?.gate_met !== "boolean" ||
			!validDistribution(report.patch_mutation?.single_fixture?.total_server) ||
			!validDistribution(report.patch_mutation?.hundred_fixtures?.total_server))
	) {
		return {
			valid: false,
			reason: "The benchmark JSON is missing required mutation evidence.",
		};
	}
	const failed =
		report.required_floor_met === false ||
		report.show_mutation?.gate_met === false ||
		report.patch_mutation?.gate_met === false;
	const expectedExit = failed ? 1 : 0;
	if (stage.exit_code !== expectedExit) {
		return {
			valid: false,
			reason: `Benchmark exit code ${stage.exit_code} is inconsistent with its measured gates.`,
		};
	}
	return { valid: true, report, scenario };
}

export function classifyPerformance(baselineValidation, doubledValidation) {
	if (!baselineValidation?.valid) {
		return {
			status: "unknown",
			summary:
				baselineValidation?.reason ??
				"The released benchmark artifact did not produce valid evidence.",
		};
	}
	const baseline = baselineValidation.report;
	if (baseline.required_floor_met !== true) {
		return {
			status: "degraded",
			summary:
				`The release missed the ${REQUIRED_UNIVERSES}-universe ${REQUIRED_RATE_HZ} Hz floor ` +
				`at ${BASELINE_FIXTURES_PER_UNIVERSE} fixtures per universe.`,
		};
	}
	if (baseline.show_mutation.gate_met === false) {
		return {
			status: "degraded",
			summary:
				"The release met the output floor but regressed the large-show mutation budget.",
		};
	}
	if (baseline.patch_mutation.gate_met !== true) {
		return {
			status: "degraded",
			summary:
				"The release met the output floor but regressed the persisted Patch latency budget.",
		};
	}
	if (!doubledValidation?.valid) {
		return {
			status: "healthy",
			summary:
				`The release met the required floor with ` +
				`${REQUIRED_UNIVERSES * BASELINE_FIXTURES_PER_UNIVERSE} fixtures; ` +
				"the doubled-density capacity probe was inconclusive.",
		};
	}
	const doubledFixtures = REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE;
	return doubledValidation.report.required_floor_met === true
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
	for (const required of [
		"binary",
		"output-dir",
		"version",
		"commit",
		"release-url",
	]) {
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
	writeFileSync(
		resolve(outputDirectory, `${label}.stderr.log`),
		result.stderr ?? "",
	);
	if (result.stdout)
		writeFileSync(resolve(outputDirectory, `${label}.json`), result.stdout);
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

function reportDownloadUrl(options) {
	const base = options["release-url"].replace("/tag/", "/download/");
	return `${base}/tosklight-performance-report-${options.version}.zip`;
}

function scenarioEvidence(validation) {
	if (!validation?.valid) return null;
	const scenario = validation.scenario;
	return {
		achieved_ticks_per_second: scenario.achieved_ticks_per_second,
		minimum_one_second_completed_hz:
			scenario.frame_rate.minimum_one_second_completed_hz,
		deadline_misses: scenario.deadline.deadline_misses,
		dropped_ticks: scenario.deadline.dropped_ticks,
		deferred_ticks: scenario.deadline.deferred_ticks,
	};
}

function mutationEvidence(report) {
	if (!report?.show_mutation) return null;
	const scenario = (value, fixtureCount) => ({
		fixture_count: fixtureCount,
		p50_microseconds: value.p50_microseconds,
		p95_microseconds: value.p95_microseconds,
	});
	return {
		gate_met: report.show_mutation.gate_met,
		small: scenario(
			report.show_mutation.small,
			report.show_mutation.small_fixture_count,
		),
		large: scenario(
			report.show_mutation.large,
			report.show_mutation.large_fixture_count,
		),
	};
}

function patchEvidence(report) {
	if (!report?.patch_mutation) return null;
	return {
		server: {
			single_fixture: {
				p50_microseconds:
					report.patch_mutation.single_fixture.total_server.p50_microseconds,
				p95_microseconds:
					report.patch_mutation.single_fixture.total_server.p95_microseconds,
				gate_p95_microseconds:
					report.patch_mutation.single_fixture.gate_p95_microseconds,
				gate_met: report.patch_mutation.single_fixture.gate_met,
			},
			hundred_fixtures: {
				p50_microseconds:
					report.patch_mutation.hundred_fixtures.total_server.p50_microseconds,
				p95_microseconds:
					report.patch_mutation.hundred_fixtures.total_server.p95_microseconds,
				gate_p95_microseconds:
					report.patch_mutation.hundred_fixtures.gate_p95_microseconds,
				gate_met: report.patch_mutation.hundred_fixtures.gate_met,
			},
			gate_met: report.patch_mutation.gate_met,
		},
		ui: {
			gate_enforced: false,
			status: "informational",
			p50_microseconds: null,
			p95_microseconds: null,
			note: "Action-to-visible Patch paint is measured by the focused browser acceptance run.",
		},
	};
}

export function statusDocument(options, baseline, doubled) {
	const baselineValidation = validateStage(baseline, { mutations: true });
	const doubledValidation = doubled
		? validateStage(doubled, { mutations: false })
		: null;
	const classification = classifyPerformance(
		baselineValidation,
		doubledValidation,
	);
	const report = baselineValidation.valid ? baselineValidation.report : null;
	const scenario = baselineValidation.valid
		? baselineValidation.scenario
		: null;
	const observed = scenarioEvidence(baselineValidation);
	const failedGates = report
		? [
				report.required_floor_met === false && "required_floor",
				report.show_mutation.gate_met === false && "show_mutation",
				report.patch_mutation.gate_met === false && "patch_mutation",
			].filter(Boolean)
		: [];
	const doubledObserved = scenarioEvidence(doubledValidation);
	return {
		schema_version: 3,
		status: classification.status,
		summary: classification.summary,
		generated_at: new Date().toISOString(),
		release: {
			version: options.version,
			commit: options.commit,
			url: options["release-url"],
		},
		report_url: reportDownloadUrl(options),
		evidence: {
			kind: baselineValidation.valid ? "measured" : "unknown",
			baseline: {
				exit_code: baseline?.exit_code ?? null,
				signal: baseline?.signal ?? null,
				error: baseline?.error ?? baselineValidation.reason ?? null,
			},
			failed_gates: failedGates,
		},
		runner: report
			? {
					hardware_label: report.reference.hardware_label,
					cpu_model: report.reference.cpu_model ?? null,
					logical_cpus: report.reference.logical_cpus,
					operating_system: report.reference.operating_system,
					architecture: report.reference.architecture,
				}
			: null,
		workload: report
			? {
					benchmark: report.benchmark,
					profile: scenario.profile,
					universes: scenario.universes,
					fixture_count: scenario.fixture_count,
					requested_rate_hz: scenario.configured_rate_hz,
				}
			: null,
		required_floor: {
			universes: REQUIRED_UNIVERSES,
			rate_hz: REQUIRED_RATE_HZ,
			fixtures_per_universe: BASELINE_FIXTURES_PER_UNIVERSE,
			fixture_count: REQUIRED_UNIVERSES * BASELINE_FIXTURES_PER_UNIVERSE,
			met: report?.required_floor_met ?? null,
			achieved_ticks_per_second: observed?.achieved_ticks_per_second ?? null,
			minimum_one_second_completed_hz:
				observed?.minimum_one_second_completed_hz ?? null,
			deadline_misses: observed?.deadline_misses ?? null,
			dropped_ticks: observed?.dropped_ticks ?? null,
			deferred_ticks: observed?.deferred_ticks ?? null,
		},
		show_mutation: mutationEvidence(report),
		patch: patchEvidence(report),
		doubled_density: doubled
			? {
					attempted: true,
					reason: doubledValidation?.valid ? null : doubledValidation?.reason,
					fixtures_per_universe: DOUBLED_FIXTURES_PER_UNIVERSE,
					fixture_count: REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE,
					met: doubledValidation?.valid
						? doubledValidation.report.required_floor_met
						: null,
					achieved_ticks_per_second:
						doubledObserved?.achieved_ticks_per_second ?? null,
					minimum_one_second_completed_hz:
						doubledObserved?.minimum_one_second_completed_hz ?? null,
					deadline_misses: doubledObserved?.deadline_misses ?? null,
					dropped_ticks: doubledObserved?.dropped_ticks ?? null,
					deferred_ticks: doubledObserved?.deferred_ticks ?? null,
				}
			: {
					attempted: false,
					reason: baselineValidation.valid
						? "Not attempted because a required baseline or mutation gate did not pass."
						: "Not attempted because the baseline evidence was invalid.",
					fixtures_per_universe: DOUBLED_FIXTURES_PER_UNIVERSE,
					fixture_count: REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE,
					met: null,
					achieved_ticks_per_second: null,
					minimum_one_second_completed_hz: null,
					deadline_misses: null,
					dropped_ticks: null,
					deferred_ticks: null,
				},
	};
}

function main() {
	const options = parseArguments(process.argv.slice(2));
	const outputDirectory = resolve(options["output-dir"]);
	mkdirSync(outputDirectory, { recursive: true });
	const hardwareLabel =
		options["hardware-label"] ?? "GitHub Actions ubuntu-22.04";
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
	writeFileSync(
		resolve(outputDirectory, "status.json"),
		`${JSON.stringify(status, null, 2)}\n`,
	);
	writeFileSync(
		resolve(outputDirectory, "summary.md"),
		[
			"## Release performance",
			"",
			`**${status.status.toUpperCase()}** — ${status.summary}`,
			"",
			`- Required floor: ${status.required_floor.fixture_count} fixtures across ` +
				`${status.required_floor.universes} full universes at ${status.required_floor.rate_hz} Hz`,
			`- Achieved cadence: ${status.required_floor.achieved_ticks_per_second ?? "missing"} Hz`,
			`- Deadline misses: ${status.required_floor.deadline_misses ?? "missing"}`,
			`- Show mutation p95 (${status.show_mutation?.large.fixture_count ?? "large"} fixtures): ` +
				`${status.show_mutation?.large.p95_microseconds ?? "missing"} µs`,
			`- Doubled density: ${status.doubled_density.attempted ? status.doubled_density.met : "skipped"}`,
			`- Patch server p95 (1 fixture): ${status.patch?.server.single_fixture.p95_microseconds ?? "missing"} µs`,
			`- Patch server p95 (100 fixtures): ${status.patch?.server.hundred_fixtures.p95_microseconds ?? "missing"} µs`,
			"- Patch UI action-to-visible: informational browser evidence (not a release gate)",
			`- Release: ${status.release.url}`,
			"",
		].join("\n"),
	);
	console.log(`${status.status}: ${status.summary}`);
	if (status.status === "unknown") process.exitCode = 1;
}

const isMain =
	process.argv[1] &&
	resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
	try {
		main();
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		process.exitCode = 1;
	}
}
