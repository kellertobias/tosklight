import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import {
	SEMANTIC_CONTRACT_SCHEMAS,
	angleCurrentPartnerLaneId,
	canonicalJson,
	createSeededRandom,
	semanticContractRepositoryRoot,
	semanticDefinitionErrors,
	uuidV5,
	validateSemanticDefinition,
	validateWireDefinition,
} from "./semantic-performance-contract.mjs";
import {
	DEFAULT_SEMANTIC_RIG,
	createSyntheticSemanticPatch,
	loadFixtureLibrary,
} from "./semantic-performance-fixtures.mjs";
import {
	SEMANTIC_DYNAMIC_POOL_BASE,
	buildSemanticPerformanceWorkload,
	buildSyntheticSemanticWorkload,
	semanticWorkloadDirectory,
	trackingFrames,
	validateSemanticWorkload,
	writeSemanticWorkload,
} from "./semantic-performance-workload.mjs";
import { createSemanticPerformanceReport, writeSemanticPerformanceReport } from "./semantic-performance-report.mjs";
import { LARGE_STAGE_DYNAMIC_INSTANCES } from "./stage-large-scene.mjs";

const library = loadFixtureLibrary({
	packages: [...new Set([...DEFAULT_SEMANTIC_RIG.map((entry) => entry.packageName), "tosklight--3d-point"])],
});
const quick = { trackingDurationSeconds: 1 };
const build = (seed, request = quick, rig = DEFAULT_SEMANTIC_RIG) =>
	buildSemanticPerformanceWorkload(createSyntheticSemanticPatch({ seed: String(seed), library, rig }), { seed, request });
const baseline = build(548);

test("UUIDv5 matches RFC 4122 and the Rust Angle partner derivation", () => {
	assert.equal(uuidV5("6ba7b810-9dad-11d1-80b4-00c04fd430c8", "www.example.com"), "2ed6657d-e927-568b-95e1-2665a8aea6a2");
	const definition = "30000000-0000-4000-8000-000000000001";
	assert.equal(angleCurrentPartnerLaneId(definition, "pan"), uuidV5(definition, "tosklight:position-current:pan:v1"));
	assert.notEqual(angleCurrentPartnerLaneId(definition, "pan"), angleCurrentPartnerLaneId(definition, "tilt"));
	assert.throws(() => angleCurrentPartnerLaneId(definition, "zoom"));
});

test("the same seed reproduces byte-identical workloads and inputs; another seed differs", () => {
	const again = build(548);
	for (const key of ["manifest", "definitions", "activations", "staticValues", "positionScenarios", "dirty", "tracking"])
		assert.equal(canonicalJson(again[key]), canonicalJson(baseline[key]), key);
	const first = [...trackingFrames(baseline, { scenario: "all-points-move", rateHz: 120 })];
	const second = [...trackingFrames(again, { scenario: "all-points-move", rateHz: 120 })];
	assert.equal(canonicalJson(first), canonicalJson(second));
	const other = build(549);
	assert.notEqual(other.manifest.manifestSha256, baseline.manifest.manifestSha256);
	assert.notEqual(other.manifest.workloadId, baseline.manifest.workloadId);
	assert.notEqual(other.definitions[0].id, baseline.definitions[0].id);
	assert.notEqual(canonicalJson(other.activations.map((a) => a.targets)), canonicalJson(baseline.activations.map((a) => a.targets)));
	const random = createSeededRandom(7);
	const replay = createSeededRandom(7);
	assert.deepEqual(Array.from({ length: 5 }, random.next), Array.from({ length: 5 }, replay.next));
});

