import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import {
	REQUIRED_EVIDENCE,
	collectBuildIdentity,
	collectHostIdentity,
	createSemanticPerformanceReport,
	measured,
	unavailable,
	writeSemanticPerformanceReport,
} from "./semantic-performance-report.mjs";
import { buildSyntheticSemanticWorkload } from "./semantic-performance-workload.mjs";

const { manifest } = buildSyntheticSemanticWorkload({ seed: 548, request: { trackingDurationSeconds: 0.2 } });
const createdAt = "2026-09-30T00:00:00.000Z";
const provenance = (counter) => ({ source: "tl-548-runner", counter, method: "unit-test fixture" });
const fullEvidence = (overrides = {}) =>
	Object.fromEntries(
		Object.entries(REQUIRED_EVIDENCE).map(([group, names]) => [
			group,
			Object.fromEntries(names.map((name) => [name, overrides[name] ?? measured(1, "unit", provenance(name))])),
		]),
	);

test("synthetic workloads can never claim output or native Stage acceptance", () => {
	const report = createSemanticPerformanceReport({ manifest, build: collectBuildIdentity(), host: collectHostIdentity(), createdAt });
	assert.equal(report.evidenceSource, "synthetic");
	assert.equal(report.claims.output.state, "not-evaluated");
	assert.equal(report.claims.nativeStage.state, "not-evaluated");
	assert.equal(report.acceptance.granted, false);
	assert.deepEqual(Object.keys(report.claims).sort(), ["nativeStage", "output"]);
	assert.ok(!JSON.stringify(report).includes('"pass"'));
	const fakeRun = createSemanticPerformanceReport({ manifest, createdAt, evidence: { ...fullEvidence(), productionSemanticSupport: { status: "active" } } });
	assert.equal(fakeRun.claims.output.state, "not-evaluated", "counters on synthetic input still do not count");
});

test("missing counters stay unavailable and make a measured run incomplete", () => {
	const report = createSemanticPerformanceReport({
		manifest,
		createdAt,
		evidence: { source: "measured-run", output: { observedOutputHz: measured(59.8, "Hz", provenance("output.hz")) } },
	});
	assert.equal(report.metrics.output.schedulerP99Ms.status, "unavailable");
	assert.notEqual(report.metrics.output.schedulerP99Ms.value, 0);
	assert.equal(report.claims.output.state, "incomplete");
	assert.ok(report.claims.output.missing.includes("schedulerP99Ms"));
	assert.equal(report.claims.nativeStage.state, "incomplete");
	assert.deepEqual(report.claims.nativeStage.missing, REQUIRED_EVIDENCE.nativeStage);
	assert.match(report.claims.output.reason, /semantic production support is unavailable/u);
});

test("configured targets and observed rates are separate fields", () => {
	const report = createSemanticPerformanceReport({
		manifest,
		createdAt,
		evidence: { source: "measured-run", output: { trackingSampleHz: measured(118.7, "Hz", provenance("psn.rx")) } },
	});
	assert.deepEqual(report.rates.configured.trackingHz, [30, 60, 120]);
	assert.deepEqual(report.rates.configured.outputHz, [44, 60, 125]);
	assert.equal(report.rates.observed.trackingSampleHz.value, 118.7);
	assert.equal(report.rates.observed.outputHz.status, "unavailable");
	assert.equal(report.rates.observed.nativeStagePresentationHz.status, "unavailable");
});

test("an explicit measured zero is kept distinct from a missing counter; bare numbers are rejected", () => {
	const zero = measured(0, "count", provenance("aim.redundant"));
	const report = createSemanticPerformanceReport({ manifest, createdAt, evidence: { source: "measured-run", semantic: { redundantStaticAimSolves: zero } } });
	assert.deepEqual(report.metrics.semantic.redundantStaticAimSolves, zero);
	assert.equal(report.metrics.semantic.generationRebuildsFromMotion.status, "unavailable");
	assert.throws(() => createSemanticPerformanceReport({ manifest, evidence: { source: "measured-run", output: { schedulerP99Ms: 0.4 } } }), /measured\(\.\.\.\) or unavailable/u);
	assert.throws(() => measured(Number.NaN, "ms", provenance("x")), /finite/u);
	assert.throws(() => measured(1, "ms", {}), /provenance/u);
});

