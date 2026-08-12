import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const runner = await readFile(
	new URL("./run-packaged-stage-benchmark.mjs", import.meta.url),
	"utf8",
);
const scenarios = await readFile(
	new URL("../docs/testing/16-stage-performance.md", import.meta.url),
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