test("every definition and static base is valid against the generated wire contract and Rust rules", async (t) => {
	assert.deepEqual(validateSemanticWorkload(baseline), []);
	const broken = structuredClone(baseline.definitions[0]);
	delete broken.spatial_mapping;
	assert.ok(validateSemanticDefinition(broken).some((error) => /spatial_mapping/u.test(error)));
	await t.test("cross-check with ajv when the hoisted dependency is present", async (sub) => {
		let Ajv;
		try {
			Ajv = (await import("ajv/dist/2020.js")).default;
		} catch {
			sub.skip("ajv is not installed; the strict in-repo validator above still ran");
			return;
		}
		const ajv = new Ajv({ strict: false });
		for (const format of ["uint8", "uint16", "uint32", "uint64", "float", "double"]) ajv.addFormat(format, { type: "number", validate: () => true });
		ajv.addFormat("uuid", /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/iu);
		const compile = (contract, id) => {
			const schema = JSON.parse(fs.readFileSync(path.join(semanticContractRepositoryRoot, contract.file), "utf8"));
			return ajv.compile({ $id: id, $defs: schema.$defs, $ref: `#/$defs/${contract.definition}` });
		};
		const definition = compile(SEMANTIC_CONTRACT_SCHEMAS.dynamicDefinition, "semantic-definition");
		const attribute = compile(SEMANTIC_CONTRACT_SCHEMAS.programmingAttributeValue, "semantic-attribute");
		for (const item of baseline.definitions) assert.ok(definition(item), `${item.name}: ${JSON.stringify(definition.errors)}`);
		for (const mutation of baseline.staticValues) assert.ok(attribute(mutation.value), JSON.stringify(attribute.errors));
		assert.equal(definition(broken), false);
	});
});

test("Angle Dynamics always carry Pan and Tilt with persisted Current partners", () => {
	const angles = baseline.definitions.filter((item) => item.lanes.some((lane) => lane.programming.address.representation.kind === "angles"));
	assert.equal(angles.length, 3);
	for (const item of angles) {
		const axes = item.lanes.map((lane) => lane.programming.address.component.kind).sort();
		assert.deepEqual(axes, ["pan", "tilt"]);
	}
	assert.deepEqual(baseline.manifest.angleDynamics.map((entry) => entry.currentPartners), [["tilt"], ["pan"], []]);
	const partner = angles[0].lanes.find((lane) => lane.id === angleCurrentPartnerLaneId(angles[0].id, "tilt"));
	assert.deepEqual(partner.programming.configuration.configuration.points.map((point) => point.source.kind), ["current", "current"]);
	const lonely = { ...angles[0], lanes: angles[0].lanes.filter((lane) => lane !== partner) };
	assert.ok(semanticDefinitionErrors(lonely).some((error) => /complete Pan\/Tilt pair/u.test(error)));
});

test("targets are the patch's actual root and logical-head identities", () => {
	const patch = createSyntheticSemanticPatch({ seed: "548", library });
	const roots = new Set(patch.fixtures.map((fixture) => fixture.fixture_id));
	const logical = new Set(patch.fixtures.flatMap((fixture) => fixture.logical_heads.map((head) => head.fixture_id)));
	const targets = baseline.activations.flatMap((activation) => activation.targets);
	assert.ok(targets.every((target) => roots.has(target) || logical.has(target)));
	assert.ok(targets.some((target) => logical.has(target)), "logical heads are programmed");
	assert.ok(targets.some((target) => roots.has(target)), "master-shared heads program their root");
	const angleTargets = baseline.activations.filter((activation) => activation.family === "angle").flatMap((activation) => activation.targets);
	assert.ok(angleTargets.every((target) => roots.has(target)));
	assert.equal(baseline.manifest.counts.logicalHeads.actual, logical.size - baseline.manifest.counts.points.actual);
	assert.equal(new Set(targets).size, baseline.manifest.counts.targets.actual);
	assert.ok(baseline.definitions.every((item) => item.pool_number > 9_000 + LARGE_STAGE_DYNAMIC_INSTANCES && item.pool_number > SEMANTIC_DYNAMIC_POOL_BASE));
});

test("the manifest reports requested versus realized counts and never fakes a shortfall", () => {
	const { manifest } = baseline;
	assert.equal(manifest.identitySource, "synthetic-deterministic");
	assert.equal(manifest.counts.dynamics.actual, manifest.counts.dynamics.requested);
	assert.ok(Object.values(manifest.capabilityMix).every((entry) => entry.status === "covered"));
	assert.deepEqual(manifest.shortfalls, []);
	const greedy = build(548, { ...quick, color: { cmy: 100 }, zoom: 50, position: { separateMounts: 9 } });
	assert.deepEqual(greedy.manifest.capabilityMix["color.cmy"], { requested: 100, realized: 12, status: "partial" });
	assert.equal(greedy.manifest.capabilityMix["zoom.physicalDegrees"].realized, greedy.manifest.capabilityAvailability["zoom.physicalDegrees"]);
	assert.equal(greedy.manifest.capabilityMix["position.separateMovingMount"].realized, 6);
	assert.ok(greedy.manifest.counts.dynamics.actual < greedy.manifest.counts.dynamics.requested);
	assert.deepEqual(
		greedy.manifest.shortfalls.map((entry) => entry.key).sort(),
		["color.cmy", "position.separateMovingMount", "zoom.physicalDegrees"],
	);
	const noUv = build(548, quick, DEFAULT_SEMANTIC_RIG.filter((entry) => !["rgbwauv", "rootpar"].includes(entry.key)));
	assert.deepEqual(noUv.manifest.capabilityMix.uv, { requested: 16, realized: 0, status: "missing" });
	assert.ok(!noUv.activations.some((activation) => activation.family === "uv"));
	const missingPackage = build(548, quick, [...DEFAULT_SEMANTIC_RIG, { key: "ghost", packageName: "no--such-fixture", mode: "x", count: 3 }]);
	assert.ok(missingPackage.manifest.shortfalls.some((entry) => entry.kind === "rig-entry" && entry.key === "ghost"));
});

