#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

export function summarizeHeadlessStress(report, expectedFixtures) {
	const scenario = report?.scenarios?.find(
		(candidate) => candidate.workload_tier === "headless_stress",
	);
	if (!scenario?.frame_rate || !scenario.fixture_inventory) {
		throw new Error("benchmark report has no headless stress evidence");
	}
	const valid =
		scenario.fixture_count === expectedFixtures &&
		scenario.physical_instance_count === expectedFixtures &&
		scenario.dynamic_definition_count === 20 &&
		scenario.dynamic_excluded_fixture_count === expectedFixtures * 0.46 &&
		["intensity", "color.red", "color.green", "color.blue", "pan", "tilt"]
			.every((attribute) => scenario.dynamic_lane_attributes?.includes(attribute)) &&
		scenario.release_blocking === false &&
		scenario.visualization_enabled === false &&
		Array.isArray(scenario.active_ui_surfaces) &&
		scenario.active_ui_surfaces.length === 0;
	return {
		valid,
		targetMet: scenario.met_configured_rate === true,
		fixtureCount: scenario.fixture_count,
		physicalInstances: scenario.physical_instance_count,
		dynamicCount: scenario.dynamic_definition_count,
		universes: scenario.universes,
		occupiedSlots: scenario.fixture_inventory.total_slots,
		averageHz: scenario.frame_rate.average_completed_hz,
		minimumHz: scenario.frame_rate.minimum_one_second_completed_hz,
		targetHz: scenario.frame_rate.required_minimum_hz,
		dropped: scenario.deadline.dropped_ticks,
		deferred: scenario.deadline.deferred_ticks,
		deadlineMisses: scenario.deadline.deadline_misses,
		residentBytes: report.process_resources?.resident_bytes ?? null,
	};
}

function parseArguments(arguments_) {
	const options = { fixtures: 2_000, seconds: 5 };
	for (let index = 0; index < arguments_.length; index += 2) {
		const option = arguments_[index];
		const value = arguments_[index + 1];
		if (!option?.startsWith("--") || value == null) {
			throw new Error(`invalid argument list near ${option ?? "<end>"}`);
		}
		if (option === "--fixtures") {
			options.fixtures = Number(value);
			if (options.fixtures !== 2_000 && options.fixtures !== 4_000) {
				throw new Error("--fixtures must be 2000 or 4000");
			}
		} else if (option === "--seconds") {
			options.seconds = Number(value);
			if (!Number.isInteger(options.seconds) || options.seconds < 1 || options.seconds > 300) {
				throw new Error("--seconds must be within 1-300");
			}
		} else if (option === "--hardware-label") {
			if (!value.trim()) throw new Error("--hardware-label must not be empty");
			options.hardwareLabel = value;
		} else if (option === "--output") {
			options.output = value;
		} else {
			throw new Error(`unknown argument: ${option}`);
		}
	}
	return options;
}

function defaultOutputPath(fixtures) {
	const timestamp = new Date().toISOString().replaceAll(/[:.]/gu, "-");
	return resolve(
		artifactPaths.performance,
		`headless-stress-${fixtures}-${timestamp}.json`,
	);
}

function main() {
	const options = parseArguments(process.argv.slice(2));
	const output = resolve(
		repositoryRoot,
		options.output ?? defaultOutputPath(options.fixtures),
	);
	const stderrOutput = output.replace(/\.json$/u, ".stderr.log");
	mkdirSync(dirname(output), { recursive: true });
	const benchmarkArguments = [
		"run", "--release", "--locked", "--no-default-features",
		"-p", "light-headless", "--bin", "light-benchmark", "--",
		"--protocol", "both",
		"--transport", "encode-only",
		"--seconds", String(options.seconds),
		"--warmup-seconds", "1",
		"--headless-stress-fixtures", String(options.fixtures),
		"--fixture-package-dir", "assets/fixture-library",
	];
	if (options.hardwareLabel) {
		benchmarkArguments.push("--hardware-label", options.hardwareLabel);
	}
	const result = spawnSync("cargo", benchmarkArguments, {
		cwd: repositoryRoot,
		encoding: "utf8",
		maxBuffer: 128 * 1024 * 1024,
	});
	writeFileSync(stderrOutput, result.stderr ?? "");
	if (result.stdout) writeFileSync(output, result.stdout);
	if (result.error) throw result.error;
	let report;
	try {
		report = JSON.parse(result.stdout);
	} catch {
		throw new Error(
			`benchmark did not produce JSON; inspect ${relative(repositoryRoot, stderrOutput)}`,
		);
	}
	const summary = summarizeHeadlessStress(report, options.fixtures);
	const rate = summary.targetMet ? "TARGET MET" : "TARGET MISSED (NON-BLOCKING)";
	console.log(
		`${rate}: ${summary.averageHz.toFixed(2)} Hz average, ` +
			`${summary.minimumHz.toFixed(2)} Hz minimum, ${summary.targetHz} Hz target`,
	);
	console.log(
		`${summary.fixtureCount} fixtures / ${summary.physicalInstances} instances, ` +
			`${summary.dynamicCount} Dynamics, ${summary.universes} universes, ` +
			`${summary.occupiedSlots} occupied slots`,
	);
	console.log(
		`Dropped ${summary.dropped}; deferred ${summary.deferred}; ` +
			`deadline misses ${summary.deadlineMisses}`,
	);
	console.log(
		`Post-run resident memory: ${
			summary.residentBytes == null
				? "unavailable"
				: `${(summary.residentBytes / 1024 / 1024).toFixed(1)} MiB`
		}`,
	);
	console.log(`Raw report: ${relative(repositoryRoot, output)}`);
	console.log(`Build/run log: ${relative(repositoryRoot, stderrOutput)}`);
	if (result.status !== 0 || !summary.valid) process.exitCode = 1;
}

const isMain =
	process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
	try {
		main();
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		process.exitCode = 1;
	}
}
