import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { classifyPerformance } from "./run-release-performance.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const result = (requiredFloorMet, gateMet = true, patchGateMet = true) => ({
  report: {
    required_floor_met: requiredFloorMet,
    show_mutation: { gate_met: gateMet },
    patch_mutation: patchGateMet == null ? null : { gate_met: patchGateMet },
  },
});

test("unknown is reserved for missing or invalid benchmark evidence", () => {
  assert.equal(classifyPerformance(null, null).status, "unknown");
  assert.equal(classifyPerformance({ report: {} }, null).status, "unknown");
});

test("missing the required floor or either mutation gate is degraded", () => {
  assert.equal(classifyPerformance(result(false), null).status, "degraded");
  assert.equal(classifyPerformance(result(true, false), null).status, "degraded");
  assert.equal(classifyPerformance(result(true, true, false), null).status, "degraded");
  assert.equal(classifyPerformance(result(true, true, null), null).status, "unknown");
});

test("passing the required floor is healthy regardless of the optional capacity probe", () => {
  assert.equal(classifyPerformance(result(true), null).status, "healthy");
  assert.equal(classifyPerformance(result(true), result(false)).status, "healthy");
  assert.equal(classifyPerformance(result(true), result(true)).status, "healthy");
});

test("release workflow publishes before measuring and Pages consumes the status", () => {
  const workflow = readFileSync(resolve(ROOT, ".github/workflows/release.yml"), "utf8");
  const release = /^  release:\n([\s\S]*?)(?=^  [\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";
  const performance =
    /^  release-performance:\n([\s\S]*?)(?=^  [\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";
  const pages = /^  pages-build:\n([\s\S]*?)(?=^  [\w-]+:\n)/mu.exec(workflow)?.[1] ?? "";

  assert.match(release, /needs:[\s\S]*?- build/u);
  assert.doesNotMatch(release, /- benchmark|- pages-build/u);
  assert.match(performance, /needs: \[metadata, release\]/u);
  assert.match(performance, /gh release download/u);
  assert.match(performance, /tools\/run-release-performance\.mjs/u);
  assert.match(performance, /continue-on-error: true/u);
  assert.match(pages, /release-performance/u);
  assert.match(pages, /LIGHT_PERFORMANCE_STATUS_FILE/u);
  const renderer = readFileSync(resolve(ROOT, "tools/render-landing-page.mjs"), "utf8");
  assert.match(renderer, /performance", "index\.html/u);
  assert.match(renderer, /Persisted Patch transaction/u);
});