test("a live patch without a rig reports its missing request side as unavailable", () => {
	const { rig, identity_source, ...livePatch } = createSyntheticSemanticPatch({ seed: "548", library });
	assert.ok(rig && identity_source);
	const live = buildSemanticPerformanceWorkload({ ...livePatch, show_id: "show", patch_revision: 4 }, { seed: 548, request: quick });
	assert.equal(live.manifest.identitySource, "live-patch");
	assert.equal(live.manifest.counts.rootFixtures.requested.status, "unavailable");
	assert.equal(live.manifest.fixtureLibrary.status, "unavailable");
	assert.equal(live.manifest.productionSupport.semanticProgrammingContract.status, "unavailable");
	assert.equal(canonicalJson(live.definitions), canonicalJson(baseline.definitions));
	const [stripped, ...others] = livePatch.profile_revisions;
	const older = buildSemanticPerformanceWorkload(
		{ ...livePatch, profile_revisions: [{ ...stripped, profile_snapshot: null }, ...others] },
		{ seed: 548, request: quick },
	);
	const hidden = new Set(livePatch.fixtures.filter((fixture) => fixture.profile_id === stripped.profile_id).map((fixture) => fixture.fixture_id));
	assert.equal(older.manifest.counts.unresolvedFixtures.length, hidden.size);
	assert.ok(older.manifest.shortfalls.some((entry) => entry.kind === "unresolved-fixtures" && entry.count === hidden.size));
	assert.ok(!older.activations.some((activation) => activation.targets.some((target) => hidden.has(target))));
});

test("workloads cover moving mounts, aim references, dirty subsets, mixed Color with UV, Focus and Zoom", () => {
	const scenarios = baseline.positionScenarios;
	const patch = createSyntheticSemanticPatch({ seed: "548", library });
	const points = new Set(patch.fixtures.filter((fixture) => fixture.profile_id === library.find((entry) => entry.packageName === "tosklight--3d-point").profile.id).map((fixture) => fixture.fixture_id));
	const fixed = scenarios.find((scenario) => scenario.kind === "fixed");
	assert.deepEqual(fixed.reference, { kind: "origin" });
	for (const kind of ["referenced", "shared-moving-mount", "separate-moving-mount"]) {
		const matching = scenarios.filter((scenario) => scenario.kind === kind);
		assert.ok(matching.length > 0 && matching.every((scenario) => points.has(scenario.aimPoint) && scenario.reference.point_id === scenario.aimPoint));
		if (kind !== "referenced")
			assert.ok(matching.every((scenario) => points.has(scenario.mountPoint) && scenario.mountPoint !== scenario.aimPoint));
	}
	assert.ok(scenarios.filter((scenario) => scenario.kind === "shared-moving-mount").every((scenario) => scenario.targets.length > 1));
	assert.ok(scenarios.filter((scenario) => scenario.kind === "separate-moving-mount").every((scenario) => scenario.targets.length === 1));
	const dirty = Object.fromEntries(baseline.manifest.dirtyScenarios.map((scenario) => [scenario.key, scenario]));
	assert.equal(dirty["static-points"].dirtyTargetCount, 0);
	assert.ok(dirty["small-subset"].dirtyTargetCount > 0);
	assert.ok(dirty["small-subset"].dirtyTargetCount < dirty["all-points-move"].dirtyTargetCount);
	assert.equal(dirty["all-points-move"].dirtyTargetCount, dirty["all-points-move"].positionTargetCount - fixed.targets.length);
	const coverage = baseline.manifest.laneCoverage;
	for (const key of ["angles/pan/current", "angles/tilt/current", "target/target_x/current", "semantic_color.recipe/red/value", "semantic_color.recipe/white_blend/value", "semantic_color.hue_saturation/hue/value", "semantic_color.retain/uv/value", "focus/focus/value", "zoom/zoom/value"])
		assert.ok(coverage[key]?.addresses > 0, key);
	assert.equal(baseline.manifest.mixedColorWithUv.uvTargetsWithColorIntentDynamic, baseline.manifest.mixedColorWithUv.uvTargets);
	assert.ok(baseline.manifest.limitations.some((entry) => entry.kind === "zoom-physical-model"));
});

