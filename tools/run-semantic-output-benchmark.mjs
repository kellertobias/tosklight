#!/usr/bin/env node
// TL-596: paired, identified runs of the output benchmark through the production Live
// transaction, and the explicit gate evaluation over them.
//
// Suites (alternated round by round on one host, never compared across sessions):
// - legacy:   the established light-benchmark gates, baseline binary against candidate binary.
// - capacity: the same capacity workloads with typed semantic lanes (`--semantic`), alternated
//             with the candidate's own scalar control.
// - workload: the TL-564 semantic workload: output rate x tracking rate x dirty scenario, the
//             static-bases memo gates, readout consumers and publication off.
// The runner never builds: build commands are in docs/engineering/semantic-performance-workloads.md.
// Missing evidence is `unavailable(reason)`, never zero. `acceptance.granted` stays false; the
// gate table is a separate, explicit evaluation (`evaluateGates`).
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { normalizeSeed, semanticWorkloadDirectory } from "./semantic-performance-contract.mjs";
import { createSyntheticSemanticPatch, DEFAULT_SEMANTIC_RIG, loadFixtureLibrary } from "./semantic-performance-fixtures.mjs";
import {
	collectBuildIdentity,
	collectHostIdentity,
	createSemanticPerformanceReport,
	measured,
	unavailable,
	writeSemanticPerformanceReport,
} from "./semantic-performance-report.mjs";
import { collectSourceManifest } from "./semantic-source-manifest.mjs";
import { buildSemanticPerformanceWorkload, validateSemanticWorkload, writeSemanticWorkload } from "./semantic-performance-workload.mjs";

export const TRACKING_SCENARIOS = Object.freeze(["static-points", "small-subset", "all-points-move"]);
export const LEGACY_CASES = Object.freeze([
	{ key: "stress-2000", args: ["--transport", "encode-only", "--headless-stress-fixtures", "2000"], seconds: 8 },
	{ key: "stress-4000", args: ["--transport", "encode-only", "--headless-stress-fixtures", "4000"], seconds: 8 },
	{ key: "hard-floor-4148-125hz", args: ["--profile", "hard-floor", "--transport", "loopback", "--rate-hz", "125", "--sustained-show"], seconds: 12 },
]);

// ---------------------------------------------------------------------------------------------
// Pure summaries (tested)

export function median(values) {
	const finite = values.filter(Number.isFinite).sort((left, right) => left - right);
	if (finite.length === 0) return null;
	const middle = Math.floor(finite.length / 2);
	return finite.length % 2 ? finite[middle] : (finite[middle - 1] + finite[middle]) / 2;
}

const ms = (distribution, key) => (distribution ? distribution[key] / 1_000 : null);

/** The numbers one benchmark scenario contributes to the summaries. */
export function summarizeScenario(report) {
	const scenario = report.scenarios[0];
	const semantic = scenario.semantic ?? null;
	const work = semantic?.work;
	return {
		profile: scenario.profile,
		fixtures: scenario.fixture_count,
		rateHz: scenario.configured_rate_hz,
		metRate: scenario.met_configured_rate,
		requiredFloorMet: report.required_floor_met,
		averageHz: scenario.frame_rate.average_completed_hz,
		minimumHz: scenario.frame_rate.minimum_one_second_completed_hz,
		deadlineMisses: scenario.deadline.deadline_misses,
		droppedTicks: scenario.deadline.dropped_ticks,
		deferredTicks: scenario.deadline.deferred_ticks,
		completedTicks: scenario.completed_ticks,
		pipelineP50Ms: ms(scenario.phases.total_pipeline, "p50_microseconds"),
		pipelineP99Ms: ms(scenario.phases.total_pipeline, "p99_microseconds"),
		pipelineMaxMs: ms(scenario.phases.total_pipeline, "maximum_microseconds"),
		residentBytes: report.process_resources?.resident_bytes ?? null,
		semantic: semantic && {
			familyEngaged: semantic.family_engaged,
			frames: semantic.frames,
			hybridFrames: semantic.hybrid_frames,
			captureP99Ms: ms(semantic.phases.prepared_capture, "p99_microseconds"),
			transactionP50Ms: ms(semantic.phases.dynamics_and_family_transaction, "p50_microseconds"),
			transactionP99Ms: ms(semantic.phases.dynamics_and_family_transaction, "p99_microseconds"),
			publicationP99Ms: ms(semantic.phases.publication, "p99_microseconds"),
			trackingToOutputP95Ms: ms(semantic.tracking?.tracking_to_output, "p95_microseconds"),
			trackingSamples: semantic.tracking?.installed_samples ?? null,
			positionFitsP50: work.position_fits?.p50 ?? null,
			positionFitsP99: work.position_fits?.p99 ?? null,
			positionHitsP50: work.position_fit_cache_hits?.p50 ?? null,
			colorFitsP50: work.color_fits_and_refits?.p50 ?? null,
			colorReusesP50: work.color_result_reuses?.p50 ?? null,
			opticsResolvesP50: work.optics_resolves?.p50 ?? null,
			changedPointsP95: work.tracking_changed_points?.p95 ?? null,
			dirtyInstancesP50: work.tracking_dirty_instances?.p50 ?? null,
			dirtyInstancesP95: work.tracking_dirty_instances?.p95 ?? null,
			compilesInWindow: work.descriptor_and_fitter_compiles_in_window,
			generationChanges: work.generation_changes_in_window,
			expectedDirtyTargets: semantic.workload.expected_dirty_targets,
			consumers: semantic.readout_consumers?.report ?? null,
			perFrame: perFrameWork(work),
		},
	};
}

