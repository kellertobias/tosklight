// Evidence reports for semantic performance workloads (TL-564). A report binds a workload
// manifest to binary, build and host identity and to the counters a runner actually supplied.
// It never grants acceptance: synthetic input is "not-evaluated", missing counters are
// "unavailable" (never zero), and output and native Stage claims stay separate. There is no
// browser Stage; browser-Stage evidence is rejected. Threshold gates remain with TL-548/TL-553.
// TL-604: build identity also carries a source manifest digest, because HEAD plus a dirty flag
// cannot tell two different uncommitted or untracked implementations apart.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
	fileSha256,
	semanticContractRepositoryRoot,
	semanticWorkloadDirectory,
	sha256,
} from "./semantic-performance-contract.mjs";
import {
	collectSourceManifest,
	loadSuppliedSourceManifest,
	ownGitCheckout,
	sourceIdentity,
	sourceManifestErrors,
} from "./semantic-source-manifest.mjs";

export const SEMANTIC_REPORT_VERSION = "tosklight.semantic-performance-report/1";

/** Counters each claim needs before it can even be handed to an acceptance gate. */
export const REQUIRED_EVIDENCE = Object.freeze({
	output: Object.freeze([
		"observedOutputHz",
		"schedulerP99Ms",
		"outputDeadlineMisses",
		"outputSendErrors",
		"trackingSampleHz",
		"trackingToOutputP95Ms",
	]),
	nativeStage: Object.freeze([
		"presentationHz",
		"sourceToPresentedP95Ms",
		"changingFramePresentationGapMaxMs",
		"lateCpuFrameP95Ms",
	]),
	semantic: Object.freeze([
		"transformAimP99Ms",
		"redundantStaticAimSolves",
		"dirtyTargetsPerFrameP95",
		"generationRebuildsFromMotion",
		"portableShowWritesFromMotion",
	]),
});

export const CLAIM_STATES = Object.freeze([
	"not-evaluated",
	"incomplete",
	"evidence-complete",
]);

export function unavailable(reason) {
	return { status: "unavailable", reason };
}

export function isUnavailable(entry) {
	return entry?.status === "unavailable";
}

/** A measured value; provenance names the runner, counter and how it was obtained. */
export function measured(value, unit, provenance) {
	if (typeof value !== "number" || !Number.isFinite(value))
		throw new Error("measured evidence needs a finite number; use unavailable() for missing data");
	if (!provenance?.source || !provenance?.counter)
		throw new Error("measured evidence needs provenance.source and provenance.counter");
	return { status: "measured", value, unit, provenance };
}

// ---------------------------------------------------------------------------------------------
// Identity

function run(command, args, cwd) {
	const result = spawnSync(command, args, { cwd, encoding: "utf8", timeout: 5_000 });
	return result.status === 0 ? result.stdout.trim() : null;
}

/**
 * Git, source and binary identity. Anything that cannot be read is explicitly unavailable.
 * `sourceManifest` accepts an explicitly supplied immutable snapshot manifest (object or JSON file)
 * whose digest is validated; otherwise the sources below `repositoryRoot` are hashed. The full
 * manifest is returned as `sourceManifest` and is retained beside the report by the writer.
 */
export function collectBuildIdentity({ repositoryRoot = semanticContractRepositoryRoot, binaryPath, buildProfile, sourceManifest } = {}) {
	// A checkout nested in another repository (e.g. a snapshot under .artifacts) must not
	// borrow the enclosing repository's HEAD.
	const ownsCheckout = ownGitCheckout(repositoryRoot).owns;
	const head = ownsCheckout ? run("git", ["rev-parse", "HEAD"], repositoryRoot) : null;
	const status = head === null ? null : run("git", ["status", "--porcelain", "--untracked-files=no"], repositoryRoot);
	const manifest = sourceManifest === undefined ? collectSourceManifest({ root: repositoryRoot }) : loadSuppliedSourceManifest(sourceManifest);
	let binary;
	if (!binaryPath) binary = unavailable("no binary path supplied; synthetic or builder-only run");
	else if (!fs.existsSync(binaryPath)) binary = unavailable(`binary ${binaryPath} does not exist`);
	else
		binary = {
			path: binaryPath,
			sha256: fileSha256(binaryPath),
			bytes: fs.statSync(binaryPath).size,
		};
	return {
		gitHead: head ?? unavailable("git HEAD unavailable: the repository root is not the top level of a readable git checkout"),
		gitTrackedChanges: status === null ? unavailable("git status is not readable here") : status.length > 0,
		source: sourceIdentity(manifest, sourceManifest === undefined ? "collected" : "supplied-snapshot-manifest"),
		binary,
		buildProfile: buildProfile ?? unavailable("build profile not supplied"),
		...(manifest.status === "recorded" ? { sourceManifest: manifest } : {}),
	};
}