test("tracking inputs follow the configured rates and move only the scenario's Points", () => {
	const workload = build(548, { trackingDurationSeconds: 0.5, trackingRatesHz: [120, 30, 60], outputRatesHz: [50] });
	assert.deepEqual(workload.manifest.rates.trackingHz, [30, 60, 120]);
	assert.deepEqual(workload.manifest.rates.matrix, [30, 60, 120].map((trackingHz) => ({ trackingHz, outputHz: 50 })));
	assert.equal(workload.manifest.rates.kind, "configured-targets");
	for (const rateHz of [30, 60, 120]) {
		const frames = [...trackingFrames(workload, { scenario: "small-subset", rateHz })];
		assert.equal(frames.length, rateHz / 2);
		assert.equal(frames[1].t_micros, Math.round(1_000_000 / rateHz));
		const moving = new Set(workload.dirty.scenarios.find((scenario) => scenario.key === "small-subset").movingPointIds);
		const at = (frame, id) => canonicalJson(frame.points.find((point) => point.point_id === id).position_metres);
		for (const id of workload.tracking.motion.map((motion) => motion.pointId))
			assert.equal(at(frames[0], id) === at(frames.at(-1), id), !moving.has(id), id);
	}
	assert.throws(() => trackingFrames(workload, { rateHz: 44 }).next(), /not configured/u);
	assert.throws(() => build(548, { trackingRatesHz: [] }), /at least one rate/u);
});

test("written workloads land in the canonical artifact tree", async () => {
	const { artifactPaths } = await import("./artifact-paths.mjs");
	const directory = fs.mkdtempSync(path.join(artifactPaths.tmp, "semantic-workload-test-"));
	try {
		writeSemanticWorkload(build(548, { trackingDurationSeconds: 0.1 }), directory, { emitTracking: true });
		assert.ok(fs.existsSync(path.join(directory, "manifest.json")));
		assert.ok(fs.existsSync(path.join(directory, "tracking-small-subset-120hz.ndjson")));
		// Canonical means the resolved artifact paths, which honour explicit LIGHT_* overrides
		// (AGENTS.md); an overridden LIGHT_TMP_DIR may legitimately lie outside the checkout.
		assert.ok(!path.relative(artifactPaths.tmp, directory).startsWith(".."));
	} finally {
		fs.rmSync(directory, { recursive: true, force: true });
	}
	assert.deepEqual(validateWireDefinition(SEMANTIC_CONTRACT_SCHEMAS.programmingAttributeValue, { kind: "normalized", value: 0.5 }), []);
});