function perFrameWork(work) {
	const mean = (entry) => (entry ? entry.total / entry.frames : null);
	return {
		positionFits: mean(work.position_fits),
		positionHits: mean(work.position_fit_cache_hits),
		colorFits: mean(work.color_fits_and_refits),
		// TL-553: a Color resolve either fits or replays an unchanged fit; how many replay depends
		// on the sampled Dynamics timeline, so readout consumers are compared by resolves.
		colorResolves: mean(work.color_resolves),
		opticsResolves: mean(work.optics_resolves),
	};
}

/** Medians of the alternated rounds of one case and side. */
export function medianSummary(runs) {
	const pick = (select) => median(runs.map(select));
	return {
		rounds: runs.length,
		pipelineP50Ms: pick((run) => run.pipelineP50Ms),
		pipelineP99Ms: pick((run) => run.pipelineP99Ms),
		minimumHz: pick((run) => run.minimumHz),
		deadlineMisses: runs.map((run) => run.deadlineMisses),
		droppedTicks: runs.map((run) => run.droppedTicks),
		metRate: runs.map((run) => run.metRate),
		transactionP99Ms: pick((run) => run.semantic?.transactionP99Ms ?? Number.NaN),
	};
}

/** Paired scheduler p99 regression allowance from tools/run-packaged-stage-benchmark.mjs. */
export function allowedP99RegressionMs(baselineP99Ms) {
	return Math.max(1, baselineP99Ms * 0.05);
}

const gate = (name, source, threshold, observed, passed) => ({
	gate: name,
	source,
	threshold,
	observed,
	status: passed === null ? "unavailable" : passed ? "pass" : "fail",
});

function legacyGates(legacy) {
	return Object.entries(legacy ?? {}).flatMap(([key, sides]) => {
		const baseline = sides.baseline && medianSummary(sides.baseline);
		const candidate = sides.candidate && medianSummary(sides.candidate);
		if (!baseline || !candidate) return [gate(`${key}: paired p99`, "engine-render-performance-series.md", "max(1 ms, 5%)", null, null)];
		const allowance = allowedP99RegressionMs(baseline.pipelineP99Ms);
		const regression = candidate.pipelineP99Ms - baseline.pipelineP99Ms;
		const rows = [
			gate(`${key}: paired median p99 regression`, "run-packaged-stage-benchmark.mjs:2744-2775", `<= ${allowance.toFixed(2)} ms`, `${regression.toFixed(2)} ms (${baseline.pipelineP99Ms.toFixed(2)} -> ${candidate.pipelineP99Ms.toFixed(2)})`, regression <= allowance),
		];
		if (key.startsWith("hard-floor"))
			rows.push(gate(`${key}: configured rate held every round (candidate)`, "run-sustained-output-benchmark.mjs:36-63", "every round", candidate.metRate.join(","), candidate.metRate.every(Boolean)));
		return rows;
	});
}