export function collectHostIdentity() {
	const cpus = os.cpus();
	return {
		platform: os.platform(),
		release: os.release(),
		arch: os.arch(),
		cpuModel: cpus[0]?.model?.trim() || unavailable("CPU model not reported by the OS"),
		logicalCpus: cpus.length || unavailable("CPU count not reported by the OS"),
		totalMemoryBytes: os.totalmem(),
		node: process.version,
		gpu: unavailable("GPU identity is not collected by the builder; the native Stage runner must supply it"),
	};
}

// ---------------------------------------------------------------------------------------------
// Report

function normalizeMetric(name, entry) {
	if (entry === undefined || entry === null)
		return unavailable(`${name} was not supplied by the runner`);
	if (isUnavailable(entry)) return entry;
	if (entry.status === "measured") return measured(entry.value, entry.unit, entry.provenance);
	throw new Error(`${name} must be measured(...) or unavailable(...), not a bare value`);
}

function claimFrom(kind, source, metrics, blockers) {
	const missing = Object.entries(metrics).filter(([, entry]) => isUnavailable(entry)).map(([name]) => name);
	if (source !== "measured-run")
		return { state: "not-evaluated", reason: `${source} input cannot establish ${kind} acceptance`, missing };
	if (missing.length > 0 || blockers.length > 0)
		return { state: "incomplete", reason: [...(missing.length ? [`missing ${missing.join(", ")}`] : []), ...blockers].join("; "), missing };
	return {
		state: "evidence-complete",
		reason: "all required counters are present; thresholds are evaluated by the TL-548/TL-553 acceptance gate, not by this report",
		missing,
	};
}

/**
 * Create a report. `evidence.source` is "synthetic" (default) or "measured-run". Measured runs
 * must supply every counter through measured()/unavailable(); omitted counters are unavailable.
 */
export function createSemanticPerformanceReport({ manifest, build, host, evidence = {}, createdAt = new Date().toISOString() }) {
	if (!manifest?.workloadVersion || !manifest?.manifestSha256)
		throw new Error("a report needs a semantic workload manifest");
	if (evidence.browserStage !== undefined || evidence.nativeStage?.kind === "browser")
		throw new Error("browser Stage evidence is not accepted: there is no browser Stage, only native Stage");
	const source = evidence.source ?? "synthetic";
	if (!["synthetic", "measured-run"].includes(source))
		throw new Error(`unknown evidence source ${source}`);
	const metrics = Object.fromEntries(
		Object.entries(REQUIRED_EVIDENCE).map(([group, names]) => [
			group,
			Object.fromEntries(names.map((name) => [name, normalizeMetric(`${group}.${name}`, evidence[group]?.[name])])),
		]),
	);
	const support = evidence.productionSemanticSupport ?? manifest.productionSupport?.semanticProgrammingContract ?? unavailable("not reported");
	const semanticBlockers = support.status === "active" ? [] : [`semantic production support is ${support.status}`];
	const outputClaim = claimFrom("output", source, { ...metrics.output, ...metrics.semantic }, semanticBlockers);
	const stageClaim = claimFrom("native Stage", source, metrics.nativeStage, semanticBlockers);
	const { sourceManifest, ...buildIdentity } = build ?? {};
	if (sourceManifest !== undefined) {
		const errors = sourceManifestErrors(sourceManifest);
		if (errors.length > 0) throw new Error(`build source manifest is invalid: ${errors.join("; ")}`);
		if (buildIdentity.source?.sourceSha256 !== sourceManifest.sourceSha256)
			throw new Error("build source identity does not match the attached source manifest");
	}
	const report = {
		reportVersion: SEMANTIC_REPORT_VERSION,
		createdAt,
		evidenceSource: source,
		identity: {
			build: build ? buildIdentity : unavailable("build identity not collected"),
			host: host ?? unavailable("host identity not collected"),
			workload: {
				version: manifest.workloadVersion,
				id: manifest.workloadId,
				seed: manifest.seed,
				manifestSha256: manifest.manifestSha256,
				identitySource: manifest.identitySource,
			},
		},
		productionSemanticSupport: support,
		rates: {
			configured: {
				trackingHz: manifest.rates.trackingHz,
				outputHz: manifest.rates.outputHz,
				nativeStageTargetHz: evidence.nativeStageTargetHz ?? unavailable("native Stage target rate not supplied"),
			},
			observed: {
				trackingSampleHz: metrics.output.trackingSampleHz,
				outputHz: metrics.output.observedOutputHz,
				nativeStagePresentationHz: metrics.nativeStage.presentationHz,
			},
		},
		metrics,
		workloadCounts: manifest.counts,
		workloadShortfalls: manifest.shortfalls,
		workloadLimitations: manifest.limitations,
		claims: { output: outputClaim, nativeStage: stageClaim },
		acceptance: {
			granted: false,
			reason: "this report records evidence only; output and native Stage acceptance are separate TL-548/TL-553 gate decisions",
		},
	};
	report.reportSha256 = sha256({ ...report, createdAt: undefined });
	// The full manifest travels with the report object but outside its JSON and digest: the
	// digest already binds `identity.build.source.sourceSha256`, and the writer retains the
	// manifest once per digest beside the report.
	if (sourceManifest !== undefined) Object.defineProperty(report, "sourceManifest", { value: sourceManifest, enumerable: false });
	return report;
}

