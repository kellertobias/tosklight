// TL-842: the exact mixed576 Live replay inside a throttled Linux container.
//
// Builds the ignored `manual_exact_mixed576_serial_and_two_workers` test for Linux in a pinned
// Rust image, then runs it under a CPU profile and prints per-frame transaction, CPU and wall
// time, the family stage split and a digest of every per-frame output fingerprint.
//
//   node tools/mixed576-container-replay.mjs [--profile free|ci|saturated] [--frames N]
//        [--label NAME] [--compare REPORT.json] [--skip-build]
//
// Profiles (calibrated on an M-series host, TL-842):
//   free       no limit. CPU-ms per frame is the hardware-proportional figure to compare.
//   ci         --cpus 0.6 in 10 ms periods; the 2-worker transaction matches the ~20 ms ordered
//              tick of the 4 vCPU x86 CI desk proof. Quota throttles total CPU, not core speed.
//   saturated  rootful podman, two pinned CPUs plus two busy threads standing in for UI/Stage.
// Wall times swing with host load (other builds); compare CPU-ms per frame and alternate A/B runs.
// Requires podman with a running machine. Outputs stay under .artifacts.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const IMAGE = "localhost/tosklight-mixed576-replay:rust1.95";
const CONTAINERFILE = "FROM docker.io/library/rust:1.95-bookworm\nRUN apt-get update && apt-get install -y --no-install-recommends libudev-dev && rm -rf /var/lib/apt/lists/*\n";
const INPUT_SHA256 = "0034aca0703cf144f23ee094f0f5d645eacee08711df7b835ddbb47bae785f3f";
const INPUT = path.join(artifactPaths.tmp, "tl775-mixed576-live-input-01.json");
const TARGET = path.join(artifactPaths.root, "build/cargo-linux-replay");
const CARGO_HOME = path.join(artifactPaths.root, "cache/cargo-linux-replay-home");
const EMPTY_FRONTEND = path.join(artifactPaths.tmp, "mixed576-container-empty-frontend");
const REPORTS = path.join(artifactPaths.performance, "mixed576-container");
const TEST = "runtime::output_scheduler::slow_phase_trace::live_phases::mixed576_tests::manual_exact_mixed576_serial_and_two_workers";
const PROFILES = {
	free: { connection: "podman-machine-default", args: [], hogs: 0 },
	ci: { connection: "podman-machine-default", args: ["--cpu-period=10000", "--cpu-quota=6000"], hogs: 0 },
	saturated: { connection: "podman-machine-default-root", args: ["--cpuset-cpus=0-1"], hogs: 2 },
};

export function options(argv) {
	const result = { profile: "free", frames: 128, label: null, compare: null, skipBuild: false };
	for (let index = 0; index < argv.length; index += 1) {
		const value = () => argv[++index] ?? assert.fail(`${argv[index - 1]} needs a value`);
		switch (argv[index]) {
			case "--profile": result.profile = value(); break;
			case "--frames": result.frames = Number(value()); break;
			case "--label": result.label = value(); break;
			case "--compare": result.compare = value(); break;
			case "--skip-build": result.skipBuild = true; break;
			default: assert.fail(`unknown option ${argv[index]}`);
		}
	}
	assert(Object.hasOwn(PROFILES, result.profile), `profile must be one of ${Object.keys(PROFILES).join(", ")}`);
	assert(Number.isInteger(result.frames) && result.frames >= 1 && result.frames <= 256, "--frames must be 1..256");
	result.label ??= `${result.profile}-${result.frames}`;
	return result;
}

function podman(connection, args, { input, capture = false } = {}) {
	const run = spawnSync("podman", ["--connection", connection, ...args], {
		input, encoding: "utf8", stdio: [input === undefined ? "inherit" : "pipe", capture ? "pipe" : "inherit", "inherit"], maxBuffer: 256 * 1024 * 1024,
	});
	assert.equal(run.status, 0, `podman ${args[0]} failed`);
	return run.stdout;
}

function ensureImage(connection) {
	const present = spawnSync("podman", ["--connection", connection, "image", "exists", IMAGE]).status === 0;
	if (!present) podman(connection, ["build", "--tag", IMAGE, "-"], { input: CONTAINERFILE });
}

function ensureInput() {
	if (!existsSync(INPUT)) execFileSync("node", ["tools/mixed576-live-probe-input.mjs", path.basename(INPUT)], { cwd: repositoryRoot, stdio: "inherit" });
	const digest = createHash("sha256").update(readFileSync(INPUT)).digest("hex");
	assert.equal(digest, INPUT_SHA256, `${INPUT} differs from the frozen mixed576 input`);
}