function capacityGates(capacity) {
	return Object.entries(capacity ?? {}).flatMap(([key, sides]) => {
		const semantic = sides.semantic && medianSummary(sides.semantic);
		if (!semantic) return [];
		return [
			gate(`${key} (typed lanes): configured rate held every round`, "STAGE-PERF-001 / plan §11", "every round", semantic.metRate.join(","), semantic.metRate.every(Boolean)),
			gate(`${key} (typed lanes): 0 deadline misses`, "STAGE-PERF-001 / plan §11", "0 per round", semantic.deadlineMisses.join(","), semantic.deadlineMisses.every((misses) => misses === 0)),
		];
	});
}

function workloadGates(workload) {
	const rows = [];
	for (const run of workload?.matrix ?? []) {
		const label = `TL-564 ${run.scenario} tracking ${run.trackingHz} Hz / output ${run.outputHz} Hz`;
		rows.push(gate(`${label}: rate held, 0 deadline misses`, "STAGE-PERF-001 / plan §11", "met and 0", `${run.summary.metRate} / ${run.summary.deadlineMisses}`, run.summary.metRate && run.summary.deadlineMisses === 0));
		rows.push(gate(`${label}: no generation change or compile from motion`, "plan §10 item 4", "0 / 0", `${run.summary.semantic.generationChanges} / ${run.summary.semantic.compilesInWindow}`, run.summary.semantic.generationChanges === 0 && run.summary.semantic.compilesInWindow === 0));
	}
	for (const run of workload?.staticBases ?? []) {
		const semantic = run.summary.semantic;
		const label = `TL-564 static bases ${run.scenario}`;
		if (run.scenario === "static-points") {
			rows.push(gate(`${label}: 0 Position fits per frame (all memo hits)`, "plan §10/§11", "p99 0", semantic.positionFitsP99, semantic.positionFitsP99 === 0));
			rows.push(gate(`${label}: unchanged Color targets skip solves`, "TL-596 scope (unchanged targets skip solves)", "p50 0", semantic.colorFitsP50, semantic.colorFitsP50 === 0));
		} else {
			rows.push(gate(`${label}: dirty instances equal the manifest's dependents`, "tracking_bench.rs:703-760 shape", String(semantic.expectedDirtyTargets), semantic.dirtyInstancesP95, semantic.dirtyInstancesP50 === semantic.expectedDirtyTargets && semantic.dirtyInstancesP95 === semantic.expectedDirtyTargets));
			rows.push(gate(`${label}: Position fits equal the dependents`, "tracking_bench.rs:703-760 shape", String(semantic.expectedDirtyTargets), semantic.positionFitsP99, semantic.positionFitsP99 === semantic.expectedDirtyTargets));
		}
	}
	const consumers = workload?.consumers ?? [];
	const reference = consumers.find((run) => run.consumers === 0);
	for (const run of consumers.filter((entry) => entry.consumers > 0)) {
		const same = reference && ["positionFits", "colorResolves", "opticsResolves"].every((key) => Math.abs(run.summary.semantic.perFrame[key] - reference.summary.semantic.perFrame[key]) < 1e-9);
		rows.push(gate(`readout consumers x${run.consumers} (hold ${run.holdMs} ms): physical solves per frame unchanged`, "TL-596 scope", "equal to 0 consumers", JSON.stringify(run.summary.semantic.perFrame), reference ? same : null));
	}
	return rows;
}

/** The explicit gate table. Thresholds come only from existing documents; no new budget. */
export function evaluateGates(summary) {
	return [...legacyGates(summary.legacy), ...capacityGates(summary.capacity), ...workloadGates(summary.workload)];
}