/** File name of a retained source manifest. Content-addressed, so it is never rewritten. */
export function sourceManifestFileName(sourceSha256) {
	return `source-manifest-${sourceSha256}.json`;
}

/** Write `data` to a new file; an existing file is kept when identical and refused otherwise. */
function writeOnce(file, data, what) {
	try {
		fs.writeFileSync(file, data, { flag: "wx" });
	} catch (error) {
		if (error.code !== "EEXIST") throw error;
		if (fs.readFileSync(file, "utf8") !== data)
			throw new Error(`refusing to overwrite prior ${what} evidence in ${file}`);
	}
}

/**
 * Write a report next to its manifest under a canonical `.artifacts/performance` directory, or into
 * an explicit `directory`. Prior evidence is never overwritten: report names carry the creation time
 * and report digest and are created exclusively, the source manifest is content-addressed, and a
 * directory that holds a different workload manifest (TL-580 ownership) is refused.
 */
export async function writeSemanticPerformanceReport(report, directory, { sourceManifest = report.sourceManifest } = {}) {
	let target = directory;
	if (!target) {
		const { artifactPaths } = await import("./artifact-paths.mjs");
		target = semanticWorkloadDirectory(artifactPaths.performance, {
			workloadId: report.identity.workload.id,
			manifestSha256: report.identity.workload.manifestSha256,
		});
	}
	const workloadManifest = path.join(target, "manifest.json");
	if (fs.existsSync(workloadManifest)) {
		const stored = JSON.parse(fs.readFileSync(workloadManifest, "utf8")).manifestSha256;
		if (stored !== report.identity.workload.manifestSha256)
			throw new Error(`refusing to write a report for manifest ${report.identity.workload.manifestSha256} into ${target}: it holds manifest ${stored}`);
	}
	const source = report.identity.build?.source;
	if (sourceManifest !== undefined && sourceManifest.sourceSha256 !== source?.sourceSha256)
		throw new Error("the source manifest does not belong to this report");
	fs.mkdirSync(target, { recursive: true });
	if (sourceManifest !== undefined)
		writeOnce(path.join(target, sourceManifestFileName(sourceManifest.sourceSha256)), `${JSON.stringify(sourceManifest, null, "\t")}\n`, "source manifest");
	const file = path.join(target, `report-${report.createdAt.replaceAll(":", "-")}-${report.reportSha256.slice(0, 16)}.json`);
	writeOnce(file, `${JSON.stringify(report, null, "\t")}\n`, "report");
	return file;
}
