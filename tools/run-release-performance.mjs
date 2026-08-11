#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const REQUIRED_UNIVERSES = 32;
const REQUIRED_RATE_HZ = 60;
const BASELINE_FIXTURES_PER_UNIVERSE = 32;
const DOUBLED_FIXTURES_PER_UNIVERSE = BASELINE_FIXTURES_PER_UNIVERSE * 2;
const INTERACTIVE_GREEN_HZ = 60;
const INTERACTIVE_YELLOW_HZ = 40;
const BENCHMARK_IDENTITY = "tosklight_render_to_protocol_encoding_pipeline";
export const PERFORMANCE_MEASUREMENT_SECONDS = 15;

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
		![7, 8].includes(report.schema_version) ||
		report.benchmark !== BENCHMARK_IDENTITY ||
		typeof report.required_floor_met !== "boolean" ||
		typeof report.reference?.hardware_label !== "string" ||
		!Number.isInteger(report.reference?.logical_cpus) ||
		!scenario ||
		scenario.profile !== "hard_floor" ||
		scenario.expectation !== "required_floor" ||
		scenario.universes !== REQUIRED_UNIVERSES ||
		![REQUIRED_RATE_HZ, 100].includes(scenario.configured_rate_hz) ||
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

function validateHeadlessStress(stage) {
	const report = stage?.report;
	const scenario = report?.scenarios?.[0];
	if (
		stage?.exit_code !== 0 ||
		![7, 8].includes(report?.schema_version) ||
		report?.benchmark !== BENCHMARK_IDENTITY ||
		scenario?.profile !== "headless_stress" ||
		scenario?.expectation !== "informational_capacity" ||
		scenario?.fixture_count !== 2_000 ||
		!finite(scenario?.frame_rate?.minimum_one_second_completed_hz) ||
		!finite(scenario?.frame_rate?.average_completed_hz)
	) {
		return {
			valid: false,
			reason:
				stage?.error ??
				"The 2,000-fixture benchmark did not produce valid evidence.",
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
	if (baseline.show_mutation.gate_met === false) {
		return {
			status: "degraded",
			summary:
				"The 1,024-fixture cadence was measured, but the large-show mutation budget regressed.",
		};
	}
	if (baseline.patch_mutation.gate_met !== true) {
		return {
			status: "degraded",
			summary:
				"The 1,024-fixture cadence was measured, but persisted Patch latency regressed.",
		};
	}
	const cadence =
		baselineValidation.scenario.frame_rate.minimum_one_second_completed_hz;
	if (cadence < INTERACTIVE_YELLOW_HZ) {
		return {
			status: "degraded",
			summary: `The 1,024-fixture workload fell below the ${INTERACTIVE_YELLOW_HZ} Hz minimum at ${cadence} Hz.`,
		};
	}
	if (cadence < INTERACTIVE_GREEN_HZ) {
		return {
			status: "warning",
			summary: `The 1,024-fixture workload sustained ${cadence} Hz: usable, but below the ${INTERACTIVE_GREEN_HZ} Hz green threshold.`,
		};
	}
	const diagnostic = doubledValidation?.valid
		? " The 2,048-fixture diagnostic is reported separately and does not change this indicator."
		: " The 2,048-fixture diagnostic was inconclusive and does not change this indicator.";
	return {
		status: "healthy",
		summary: `The 1,024-fixture workload stayed at or above ${INTERACTIVE_GREEN_HZ} Hz (${cadence} Hz).${diagnostic}`,
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

function stageCommand(binary, arguments_, executionMode) {
	if (executionMode !== "one_core" || process.platform !== "linux") {
		return { command: binary, arguments: arguments_ };
	}
	const affinity = spawnSync("taskset", ["--pid", "--cpu-list", String(process.pid)], {
		encoding: "utf8",
	});
	const firstAllowedCpu = affinity.stdout?.match(/:\s*(\d+)/u)?.[1];
	return firstAllowedCpu
		? {
				command: "taskset",
				arguments: ["--cpu-list", firstAllowedCpu, binary, ...arguments_],
			}
		: { command: "taskset", arguments: ["--cpu-list", "0", binary, ...arguments_] };
}

function harnessUsage(started) {
	const ended = process.resourceUsage();
	return {
		cpu_user_milliseconds: (ended.userCPUTime - started.userCPUTime) / 1000,
		cpu_system_milliseconds:
			(ended.systemCPUTime - started.systemCPUTime) / 1000,
		max_resident_bytes: ended.maxRSS * 1024,
		measurement:
			"Node orchestration process CPU delta and process-lifetime RSS high-water mark; the benchmark reports Light separately and no Playwright process runs in this workflow",
	};
}

function runStage(
	binary,
	outputDirectory,
	label,
	universes,
	fixturesPerUniverse,
	hardwareLabel,
	mutationGate,
	patchGate,
	executionMode = "unrestricted",
) {
	const arguments_ = [
		"--profile",
		"hard-floor",
		"--protocol",
		"artnet",
		"--transport",
		"encode-only",
		"--seconds",
		String(PERFORMANCE_MEASUREMENT_SECONDS),
		"--warmup-seconds",
		"1",
		"--fixtures-per-universe",
		String(fixturesPerUniverse),
		"--universes",
		String(universes),
		"--rate-hz",
		String(REQUIRED_RATE_HZ),
		"--hardware-label",
		hardwareLabel,
	];
	if (mutationGate) arguments_.push("--mutation-gate");
	if (patchGate) arguments_.push("--patch-gate");
	const invocation = stageCommand(binary, arguments_, executionMode);
	const harnessStarted = process.resourceUsage();
	const result = spawnSync(invocation.command, invocation.arguments, {
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
		fixture_count: universes * fixturesPerUniverse,
		universes,
		execution_mode: executionMode,
		harness_resources: harnessUsage(harnessStarted),
		exit_code: result.status,
		signal: result.signal,
		error: result.error?.message ?? null,
		report,
	};
}

function runHeadlessStress(
	binary,
	outputDirectory,
	fixturePackageDir,
	hardwareLabel,
	executionMode = "unrestricted",
) {
	const arguments_ = [
			"--headless-stress-fixtures",
			"2000",
			"--fixture-package-dir",
			fixturePackageDir,
			"--protocol",
			"artnet",
			"--transport",
			"encode-only",
			"--seconds",
			String(PERFORMANCE_MEASUREMENT_SECONDS),
			"--warmup-seconds",
			"1",
			"--rate-hz",
			String(REQUIRED_RATE_HZ),
			"--hardware-label",
			hardwareLabel,
		];
	const invocation = stageCommand(binary, arguments_, executionMode);
	const harnessStarted = process.resourceUsage();
	const result = spawnSync(invocation.command, invocation.arguments, {
		encoding: "utf8",
		maxBuffer: 32 * 1024 * 1024,
	});
	writeFileSync(
		resolve(outputDirectory, `two-thousand-${executionMode}.stderr.log`),
		result.stderr ?? "",
	);
	if (result.stdout)
		writeFileSync(
			resolve(outputDirectory, `two-thousand-${executionMode}.json`),
			result.stdout,
		);
	let report;
	try {
		report = JSON.parse(result.stdout);
	} catch {
		report = null;
	}
	return {
		attempted: true,
		execution_mode: executionMode,
		harness_resources: harnessUsage(harnessStarted),
		exit_code: result.status,
		signal: result.signal,
		error: result.error?.message ?? null,
		report,
	};
}

function reportDownloadUrl(options) {
	const base = options["release-url"].replace("/tag/", "/download/");
	return `${base}/report-performance.zip`;
}

function scenarioEvidence(validation) {
	if (!validation?.valid) return null;
	const scenario = validation.scenario;
	return {
		universes: scenario.universes,
		parameter_count:
			Number.isInteger(scenario.fixture_footprint) &&
			Number.isInteger(scenario.fixture_count)
				? scenario.fixture_footprint * scenario.fixture_count
				: (scenario.fixture_inventory?.total_slots ?? null),
		requested_rate_hz: scenario.configured_rate_hz,
		achieved_ticks_per_second: scenario.achieved_ticks_per_second,
		average_completed_hz: scenario.frame_rate.average_completed_hz,
		minimum_one_second_completed_hz:
			scenario.frame_rate.minimum_one_second_completed_hz,
		p95_one_second_completed_hz:
			scenario.frame_rate.p95_one_second_completed_hz ?? null,
		maximum_one_second_completed_hz:
			scenario.frame_rate.maximum_one_second_completed_hz ?? null,
		windows_below_minimum: scenario.frame_rate.windows_below_minimum,
		below_target_hz: scenario.frame_rate.reporting_target_hz,
		windows_below_target:
			scenario.frame_rate.windows_below_reporting_target,
		measurement_seconds: scenario.elapsed_seconds,
		dynamic_definition_count: scenario.dynamic_definition_count ?? null,
		animated_attribute_count: scenario.animated_attribute_count ?? null,
		master_lane_count: scenario.master_lane_count ?? null,
		dynamic_lane_attributes: scenario.dynamic_lane_attributes ?? [],
		dynamic_excluded_fixture_count:
			scenario.dynamic_excluded_fixture_count ?? null,
		deadline_misses: scenario.deadline.deadline_misses,
		dropped_ticks: scenario.deadline.dropped_ticks,
		deferred_ticks: scenario.deadline.deferred_ticks,
		phases: scenario.phases ?? null,
		resources: scenario.measurement_resources ?? null,
	};
}

function limitingPhase(validation) {
	if (!validation?.valid) return null;
	const phases = validation.scenario.phases ?? {};
	const candidates = [
		["Engine render and fixture projection", phases.engine_render_combined],
		["Protocol encoding", phases.protocol_encoding],
		["Loopback datagram delivery", phases.loopback_datagram_delivery],
		["Benchmark validation", phases.benchmark_validation_overhead],
	]
		.filter(([, distribution]) => finite(distribution?.p95_microseconds))
		.map(([name, distribution]) => ({
			name,
			p50_microseconds: distribution.p50_microseconds,
			p95_microseconds: distribution.p95_microseconds,
			p99_microseconds: distribution.p99_microseconds,
		}));
	if (candidates.length === 0) return null;
	return candidates.sort(
		(left, right) => right.p95_microseconds - left.p95_microseconds,
	)[0];
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

function canonicalDemoEvidence(candidate) {
	if (
		candidate?.schema_version !== 1 ||
		candidate?.measurement_surface !== "browser_playwright_product_demo" ||
		candidate?.scene?.fixture_records !== 295 ||
		candidate?.scene?.physical_instances !== 343 ||
		candidate?.scene?.stage_visible !== true ||
		!finite(candidate?.window?.elapsed_ms) ||
		!finite(candidate?.stage?.presentation_rate_hz) ||
		!finite(candidate?.stage?.source_to_settled_canvas_ms?.p95) ||
		!finite(candidate?.stage?.render_duration_ms?.p95)
	) {
		return {
			attempted: candidate != null,
			reason:
				"The canonical demo did not produce valid current-release performance evidence.",
		};
	}
	return { ...candidate, attempted: true, reason: null };
}

const PERFORMANCE_CASES = {
	demo: {
		thresholds: { red_below_hz: 44, critical_below_hz: 40, yellow_below_hz: 59 },
	},
	sixteen_universe: {
		thresholds: { red_below_hz: 40, yellow_below_hz: 59 },
	},
	required_1024: {
		thresholds: { red_below_hz: 40, yellow_below_hz: 44 },
	},
	doubled_2048: {
		thresholds: { red_below_hz: 30, yellow_below_hz: 44 },
	},
	maximum: {
		thresholds: { red_below_hz: 30, yellow_below_hz: 45 },
	},
};

function performanceCaseName(caseId, parameterCount, fixtureCount) {
	const measuredSize = `${parameterCount.toLocaleString("en-US")} parameters / ${fixtureCount.toLocaleString("en-US")} fixtures`;
	return caseId === "demo" ? `Demo show — ${measuredSize}` : measuredSize;
}

function benchmarkScenarioEvidence(caseId, stage) {
	const scenario = stage?.report?.scenarios?.[0];
	if (
		stage?.report?.schema_version !== 8 ||
		!scenario ||
		stage.exit_code == null ||
		scenario.configured_rate_hz !== 60 ||
		scenario.frame_rate?.reporting_target_hz !== 44 ||
		!Number.isInteger(scenario.frame_rate?.windows_below_reporting_target) ||
		!finite(scenario.elapsed_seconds) ||
		!Number.isInteger(scenario.animated_attribute_count) ||
		!Number.isInteger(scenario.master_lane_count) ||
		!finite(scenario.measurement_resources?.application_cpu_average_percent) ||
		!finite(scenario.measurement_resources?.application_cpu_max_percent) ||
		!Number.isInteger(
			scenario.measurement_resources?.application_peak_resident_bytes,
		)
	) return null;
	const evidence = scenarioEvidence({ valid: true, scenario });
	return {
		case_id: caseId,
		case_name: performanceCaseName(
			caseId,
			evidence.parameter_count,
			scenario.fixture_count,
		),
		execution_mode: stage.execution_mode,
		cpu_limit: stage.execution_mode === "one_core" ? 1 : null,
		fixture_count: scenario.fixture_count,
		physical_instance_count: scenario.physical_instance_count,
		...evidence,
		thresholds: PERFORMANCE_CASES[caseId].thresholds,
		harness_resources: stage.harness_resources ?? null,
	};
}

export function statusDocument(
	options,
	baseline,
	doubled,
	canonicalDemo = null,
	twoThousand = null,
	benchmarkRuns = [],
) {
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
	const baselineCadence = observed?.minimum_one_second_completed_hz ?? null;
	const failedGates = report
		? [
				baselineCadence != null &&
					baselineCadence < INTERACTIVE_YELLOW_HZ &&
					"interactive_output_red",
				report.show_mutation.gate_met === false && "show_mutation",
				report.patch_mutation.gate_met === false && "patch_mutation",
			].filter(Boolean)
		: [];
	const doubledObserved = scenarioEvidence(doubledValidation);
	const twoThousandValidation = twoThousand
		? validateHeadlessStress(twoThousand)
		: null;
	const twoThousandObserved = scenarioEvidence(twoThousandValidation);
	return {
		schema_version: 5,
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
			warnings:
				baselineCadence != null &&
				baselineCadence >= INTERACTIVE_YELLOW_HZ &&
				baselineCadence < INTERACTIVE_GREEN_HZ
					? ["interactive_output_yellow"]
					: [],
		},
		runner: report
			? {
					hardware_label: report.reference.hardware_label,
					cpu_model: report.reference.cpu_model ?? null,
					logical_cpus: report.reference.logical_cpus,
					total_memory_bytes: report.reference.total_memory_bytes ?? null,
					operating_system: report.reference.operating_system,
					architecture: report.reference.architecture,
					rustc_version: report.reference.rustc_version ?? null,
					package_version: report.reference.package_version ?? null,
					build_profile: report.reference.build_profile ?? null,
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
		benchmark_scenarios: benchmarkRuns
			.map(({ case_id, stage }) => benchmarkScenarioEvidence(case_id, stage))
			.filter(Boolean),
		required_floor: {
			universes: REQUIRED_UNIVERSES,
			rate_hz: REQUIRED_RATE_HZ,
			fixtures_per_universe: BASELINE_FIXTURES_PER_UNIVERSE,
			fixture_count: REQUIRED_UNIVERSES * BASELINE_FIXTURES_PER_UNIVERSE,
			parameter_count: observed?.parameter_count ?? null,
			met:
				baselineCadence == null
					? null
					: baselineCadence >= INTERACTIVE_GREEN_HZ,
			configured_target_met: report?.required_floor_met ?? null,
			green_threshold_hz: INTERACTIVE_GREEN_HZ,
			yellow_threshold_hz: INTERACTIVE_YELLOW_HZ,
			achieved_ticks_per_second: observed?.achieved_ticks_per_second ?? null,
			average_completed_hz: observed?.average_completed_hz ?? null,
			minimum_one_second_completed_hz:
				observed?.minimum_one_second_completed_hz ?? null,
			p95_one_second_completed_hz:
				observed?.p95_one_second_completed_hz ?? null,
			maximum_one_second_completed_hz:
				observed?.maximum_one_second_completed_hz ?? null,
			windows_below_minimum: observed?.windows_below_minimum ?? null,
			dynamic_definition_count: observed?.dynamic_definition_count ?? null,
			dynamic_lane_attributes: observed?.dynamic_lane_attributes ?? [],
			dynamic_excluded_fixture_count:
				observed?.dynamic_excluded_fixture_count ?? null,
			deadline_misses: observed?.deadline_misses ?? null,
			dropped_ticks: observed?.dropped_ticks ?? null,
			deferred_ticks: observed?.deferred_ticks ?? null,
			phases: observed?.phases ?? null,
			limiting_phase: limitingPhase(baselineValidation),
			resources: report?.process_resources ?? null,
		},
		show_mutation: mutationEvidence(report),
		patch: patchEvidence(report),
		canonical_demo: canonicalDemoEvidence(canonicalDemo),
		two_thousand_show: twoThousandValidation?.valid
			? {
					attempted: true,
					reason: null,
					fixture_count: twoThousandValidation.scenario.fixture_count,
					universes: twoThousandObserved.universes,
					parameter_count: twoThousandObserved.parameter_count,
					average_completed_hz: twoThousandObserved.average_completed_hz,
					minimum_one_second_completed_hz:
						twoThousandObserved.minimum_one_second_completed_hz,
					p95_one_second_completed_hz:
						twoThousandObserved.p95_one_second_completed_hz,
					maximum_one_second_completed_hz:
						twoThousandObserved.maximum_one_second_completed_hz,
					windows_below_minimum: twoThousandObserved.windows_below_minimum,
					requested_rate_hz: twoThousandObserved.requested_rate_hz,
					dynamic_definition_count:
						twoThousandObserved.dynamic_definition_count,
					dynamic_lane_attributes: twoThousandObserved.dynamic_lane_attributes,
					dynamic_excluded_fixture_count:
						twoThousandObserved.dynamic_excluded_fixture_count,
					resources: twoThousandValidation.report.process_resources ?? null,
				}
			: {
					attempted: twoThousand != null,
					reason:
						twoThousandValidation?.reason ??
						"The exact 2,000-fixture workload was not executed.",
					fixture_count: 2_000,
				},
		doubled_density: doubled
			? {
					attempted: true,
					reason: doubledValidation?.valid ? null : doubledValidation?.reason,
					fixtures_per_universe: DOUBLED_FIXTURES_PER_UNIVERSE,
					fixture_count: REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE,
					universes: doubledObserved?.universes ?? REQUIRED_UNIVERSES,
					parameter_count: doubledObserved?.parameter_count ?? null,
					met: null,
					requested_rate_hz:
						doubledObserved?.requested_rate_hz ?? REQUIRED_RATE_HZ,
					configured_target_met: doubledValidation?.valid
						? doubledValidation.report.required_floor_met
						: null,
					achieved_ticks_per_second:
						doubledObserved?.achieved_ticks_per_second ?? null,
					minimum_one_second_completed_hz:
						doubledObserved?.minimum_one_second_completed_hz ?? null,
					average_completed_hz: doubledObserved?.average_completed_hz ?? null,
					p95_one_second_completed_hz:
						doubledObserved?.p95_one_second_completed_hz ?? null,
					maximum_one_second_completed_hz:
						doubledObserved?.maximum_one_second_completed_hz ?? null,
					windows_below_minimum: doubledObserved?.windows_below_minimum ?? null,
					dynamic_definition_count:
						doubledObserved?.dynamic_definition_count ?? null,
					dynamic_lane_attributes:
						doubledObserved?.dynamic_lane_attributes ?? [],
					dynamic_excluded_fixture_count:
						doubledObserved?.dynamic_excluded_fixture_count ?? null,
					deadline_misses: doubledObserved?.deadline_misses ?? null,
					dropped_ticks: doubledObserved?.dropped_ticks ?? null,
					deferred_ticks: doubledObserved?.deferred_ticks ?? null,
					phases: doubledObserved?.phases ?? null,
					limiting_phase: limitingPhase(doubledValidation),
					resources: doubledValidation?.report?.process_resources ?? null,
				}
			: {
					attempted: false,
					reason: "The diagnostic could not be executed.",
					fixtures_per_universe: DOUBLED_FIXTURES_PER_UNIVERSE,
					fixture_count: REQUIRED_UNIVERSES * DOUBLED_FIXTURES_PER_UNIVERSE,
					universes: REQUIRED_UNIVERSES,
					parameter_count: null,
					met: null,
					requested_rate_hz: REQUIRED_RATE_HZ,
					configured_target_met: null,
					achieved_ticks_per_second: null,
					minimum_one_second_completed_hz: null,
					average_completed_hz: null,
					p95_one_second_completed_hz: null,
					maximum_one_second_completed_hz: null,
					windows_below_minimum: null,
					dynamic_definition_count: null,
					dynamic_lane_attributes: [],
					dynamic_excluded_fixture_count: null,
					deadline_misses: null,
					dropped_ticks: null,
					deferred_ticks: null,
					phases: null,
					limiting_phase: null,
					resources: null,
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
		REQUIRED_UNIVERSES,
		BASELINE_FIXTURES_PER_UNIVERSE,
		hardwareLabel,
		true,
		true,
	);
	const doubled = runStage(
		resolve(options.binary),
		outputDirectory,
		"doubled-density",
		REQUIRED_UNIVERSES,
		DOUBLED_FIXTURES_PER_UNIVERSE,
		hardwareLabel,
		false,
		false,
	);
	const twoThousand = options["fixture-package-dir"]
		? runHeadlessStress(
				resolve(options.binary),
				outputDirectory,
				resolve(options["fixture-package-dir"]),
				hardwareLabel,
			)
		: null;
	const benchmarkRuns = [
		{ case_id: "required_1024", stage: baseline },
		{ case_id: "doubled_2048", stage: doubled },
		...(twoThousand ? [{ case_id: "maximum", stage: twoThousand }] : []),
	];
	for (const executionMode of ["one_core", "unrestricted"]) {
		for (const workload of [
			{ case_id: "demo", universes: 8, fixtures: 36 },
			{ case_id: "sixteen_universe", universes: 16, fixtures: 36 },
		]) {
			benchmarkRuns.push({
				case_id: workload.case_id,
				stage: runStage(
					resolve(options.binary),
					outputDirectory,
					`${workload.case_id}-${executionMode}`,
					workload.universes,
					workload.fixtures,
					hardwareLabel,
					false,
					false,
					executionMode,
				),
			});
		}
	}
	for (const workload of [
		{ case_id: "required_1024", universes: 32, fixtures: 32 },
		{ case_id: "doubled_2048", universes: 32, fixtures: 64 },
	]) {
		benchmarkRuns.push({
			case_id: workload.case_id,
			stage: runStage(
				resolve(options.binary), outputDirectory,
				`${workload.case_id}-one_core`, workload.universes, workload.fixtures,
				hardwareLabel, false, false, "one_core",
			),
		});
	}
	if (options["fixture-package-dir"]) {
		benchmarkRuns.push({
			case_id: "maximum",
			stage: runHeadlessStress(
				resolve(options.binary), outputDirectory,
				resolve(options["fixture-package-dir"]), hardwareLabel, "one_core",
			),
		});
	}
	let canonicalDemo = null;
	if (options["canonical-demo-performance"]) {
		try {
			canonicalDemo = JSON.parse(
				readFileSync(resolve(options["canonical-demo-performance"]), "utf8"),
			);
		} catch {
			canonicalDemo = null;
		}
	}
	const status = statusDocument(
		options,
		baseline,
		doubled,
		canonicalDemo,
		twoThousand,
		benchmarkRuns,
	);
	if (status.benchmark_scenarios.length !== benchmarkRuns.length) {
		status.status = "unknown";
		status.summary =
			"One or more 60 Hz performance scenarios did not produce complete Light-process cadence and resource evidence.";
	}
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
			`- Interactive ceiling: ${status.required_floor.fixture_count} fixtures across ` +
				`${status.required_floor.universes} universes; green ≥${INTERACTIVE_GREEN_HZ} Hz, yellow ≥${INTERACTIVE_YELLOW_HZ} Hz`,
			`- Achieved cadence: ${status.required_floor.achieved_ticks_per_second ?? "missing"} Hz`,
			`- Deadline misses: ${status.required_floor.deadline_misses ?? "missing"}`,
			`- Canonical demo Stage presentation cadence (306 physical instances): ${status.canonical_demo.stage?.presentation_rate_hz ?? "missing"} Hz`,
			`- Canonical demo Stage source-to-canvas p95: ${status.canonical_demo.stage?.source_to_settled_canvas_ms?.p95 ?? "missing"} ms`,
			`- Show mutation p95 (${status.show_mutation?.large.fixture_count ?? "large"} fixtures): ` +
				`${status.show_mutation?.large.p95_microseconds ?? "missing"} µs`,
			`- 2,048-fixture diagnostic cadence: ${status.doubled_density.achieved_ticks_per_second ?? "missing"} Hz`,
			`- 2,048-fixture limiting phase: ${status.doubled_density.limiting_phase?.name ?? "missing"}`,
			`- Exact 2,000-fixture shipped-mode cadence: ${status.two_thousand_show.average_completed_hz ?? "missing"} Hz average`,
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