/** TL-564 report evidence from one workload run. Missing data stays unavailable. */
export function workloadEvidence(run, raw) {
	const semantic = run.summary.semantic;
	const scenario = raw.scenarios[0];
	const provenance = (counter) => ({ source: "light-benchmark --semantic-workload (TL-596)", counter });
	const seconds = scenario.warmup_elapsed_seconds + scenario.elapsed_seconds;
	return {
		source: "measured-run",
		productionSemanticSupport: semantic.familyEngaged
			? { status: "active", provenance: provenance("LiveOutputBench::family_engaged (production opt-in, contract 1)") }
			: unavailable("the family adapters were not engaged"),
		output: {
			observedOutputHz: measured(scenario.frame_rate.average_completed_hz, "Hz", provenance("frame_rate.average_completed_hz")),
			schedulerP99Ms: measured(run.summary.pipelineP99Ms, "ms", provenance("phases.total_pipeline.p99: capture + Dynamics/family transaction + publication + encode; benchmark pacing, not the production scheduler")),
			outputDeadlineMisses: measured(scenario.deadline.deadline_misses, "count", provenance("deadline.deadline_misses (completed after its interval)")),
			outputSendErrors: unavailable("encode-only transport: no datagram is sent"),
			trackingSampleHz: semantic.trackingSamples === null
				? unavailable("no tracking stream in this run")
				: measured(semantic.trackingSamples / seconds, "Hz", provenance("installed receiver samples / run seconds; injected at the engine boundary at output ticks")),
			trackingToOutputP95Ms: semantic.trackingToOutputP95Ms === null
				? unavailable("no tracking stream in this run")
				: measured(semantic.trackingToOutputP95Ms, "ms", provenance("semantic.tracking.tracking_to_output.p95")),
		},
		semantic: {
			transformAimP99Ms: unavailable("the Live transaction has no separate transform+aim phase; see the family transaction p99 in the raw run"),
			redundantStaticAimSolves: run.staticBases && run.scenario === "static-points"
				? measured(semantic.positionFitsP99, "fits/frame", provenance("semantic.work.position_fits.p99 with static bases only"))
				: unavailable("Position Dynamics re-solve every frame by design; measured by the static-bases-only runs"),
			dirtyTargetsPerFrameP95: semantic.dirtyInstancesP95 === null
				? unavailable("no tracking census was accepted")
				: measured(semantic.dirtyInstancesP95, "instances/frame", provenance("semantic.work.tracking_dirty_instances.p95")),
			generationRebuildsFromMotion: measured(semantic.generationChanges, "count", provenance("RenderResult.generation changes in the measured window")),
			portableShowWritesFromMotion: unavailable("the benchmark seam holds no ShowStore; tracking enters only through Engine::set_tracking_frame"),
		},
		nativeStage: Object.fromEntries(
			["presentationHz", "sourceToPresentedP95Ms", "changingFramePresentationGapMaxMs", "lateCpuFrameP95Ms"].map((name) => [name, unavailable("no native Stage in the headless output benchmark")]),
		),
	};
}

// ---------------------------------------------------------------------------------------------
// Running

function parseArguments(argv) {
	const options = { rounds: 3, seed: "596", suites: ["legacy", "capacity", "workload"], seconds: 4, outputHz: [44, 60, 125], trackingHz: [30, 60, 120] };
	for (let index = 0; index < argv.length; index += 1) {
		const flag = argv[index];
		const value = () => {
			index += 1;
			if (argv[index] === undefined) throw new Error(`${flag} needs a value`);
			return argv[index];
		};
		if (flag === "--candidate-binary") options.candidateBinary = path.resolve(value());
		else if (flag === "--candidate-root") options.candidateRoot = path.resolve(value());
		else if (flag === "--baseline-binary") options.baselineBinary = path.resolve(value());
		else if (flag === "--baseline-root") options.baselineRoot = path.resolve(value());
		else if (flag === "--rounds") options.rounds = Number(value());
		else if (flag === "--seconds") options.seconds = Number(value());
		else if (flag === "--seed") options.seed = value();
		else if (flag === "--suites") options.suites = value().split(",");
		else if (flag === "--output-hz") options.outputHz = value().split(",").map(Number);
		else if (flag === "--tracking-hz") options.trackingHz = value().split(",").map(Number);
		else if (flag === "--out") options.out = path.resolve(value());
		else if (flag === "--candidate-source-manifest") options.candidateManifest = path.resolve(value());
		else if (flag === "--baseline-source-manifest") options.baselineManifest = path.resolve(value());
		else throw new Error(`unknown argument ${flag}`);
	}
	if (!options.candidateBinary) throw new Error("--candidate-binary is required");
	return options;
}