test("browser Stage evidence is rejected", () => {
	assert.throws(() => createSemanticPerformanceReport({ manifest, evidence: { browserStage: {} } }), /no browser Stage/u);
	assert.throws(() => createSemanticPerformanceReport({ manifest, evidence: { nativeStage: { kind: "browser" } } }), /no browser Stage/u);
});

test("complete evidence is handed to the gate without granting acceptance", () => {
	const inactive = createSemanticPerformanceReport({ manifest, createdAt, evidence: { source: "measured-run", ...fullEvidence() } });
	assert.equal(inactive.claims.output.state, "incomplete", "inactive semantic production support blocks the claim");
	const active = createSemanticPerformanceReport({
		manifest,
		createdAt,
		evidence: { source: "measured-run", productionSemanticSupport: { status: "active", provenance: provenance("contract") }, ...fullEvidence() },
	});
	assert.equal(active.claims.output.state, "evidence-complete");
	assert.equal(active.claims.nativeStage.state, "evidence-complete");
	assert.equal(active.acceptance.granted, false);
	const noStage = createSemanticPerformanceReport({
		manifest,
		createdAt,
		evidence: { source: "measured-run", productionSemanticSupport: { status: "active" }, ...fullEvidence({ presentationHz: unavailable("Stage closed") }) },
	});
	assert.equal(noStage.claims.output.state, "evidence-complete");
	assert.equal(noStage.claims.nativeStage.state, "incomplete", "output and native Stage claims are independent");
});

test("reports retain binary, build, host and workload identity", async () => {
	const { artifactPaths } = await import("./artifact-paths.mjs");
	const directory = fs.mkdtempSync(path.join(artifactPaths.tmp, "semantic-report-test-"));
	try {
		const binary = path.join(directory, "light-headless");
		fs.writeFileSync(binary, "binary");
		const build = collectBuildIdentity({ binaryPath: binary, buildProfile: "release" });
		assert.equal(build.binary.sha256, "9a3a45d01531a20e89ac6ae10b0b0beb0492acd7216a368aa062d1a5fecaf9cd");
		assert.ok(typeof build.gitHead === "string" || build.gitHead.status === "unavailable");
		assert.equal(collectBuildIdentity({ binaryPath: path.join(directory, "missing") }).binary.status, "unavailable");
		const host = collectHostIdentity();
		assert.ok(host.logicalCpus > 0 && host.platform && host.arch);
		assert.equal(host.gpu.status, "unavailable");
		const report = createSemanticPerformanceReport({ manifest, build, host, createdAt });
		assert.deepEqual(report.identity.workload, {
			version: manifest.workloadVersion,
			id: manifest.workloadId,
			seed: "548",
			manifestSha256: manifest.manifestSha256,
			identitySource: "synthetic-deterministic",
		});
		const file = await writeSemanticPerformanceReport(report, directory);
		assert.equal(JSON.parse(fs.readFileSync(file, "utf8")).reportSha256, report.reportSha256);
		const again = createSemanticPerformanceReport({ manifest, build, host, createdAt: "2027-01-01T00:00:00.000Z" });
		assert.equal(again.reportSha256, report.reportSha256, "report digest excludes wall-clock time");
	} finally {
		fs.rmSync(directory, { recursive: true, force: true });
	}
});

