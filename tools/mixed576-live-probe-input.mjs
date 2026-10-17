// Private ignored-probe input: production JS scene/partition constructors, no server or UI.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths } from "./artifact-paths.mjs";
import { createPerformanceFixtureInputs } from "./stage-large-scene.mjs";
import { createLargeStageDynamicsPlan } from "./stage-dynamics-scene.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageNames = [
	"robe--robin-dls-profile", "robe--robin-600x-ledwash",
	"showtec--sunstrip-led-rgb-42206", "robe--robin-ledbeam-150", "generic--dimmer",
	"venue--stage-element-2-1-m", "venue--stage-stairs", "venue--four-point-truss", "venue--curtain-5-m",
];
const packages = packageNames.map((name) => path.join(root, "assets/fixture-library", `${name}.toskfixture`));
// Read archive metadata without extracting files or substituting synthetic profiles. Rust
// reopens the same hash-bound packages through read_fixture_package/runtime compatibility.
const profiles = JSON.parse(execFileSync("python3", ["-c", `
import json,sys,zipfile
print(json.dumps([json.loads(zipfile.ZipFile(p).read("fixture.json"))["profile"] for p in sys.argv[1:]]))
`, ...packages], { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 }));
const scene = createPerformanceFixtureInputs(profiles, 576);
const uuid = (text) => {
	const hex = createHash("sha256").update(text).digest("hex");
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-8${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
};
const selected = new Map(profiles.map((profile) => [profile.id, profile]));
const fixtures = scene.fixtures.map((fixture) => {
	const mode = selected.get(fixture.profile_id).modes.find((mode) => mode.id === fixture.mode_id);
	return { ...fixture, logical_heads: mode.heads.flatMap((head, index) => head.master_shared ? [] : [{
		profile_head_id: head.id, head_index: index,
		fixture_id: uuid(`tl775-mixed576:${fixture.fixture_id}:${head.id}`),
	}]) };
});
const usedProfiles = new Set(fixtures.map((fixture) => fixture.profile_id));
const patch = {
	fixtures,
	profile_revisions: profiles.filter((profile) => usedProfiles.has(profile.id)).map((profile) => ({
		profile_id: profile.id, profile_revision: profile.revision, profile_snapshot: profile,
	})),
};
const plan = createLargeStageDynamicsPlan(patch, scene);
const inventory = Object.fromEntries(scene.inventory.map((entry) => [entry.key, entry.records]));
assert.equal(scene.fixtures.length, 576);
assert.equal(scene.patch.occupiedSlots, 8064);
assert.equal(scene.patch.universeCount, 16);
assert.equal(plan.dynamicTargetCount, 5408);
assert.equal(plan.activations.length, 20);
assert.equal(plan.definitions.length, 20);
assert.equal(inventory.beam, 224);
assert.equal(inventory.sunstrip, 96);
assert.equal(inventory.dimmer, 256);
assert.equal(plan.laneCoverage.intensity, 1184);
assert.equal(plan.laneCoverage["position.pan"], 224);
assert.equal(plan.laneCoverage["position.tilt"], 224);
assert.equal(plan.laneCoverage["color.red"], 1184);
assert.equal(plan.laneCoverage["color.green"], 1184);
assert.equal(plan.laneCoverage["color.blue"], 1184);
assert.equal(plan.laneCoverage["color.white_blend"], 224);
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const packageBindings = await Promise.all(packages.map(async (file, index) => ({
	file: path.relative(root, file), sha256: digest(await readFile(file)), profile_id: profiles[index].id,
})));
const sources = await Promise.all(["tools/stage-large-scene.mjs", "tools/stage-dynamics-scene.mjs", "tools/mixed576-live-probe-input.mjs"].map(async (file) => ({ file, sha256: digest(await readFile(path.join(root, file))) })));
const input = {
	schema: 1, kind: "private.production-js-mixed576-live-input/v1", scene, patch, plan,
	package_bindings: packageBindings, source_bindings: sources,
	qualification: "Actual shipped profiles, scene packing and twenty-controller partition/lanes. Logical-head UUIDs are deterministic harness identities, not the unretrieved historical Linux show's random IDs. Both worker cases must consume these identical bytes. No benchmark, SQL, server, renderer, output gate or public telemetry is run here.",
};
const name = process.argv[2] ?? "tl775-mixed576-live-input-01.json";
assert.match(name, /^tl775-mixed576-live-input-[0-9]+\.json$/);
const output = path.join(artifactPaths.tmp, name);
const bytes = `${JSON.stringify(input)}\n`;
await mkdir(artifactPaths.tmp, { recursive: true });
await writeFile(output, bytes, { flag: "wx", mode: 0o600 });
process.stdout.write(`${JSON.stringify({ output, sha256: digest(bytes), fixtures: 576, slots: 8064, universes: 16, lanes: 5408, controllers: 20 })}\n`);