function runBenchmark(binary, cwd, args, file) {
	const result = spawnSync(binary, ["--protocol", "both", ...args, "--fixture-package-dir", "assets/fixture-library"], {
		cwd,
		encoding: "utf8",
		maxBuffer: 256 * 1024 * 1024,
	});
	fs.writeFileSync(file.replace(/\.json$/u, ".stderr.log"), result.stderr ?? "");
	if (!result.stdout) throw new Error(`${path.basename(binary)} ${args.join(" ")} produced no JSON (exit ${result.status}); see ${file}`);
	fs.writeFileSync(file, result.stdout);
	return JSON.parse(result.stdout);
}

function identity(root, binary, outDirectory, side, supplied) {
	// Captured at build time when supplied, otherwise once before any run: concurrent edits
	// must not re-describe a measured binary.
	const sourceManifest = supplied ? JSON.parse(fs.readFileSync(supplied, "utf8")) : collectSourceManifest({ root });
	fs.writeFileSync(path.join(outDirectory, `${side}-source-manifest.json`), `${JSON.stringify(sourceManifest, null, "\t")}\n`);
	return collectBuildIdentity({ repositoryRoot: root, binaryPath: binary, buildProfile: "release --locked --no-default-features", sourceManifest });
}

function runLegacy(options, out, rounds) {
	const legacy = {};
	for (let round = 1; round <= rounds; round += 1)
		for (const side of ["baseline", "candidate"])
			for (const entry of LEGACY_CASES) {
				const binary = side === "baseline" ? options.baselineBinary : options.candidateBinary;
				const cwd = side === "baseline" ? options.baselineRoot : options.candidateRoot;
				const raw = runBenchmark(binary, cwd, [...entry.args, "--seconds", String(entry.seconds), "--warmup-seconds", "1"], path.join(out, `legacy-${entry.key}-${side}-r${round}.json`));
				((legacy[entry.key] ??= {})[side] ??= []).push(summarizeScenario(raw));
				console.log(`legacy ${entry.key} ${side} r${round}: p99 ${summarizeScenario(raw).pipelineP99Ms?.toFixed(2)} ms`);
			}
	return legacy;
}

function runCapacity(options, out, rounds) {
	const capacity = {};
	for (let round = 1; round <= rounds; round += 1)
		for (const entry of LEGACY_CASES)
			for (const side of ["control", "semantic"]) {
				const args = [...entry.args, "--seconds", String(Math.min(entry.seconds, 6)), "--warmup-seconds", "2", ...(side === "semantic" ? ["--semantic"] : [])];
				const raw = runBenchmark(options.candidateBinary, options.candidateRoot, args, path.join(out, `capacity-${entry.key}-${side}-r${round}.json`));
				((capacity[entry.key] ??= {})[side] ??= []).push(summarizeScenario(raw));
				console.log(`capacity ${entry.key} ${side} r${round}: p99 ${summarizeScenario(raw).pipelineP99Ms?.toFixed(2)} ms`);
			}
	return capacity;
}

function prepareWorkload(seed) {
	const library = loadFixtureLibrary({ packages: [...new Set([...DEFAULT_SEMANTIC_RIG.map((entry) => entry.packageName), "tosklight--3d-point"])] });
	const patch = createSyntheticSemanticPatch({ seed: normalizeSeed(seed), library });
	const workload = buildSemanticPerformanceWorkload(patch, { seed });
	const errors = validateSemanticWorkload(workload);
	if (errors.length > 0) throw new Error(`workload is not contract-valid:\n${errors.join("\n")}`);
	return { patch, workload };
}

async function writeWorkloadInputs(patch, workload) {
	const { artifactPaths } = await import("./artifact-paths.mjs");
	const directory = semanticWorkloadDirectory(artifactPaths.performance, workload.manifest);
	writeSemanticWorkload(workload, directory, { emitTracking: true });
	const file = path.join(directory, "patch.json");
	const data = `${JSON.stringify(patch, null, "\t")}\n`;
	if (fs.existsSync(file) && fs.readFileSync(file, "utf8") !== data) throw new Error(`${file} holds a different patch`);
	fs.writeFileSync(file, data);
	return directory;
}

