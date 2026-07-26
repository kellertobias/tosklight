#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function parseArguments(arguments_) {
	const options = { seconds: 120 };
	for (let index = 0; index < arguments_.length; index += 2) {
		const option = arguments_[index];
		const value = arguments_[index + 1];
		if (!option?.startsWith("--") || value == null) {
			throw new Error(`invalid argument list near ${option ?? "<end>"}`);
		}
		if (option === "--seconds") {
			const seconds = Number(value);
			if (seconds !== 60 && seconds !== 120) {
				throw new Error("--seconds must be 60 or 120");
			}
			options.seconds = seconds;
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

export function summarizeSustainedOutput(report) {
	const scenario = report?.scenarios?.find(
		(candidate) => candidate.profile === "hard_floor",
	);
	if (!scenario?.frame_rate) {
		throw new Error("benchmark report has no hard-floor frame-rate evidence");
	}
	if (!scenario.fixture_inventory) {
		throw new Error("benchmark report has no fixture inventory");
	}
	return {
		passed:
			report.required_floor_met === true &&
			scenario.met_configured_rate === true,
		averageHz: scenario.frame_rate.average_completed_hz,
		minimumHz: scenario.frame_rate.minimum_one_second_completed_hz,
		requiredHz: scenario.frame_rate.required_minimum_hz,
		windows: scenario.frame_rate.one_second_windows,
		windowsBelowMinimum: scenario.frame_rate.windows_below_minimum,
		dropped: scenario.deadline.dropped_ticks,
		deferred: scenario.deadline.deferred_ticks,
		deadlineMisses: scenario.deadline.deadline_misses,
		fixtureCount: scenario.fixture_count,
		manufacturerFixtureSlots:
			scenario.fixture_inventory.manufacturer_fixture_slots,
		rgbParFillSlots: scenario.fixture_inventory.rgb_par_fill_slots,
		totalSlots: scenario.fixture_inventory.total_slots,
	};
}

function defaultOutputPath() {
	const timestamp = new Date().toISOString().replaceAll(/[:.]/gu, "-");
	return resolve(
		repositoryRoot,
		".artifacts/performance",
		`sustained-output-${timestamp}.json`,
	);
}

function main() {
	const options = parseArguments(process.argv.slice(2));
	const output = resolve(repositoryRoot, options.output ?? defaultOutputPath());
	const stderrOutput = output.replace(/\.json$/u, ".stderr.log");
	mkdirSync(dirname(output), { recursive: true });

	const benchmarkArguments = [
		"run",
		"--release",
		"--locked",
		"--no-default-features",
		"-p",
		"light-headless",
		"--bin",
		"light-benchmark",
		"--",
		"--profile",
		"hard-floor",
		"--protocol",
		"both",
		"--transport",
		"loopback",
		"--seconds",
		String(options.seconds),
		"--warmup-seconds",
		"1",
		"--rate-hz",
		"125",
		"--demo-show",
		"--fixture-package-dir",
		"assets/fixture-library",
	];
	if (options.hardwareLabel) {
		benchmarkArguments.push("--hardware-label", options.hardwareLabel);
	}

	const result = spawnSync("cargo", benchmarkArguments, {
		cwd: repositoryRoot,
		encoding: "utf8",
		maxBuffer: 64 * 1024 * 1024,
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
	const summary = summarizeSustainedOutput(report);
	const status = summary.passed ? "PASS" : "FAIL";
	console.log(
		`${status}: average ${summary.averageHz.toFixed(2)} Hz; ` +
			`minimum ${summary.minimumHz.toFixed(2)} Hz across ${summary.windows} one-second windows; ` +
			`required ${summary.requiredHz} Hz`,
	);
	console.log(
		`Dropped ${summary.dropped}; deferred ${summary.deferred}; ` +
			`deadline misses ${summary.deadlineMisses}; windows below minimum ${summary.windowsBelowMinimum}`,
	);
	console.log(
		`${summary.fixtureCount} fixtures: ${summary.manufacturerFixtureSlots} manufacturer-fixture slots + ` +
			`${summary.rgbParFillSlots} RGB PAR fill slots = ${summary.totalSlots} total`,
	);
	console.log(`Raw report: ${relative(repositoryRoot, output)}`);
	console.log(`Build/run log: ${relative(repositoryRoot, stderrOutput)}`);
	if (result.status !== 0 || !summary.passed) process.exitCode = 1;
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
