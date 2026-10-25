import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { controlStageFrameFailures } from "./control-stage-frame-admission.mjs";

const runner = await readFile(
	new URL("./run-packaged-stage-benchmark.mjs", import.meta.url),
	"utf8",
);
const scenarios = await readFile(
	new URL("../docs/testing/16-stage-performance.md", import.meta.url),
	"utf8",
);
const nativeShell = await readFile(
	new URL(
		"../apps/light-desktop/src/NativePackagedStageBenchmarkApp.tsx",
		import.meta.url,
	),
	"utf8",
);
const helperProtocol = await readFile(
	new URL("../crates/viz/helper/src/protocol.rs", import.meta.url),
	"utf8",
);

test("native Stage acceptance retains its named profiles and canonical artifact", () => {
	assert.match(runner, /profile === "stage-500"/u);
	assert.match(runner, /stage-visualization-timing\.json/u);
	assert.match(runner, /applicationIdentity: packagedApplicationIdentity/u);
	assert.match(scenarios, /STAGE-PERF-001/u);
	assert.match(scenarios, /STAGE-PERF-002/u);
	assert.match(
		scenarios,
		/Browser Playwright, software rendering[\s\S]*never satisfy/u,
	);
});

test("an isolated packaged run never stops or addresses the desk on the default port", () => {
	assert.match(
		runner,
		/if \(isolatedPort === null\) await stopExistingDevelopmentDesk\(\);/u,
	);
	assert.match(runner, /if \(isolatedServer\) await stopIsolatedDesktop/u);
	assert.match(
		runner,
		/LIGHT_DESKTOP_TEST_BIND: `127\.0\.0\.1:\$\{isolatedPort\}`/u,
	);
	assert.match(runner, /direct: isolated,/u);
	assert.doesNotMatch(runner, /fetch\(\s*"http:\/\/127\.0\.0\.1:5000/u);
	assert.doesNotMatch(runner, /`http:\/\/127\.0\.0\.1:5000\$\{route\}`/u);
});

test("native gates measure first source presentations and windows across show switches", () => {
	assert.match(
		runner,
		/const firstPresentations = firstSourcePresentations\(/u,
	);
	assert.match(runner, /visibleChanged:\s*presentedSources\.has\(frame\)/u);
	assert.match(runner, /createOutputWindowAccumulator\(before\.output(?:\)|,)/u);
	assert.match(runner, /runtime\.outputWindows\?\.stage \?\?/u);
});

test("packaged acceptance measures the native helper rather than a removed WebGL canvas", () => {
	assert.match(nativeShell, /takeStagePaneBenchmarkSamples/u);
	assert.match(nativeShell, /stageView="3d"/u);
	assert.match(nativeShell, /followPreload/u);
	assert.match(nativeShell, /unmount-and-recreate/u);
	assert.match(helperProtocol, /FramePresented/u);
	assert.match(helperProtocol, /renderer: String/u);
	assert.match(runner, /packaged-tauri-native-stage/u);
	assert.match(runner, /reported no GPU\/backend identity/u);
	assert.match(runner, /native shared-surface resize\/recovery/u);
	assert.match(nativeShell, /appendNativeFrames/u);
	assert.match(nativeShell, /sourceFrame === previous\.sourceFrame/u);
	assert.match(nativeShell, /latestFrameByPane/u);
	assert.doesNotMatch(nativeShell, /for \(const frame of target\)/u);
	assert.match(runner, /function maximum\(values\)/u);
	assert.doesNotMatch(runner, /Math\.max\(\.\.\.latencies\)/u);
});

test("Control packaged acceptance verifies actual Draft wireframe frames", () => {
	assert.match(nativeShell, /vizQuality: config\.baselineQuality \?\? "draft"/u);
	assert.doesNotMatch(
		nativeShell,
		/const qualities = \["draft", "standard", "high", "ultra"\]/u,
	);
	assert.match(runner, /controlStageFrameFailures\(frames\)/u);
	const wireframe = { lane: "normal", mode: "lines_3d", quality: "draft" };
	const plan = { lane: "preload", mode: "top_down", quality: "draft" };
	assert.deepEqual(controlStageFrameFailures([wireframe, plan]), []);
	for (const mode of ["full_3d", "simple_3d", undefined, "unknown"]) {
		assert.ok(
			controlStageFrameFailures([wireframe, { ...plan, mode }]).some(
				(message) => message.includes("unverified view mode"),
			),
		);
	}
	for (const quality of ["standard", "high", "ultra", undefined]) {
		assert.ok(
			controlStageFrameFailures([{ ...wireframe, quality }, plan]).some(
				(message) => message.includes("fixed Draft quality"),
			),
		);
	}
	assert.ok(
		controlStageFrameFailures([plan]).some((message) =>
			message.includes("no actual wireframe"),
		),
	);
});