test("the offline convenience builder uses shipped packages end to end", () => {
	const workload = buildSyntheticSemanticWorkload({ seed: "tl-553", request: quick });
	assert.deepEqual(validateSemanticWorkload(workload), []);
	assert.ok(workload.manifest.fixtureLibrary.packages.every((entry) => /^assets\/fixture-library\//u.test(entry.file) && entry.sha256.length === 64));
});

// TL-580: two different patches with the same seed and request share a workloadId. Their
// evidence must not alias: each report resolves to its own exact manifest, workload and
// tracking inputs, and a conflicting write into an occupied directory is refused.
const aliasId = (n) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
const aliasProfile = {
	id: aliasId(10),
	revision: 1,
	name: "Alias Focus",
	modes: [{ id: aliasId(11), heads: [{ id: aliasId(12), name: "Root", master_shared: true }], channels: [{ head_id: aliasId(12), attribute: "focus", functions: [] }] }],
};
const aliasPatch = (fixture) => ({
	show_id: aliasId(1),
	patch_revision: 1,
	profile_revisions: [{ profile_id: aliasProfile.id, profile_revision: 1, profile_snapshot: aliasProfile }],
	fixtures: [{ fixture_id: fixture, profile_id: aliasProfile.id, profile_revision: 1, mode_id: aliasId(11), logical_heads: [], position_master: null }],
});
const aliasRequest = {
	angle: 0,
	position: { fixed: 0, referenced: 0, sharedMountGroups: 0, sharedMountGroupSize: 0, separateMounts: 0 },
	color: { rgb: 0, rgbw: 0, cmy: 0, wheel: 0 },
	uv: 0,
	focus: 1,
	zoom: 0,
	trackingDurationSeconds: 0.1,
};
const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
const snapshotDirectory = (directory) =>
	Object.fromEntries(fs.readdirSync(directory).sort().map((name) => [name, fs.readFileSync(path.join(directory, name), "utf8")]));

test("workloads sharing a seed and request keep separate, exactly attributable evidence", async () => {
	const { artifactPaths } = await import("./artifact-paths.mjs");
	const root = fs.mkdtempSync(path.join(artifactPaths.tmp, "semantic-workload-alias-"));
	try {
		const a = buildSemanticPerformanceWorkload(aliasPatch(aliasId(101)), { seed: 553, request: aliasRequest });
		const b = buildSemanticPerformanceWorkload(aliasPatch(aliasId(102)), { seed: 553, request: aliasRequest });
		assert.deepEqual(validateSemanticWorkload(a), []);
		assert.deepEqual(validateSemanticWorkload(b), []);
		assert.equal(a.manifest.workloadId, b.manifest.workloadId, "precondition: the IDs alias");
		assert.notEqual(a.manifest.manifestSha256, b.manifest.manifestSha256);

		const reports = [];
		for (const [workload, createdAt] of [[a, "2026-09-30T08:00:00.000Z"], [b, "2026-09-30T08:05:00.000Z"]]) {
			const directory = semanticWorkloadDirectory(root, workload.manifest);
			writeSemanticWorkload(workload, directory, { emitTracking: true });
			const report = createSemanticPerformanceReport({ manifest: workload.manifest, createdAt });
			reports.push({ directory, file: await writeSemanticPerformanceReport(report, directory), workload });
		}
		assert.notEqual(reports[0].directory, reports[1].directory);
		for (const { directory, file, workload } of reports) {
			const report = readJson(file);
			const manifest = readJson(path.join(directory, "manifest.json"));
			assert.equal(report.identity.workload.manifestSha256, manifest.manifestSha256);
			assert.equal(manifest.manifestSha256, workload.manifest.manifestSha256);
			assert.deepEqual(readJson(path.join(directory, "workload.json")).activations, workload.activations);
			// Every tracking stream on disk is the one this manifest's digests name.
			const trackingFiles = fs.readdirSync(directory).filter((name) => name.startsWith("tracking-"));
			assert.ok(trackingFiles.length > 0);
			for (const name of trackingFiles) {
				const [, scenario, rate] = /^tracking-(.+)-(\d+)hz\.ndjson$/u.exec(name);
				const expected = [...trackingFrames(workload, { scenario, rateHz: Number(rate) })].map((frame) => canonicalJson(frame)).join("\n");
				assert.equal(fs.readFileSync(path.join(directory, name), "utf8"), `${expected}\n`, name);
			}
		}

		// Identical regeneration is deterministic and allowed.
		const before = snapshotDirectory(reports[0].directory);
		writeSemanticWorkload(a, reports[0].directory, { emitTracking: true });
		assert.deepEqual(snapshotDirectory(reports[0].directory), before);

		// An explicit output override still works, but different inputs into occupied evidence
		// are refused without touching the manifest, workload, tracking or report files.
		assert.throws(() => writeSemanticWorkload(b, reports[0].directory, { emitTracking: true }), /refusing to overwrite/u);
		assert.deepEqual(snapshotDirectory(reports[0].directory), before);
		const orphan = path.join(root, "orphan");
		fs.mkdirSync(orphan);
		fs.writeFileSync(path.join(orphan, "tracking-small-subset-30hz.ndjson"), "stale\n");
		assert.throws(() => writeSemanticWorkload(a, orphan), /without a manifest/u);
		assert.equal(fs.readFileSync(path.join(orphan, "tracking-small-subset-30hz.ndjson"), "utf8"), "stale\n");
		const explicit = path.join(root, "explicit-out");
		writeSemanticWorkload(b, explicit);
		assert.equal(readJson(path.join(explicit, "manifest.json")).manifestSha256, b.manifest.manifestSha256);
	} finally {
		fs.rmSync(root, { recursive: true, force: true });
	}
});