function workloadRun(options, directory, out, spec) {
	const args = [
		"--transport", "encode-only", "--seconds", String(options.seconds), "--warmup-seconds", "1",
		"--rate-hz", String(spec.outputHz), "--semantic-workload", directory,
		"--tracking-hz", String(spec.trackingHz), "--tracking-scenario", spec.scenario,
		...(spec.staticBases ? ["--static-bases-only"] : []),
		...(spec.consumers ? ["--readout-consumers", String(spec.consumers), "--slow-consumer-ms", String(spec.holdMs ?? 0)] : []),
		...(spec.noPublish ? ["--no-publish"] : []),
	];
	const name = `workload-${spec.label}.json`;
	const raw = runBenchmark(options.candidateBinary, options.candidateRoot, args, path.join(out, name));
	const run = { ...spec, file: name, summary: summarizeScenario(raw) };
	console.log(`workload ${spec.label}: p99 ${run.summary.pipelineP99Ms?.toFixed(2)} ms, misses ${run.summary.deadlineMisses}`);
	return { run, raw };
}

async function runWorkload(options, out, build, host) {
	const { patch, workload } = prepareWorkload(options.seed);
	const directory = await writeWorkloadInputs(patch, workload);
	const result = { inputs: directory, manifestSha256: workload.manifest.manifestSha256, matrix: [], staticBases: [], consumers: [], publication: [], reports: [] };
	const record = async (bucket, spec) => {
		const { run, raw } = workloadRun(options, directory, out, spec);
		result[bucket].push(run);
		const report = createSemanticPerformanceReport({ manifest: workload.manifest, build, host, evidence: workloadEvidence(run, raw) });
		result.reports.push({ label: spec.label, file: await writeSemanticPerformanceReport(report) });
	};
	for (const outputHz of options.outputHz)
		for (const trackingHz of options.trackingHz)
			for (const scenario of TRACKING_SCENARIOS)
				await record("matrix", { label: `${scenario}-t${trackingHz}-o${outputHz}`, scenario, trackingHz, outputHz });
	for (const scenario of TRACKING_SCENARIOS)
		await record("staticBases", { label: `static-bases-${scenario}-t60-o60`, scenario, trackingHz: 60, outputHz: 60, staticBases: true });
	for (const [consumers, holdMs] of [[0, 0], [4, 0], [4, 50]])
		await record("consumers", { label: `consumers-${consumers}-hold${holdMs}`, scenario: "small-subset", trackingHz: 120, outputHz: 60, consumers, holdMs });
	await record("publication", { label: "no-publish-small-subset-t120-o60", scenario: "small-subset", trackingHz: 120, outputHz: 60, noPublish: true });
	return result;
}

async function main() {
	const options = parseArguments(process.argv.slice(2));
	const { artifactPaths, repositoryRoot } = await import("./artifact-paths.mjs");
	options.candidateRoot ??= repositoryRoot;
	const out = options.out ?? path.join(artifactPaths.performance, "semantic-output", new Date().toISOString().replaceAll(":", "-"));
	fs.mkdirSync(out, { recursive: true });
	const host = collectHostIdentity();
	const candidate = identity(options.candidateRoot, options.candidateBinary, out, "candidate", options.candidateManifest);
	const baseline = options.baselineBinary ? identity(options.baselineRoot, options.baselineBinary, out, "baseline", options.baselineManifest) : null;
	const summary = { createdAt: new Date().toISOString(), host, identity: { candidate: stripManifest(candidate), baseline: baseline && stripManifest(baseline) }, rounds: options.rounds };
	if (options.suites.includes("legacy")) {
		if (!baseline || !options.baselineRoot) throw new Error("the legacy suite needs --baseline-binary and --baseline-root");
		summary.legacy = runLegacy(options, out, options.rounds);
	}
	if (options.suites.includes("capacity")) summary.capacity = runCapacity(options, out, options.rounds);
	if (options.suites.includes("workload")) summary.workload = await runWorkload(options, out, candidate, host);
	summary.gates = evaluateGates(summary);
	summary.acceptance = { granted: false, reason: "evidence and an explicit gate table only; acceptance stays with TL-553" };
	fs.writeFileSync(path.join(out, "summary.json"), `${JSON.stringify(summary, null, "\t")}\n`);
	for (const row of summary.gates) console.log(`${row.status.toUpperCase().padEnd(11)} ${row.gate}: ${row.observed ?? "unavailable"} (threshold ${row.threshold})`);
	console.log(`wrote ${out}`);
}

function stripManifest(build) {
	const { sourceManifest: _manifest, ...rest } = build;
	return rest;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
	main().catch((error) => {
		console.error(error.stack ?? error.message);
		process.exit(1);
	});