// TL-604 fixtures. Only exports that predate TL-604 are imported above, so the identity
// regression below can be run against the previous collectBuildIdentity as a negative control.
const gitEnv = { ...process.env, GIT_CONFIG_GLOBAL: "/dev/null", GIT_CONFIG_NOSYSTEM: "1" };
function git(cwd, ...args) {
	const result = spawnSync("git", ["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", ...args], { cwd, env: gitEnv, encoding: "utf8" });
	assert.equal(result.status, 0, `git ${args.join(" ")}: ${result.stderr}`);
	return result.stdout.trim();
}
async function fixtureDirectory(name) {
	const { artifactPaths } = await import("./artifact-paths.mjs");
	return fs.mkdtempSync(path.join(artifactPaths.tmp, `semantic-report-${name}-`));
}
function writeFile(root, file, content) {
	fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
	fs.writeFileSync(path.join(root, file), content);
}

test("equal HEAD and dirty flag with different source bytes yield different identity", async () => {
	const root = await fixtureDirectory("bytes");
	try {
		git(root, "init", "-q", "-b", "main");
		writeFile(root, "tools/engine.mjs", "export const gain = 1;\n");
		git(root, "add", "-A");
		git(root, "commit", "-q", "-m", "fixture");
		const identityWith = (content) => {
			writeFile(root, "tools/engine.mjs", content);
			return collectBuildIdentity({ repositoryRoot: root, buildProfile: "release" });
		};
		const a = identityWith("export const gain = 2;\n");
		const b = identityWith("export const gain = 3;\n");
		assert.equal(a.gitHead, b.gitHead, "precondition: same HEAD");
		assert.equal(a.gitTrackedChanges, true, "precondition: dirty");
		assert.equal(b.gitTrackedChanges, true, "precondition: same dirty flag");
		const reportA = createSemanticPerformanceReport({ manifest, build: a, createdAt });
		const reportB = createSemanticPerformanceReport({ manifest, build: b, createdAt });
		assert.notDeepEqual(reportA.identity.build, reportB.identity.build, "build identity must distinguish the bytes");
		assert.notEqual(reportA.reportSha256, reportB.reportSha256, "report digest must distinguish the bytes");
		// Untracked additions do not flip the tracked dirty flag, but still change identity.
		const c = identityWith("export const gain = 3;\n");
		writeFile(root, "tools/new-stage.mjs", "export {};\n");
		const d = collectBuildIdentity({ repositoryRoot: root, buildProfile: "release" });
		assert.equal(c.gitTrackedChanges, d.gitTrackedChanges);
		assert.notEqual(createSemanticPerformanceReport({ manifest, build: c, createdAt }).reportSha256, createSemanticPerformanceReport({ manifest, build: d, createdAt }).reportSha256);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

test("source identity is recorded, nested snapshots stay unavailable, and supplied manifests are validated", async () => {
	const root = await fixtureDirectory("source");
	try {
		git(root, "init", "-q", "-b", "main");
		writeFile(root, "tools/engine.mjs", "export const gain = 1;\n");
		git(root, "add", "-A");
		git(root, "commit", "-q", "-m", "fixture");
		const head = git(root, "rev-parse", "HEAD");
		const build = collectBuildIdentity({ repositoryRoot: root });
		assert.equal(build.source.status, "recorded");
		assert.equal(build.source.origin, "collected");
		assert.equal(build.source.mode, "git");
		assert.equal(build.source.gitHead, head);
		assert.match(build.source.sourceSha256, /^[0-9a-f]{64}$/u);
		const report = createSemanticPerformanceReport({ manifest, build, createdAt });
		assert.equal(report.identity.build.sourceManifest, undefined, "the full manifest stays out of the report JSON");
		assert.equal(report.sourceManifest.sourceSha256, build.source.sourceSha256);

		const snapshot = path.join(root, "tools", "snapshot");
		writeFile(snapshot, "tools/engine.mjs", "export const gain = 1;\n");
		const nested = collectBuildIdentity({ repositoryRoot: snapshot });
		assert.equal(nested.gitHead.status, "unavailable");
		assert.equal(nested.gitTrackedChanges.status, "unavailable");
		assert.equal(nested.source.mode, "filesystem");
		assert.equal(nested.source.gitHead.status, "unavailable");
		assert.equal(nested.source.deletions.status, "unavailable");
		assert.ok(!JSON.stringify(nested).includes(head), "the enclosing HEAD is not borrowed");

		const supplied = collectBuildIdentity({ repositoryRoot: path.join(root, "missing"), sourceManifest: nested.sourceManifest });
		assert.equal(supplied.source.origin, "supplied-snapshot-manifest");
		assert.equal(supplied.source.sourceSha256, nested.source.sourceSha256);
		const tampered = JSON.parse(JSON.stringify(nested.sourceManifest));
		tampered.entries[0].bytes += 1;
		assert.throws(() => collectBuildIdentity({ repositoryRoot: root, sourceManifest: tampered }), /digest does not match/u);
		assert.throws(() => createSemanticPerformanceReport({ manifest, createdAt, build: { ...build, sourceManifest: nested.sourceManifest } }), /does not match/u);
		assert.equal(collectBuildIdentity({ repositoryRoot: path.join(root, "missing") }).source.status, "unavailable");
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});

test("reports retain the source manifest and never overwrite prior evidence", async () => {
	const root = await fixtureDirectory("retain");
	try {
		const sources = path.join(root, "sources");
		writeFile(sources, "tools/engine.mjs", "export const gain = 1;\n");
		const build = collectBuildIdentity({ repositoryRoot: sources });
		const out = path.join(root, "out");
		const first = createSemanticPerformanceReport({ manifest, build, createdAt });
		const firstFile = await writeSemanticPerformanceReport(first, out);
		const firstBytes = fs.readFileSync(firstFile, "utf8");
		const manifestFiles = fs.readdirSync(out).filter((name) => name.startsWith("source-manifest-"));
		assert.deepEqual(manifestFiles, [`source-manifest-${build.source.sourceSha256}.json`]);
		const retained = JSON.parse(fs.readFileSync(path.join(out, manifestFiles[0]), "utf8"));
		assert.equal(retained.sourceSha256, JSON.parse(firstBytes).identity.build.source.sourceSha256);
		assert.deepEqual(retained.entries.map((entry) => entry.path), ["tools/engine.mjs"]);

		// Rewriting the identical report is idempotent; a second run at the same instant with
		// different sources gets its own report and manifest; prior files are byte-identical.
		assert.equal(await writeSemanticPerformanceReport(first, out), firstFile);
		writeFile(sources, "tools/engine.mjs", "export const gain = 2;\n");
		const secondFile = await writeSemanticPerformanceReport(createSemanticPerformanceReport({ manifest, build: collectBuildIdentity({ repositoryRoot: sources }), createdAt }), out);
		assert.notEqual(secondFile, firstFile);
		assert.equal(fs.readFileSync(firstFile, "utf8"), firstBytes);
		assert.equal(fs.readdirSync(out).filter((name) => name.startsWith("source-manifest-")).length, 2);
		assert.equal(fs.readdirSync(out).filter((name) => name.startsWith("report-")).length, 2);

		// Corrupted prior evidence is refused, not repaired.
		fs.writeFileSync(firstFile, "{}\n");
		await assert.rejects(writeSemanticPerformanceReport(first, out), /refusing to overwrite prior report/u);
		assert.equal(fs.readFileSync(firstFile, "utf8"), "{}\n");

		// TL-580 ownership: a directory holding another workload manifest is refused.
		const owned = path.join(root, "owned");
		fs.mkdirSync(owned);
		fs.writeFileSync(path.join(owned, "manifest.json"), JSON.stringify({ manifestSha256: "0".repeat(64) }));
		await assert.rejects(writeSemanticPerformanceReport(first, owned), /refusing to write a report/u);
		assert.deepEqual(fs.readdirSync(owned), ["manifest.json"]);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});