// The test reads fixture packages through CARGO_MANIFEST_DIR, so source sits at /src for both
// the build and the run. Native audio needs ALSA headers the image does not carry.
function build() {
	for (const directory of [TARGET, CARGO_HOME, EMPTY_FRONTEND]) mkdirSync(directory, { recursive: true });
	ensureImage(PROFILES.free.connection);
	podman(PROFILES.free.connection, [
		"run", "--rm", "--volume", `${repositoryRoot}:/src:ro`, "--volume", `${TARGET}:/target`, "--volume", `${CARGO_HOME}:/cargo-home`,
		"--volume", `${EMPTY_FRONTEND}:/frontend:ro`, "--env", "CARGO_HOME=/cargo-home", "--env", "CARGO_TARGET_DIR=/target",
		"--env", "LIGHT_CONTROL_FRONTEND_DIR=/frontend", "--workdir", "/src", IMAGE,
		"cargo", "test", "--release", "--locked", "-p", "light-headless-runtime", "--no-default-features", "--lib", "--no-run",
	]);
}

function testBinary() {
	const deps = path.join(TARGET, "release/deps");
	const candidates = readdirSync(deps).filter((name) => /^light_headless_runtime-[0-9a-f]+$/u.test(name))
		.map((name) => path.join(deps, name)).sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
	assert(candidates.length > 0, "build the Linux replay first (omit --skip-build)");
	return candidates[0];
}

function run({ profile, frames }) {
	const selected = PROFILES[profile];
	if (selected.connection !== PROFILES.free.connection) {
		const present = spawnSync("podman", ["--connection", selected.connection, "image", "exists", IMAGE]).status === 0;
		if (!present) podman(selected.connection, ["load"], { input: podman(PROFILES.free.connection, ["save", IMAGE], { capture: true }) });
	}
	const binary = testBinary();
	const hogs = Array.from({ length: selected.hogs }, () => "(while :; do :; done) &").join(" ");
	const output = podman(selected.connection, [
		"run", "--rm", "--network=none", "--memory=8g", ...selected.args,
		"--volume", `${repositoryRoot}:/src:ro`, "--volume", `${path.dirname(binary)}:/replay:ro`, "--workdir", "/src",
		"--env", "LIGHT_TMP_DIR=/tmp", "--env", `LIGHT_MIXED576_INPUT=/src/${path.relative(repositoryRoot, INPUT)}`,
		"--env", `LIGHT_MIXED576_INPUT_SHA256=${INPUT_SHA256}`, "--env", "LIGHT_MIXED576_WARMUP_FRAMES=8",
		"--env", `LIGHT_MIXED576_MEASURED_FRAMES=${frames}`, "--env", "LIGHT_MIXED576_USER_HZ=100", IMAGE,
		"sh", "-c", `${hogs} /replay/${path.basename(binary)} --ignored --exact ${TEST} --test-threads 1 --nocapture; status=$?; kill $(jobs -p) 2>/dev/null; exit $status`,
	], { capture: true });
	const line = output.split("\n").map((text) => text.slice(text.indexOf("{\"animated_addresses\""))).find((text) => text.startsWith("{"));
	assert(line, "the replay printed no report");
	return JSON.parse(line);
}

export function summarize(report) {
	const fingerprints = createHash("sha256").update(JSON.stringify(report.per_frame_fingerprints)).digest("hex");
	const cases = report.cases.map((entry) => {
		const frames = entry.warmup + entry.measured;
		const phases = Object.fromEntries(entry.family_phase_order.map((name, index) => [name, entry.family_phase_total_us[index] / entry.measured / 1000]));
		return {
			workers: entry.workers,
			transaction_ms: entry.transaction_total_ns / entry.measured / 1e6,
			cpu_ms_per_frame: entry.process_cpu?.available ? entry.process_cpu.total_seconds * 1000 / frames : null,
			wall_ms_per_frame: entry.whole_loop_including_warmup_and_fingerprints_ns / frames / 1e6,
			family_ms: phases,
		};
	});
	return { fingerprints_sha256: fingerprints, cases };
}

function main() {
	const parsed = options(process.argv.slice(2));
	ensureInput();
	if (!parsed.skipBuild) build();
	const report = run(parsed);
	const summary = { profile: parsed.profile, frames: parsed.frames, ...summarize(report) };
	mkdirSync(REPORTS, { recursive: true });
	const destination = path.join(REPORTS, `${parsed.label}.json`);
	writeFileSync(destination, `${JSON.stringify({ summary, report }, null, 1)}\n`);
	for (const entry of summary.cases) {
		const stages = Object.entries(entry.family_ms).filter(([, ms]) => ms >= 0.05).sort((a, b) => b[1] - a[1]).map(([name, ms]) => `${name}=${ms.toFixed(2)}`).join(" ");
		console.log(`workers=${entry.workers} transaction=${entry.transaction_ms.toFixed(2)}ms cpu/frame=${entry.cpu_ms_per_frame?.toFixed(2) ?? "n/a"}ms wall/frame=${entry.wall_ms_per_frame.toFixed(2)}ms\n  ${stages}`);
	}
	console.log(`fingerprints ${summary.fingerprints_sha256}\nreport ${path.relative(repositoryRoot, destination)}`);
	if (parsed.compare) {
		const previous = JSON.parse(readFileSync(parsed.compare, "utf8")).summary;
		assert.equal(summary.fingerprints_sha256, previous.fingerprints_sha256, "per-frame output fingerprints differ from the compared report");
		assert.equal(summary.frames, previous.frames, "compare reports with the same frame count");
		console.log("fingerprints identical to the compared report");
	}
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) main();
