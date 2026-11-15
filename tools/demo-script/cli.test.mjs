import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const cli = fileURLToPath(new URL("./cli.mjs", import.meta.url));
const starter = fileURLToPath(new URL("../../docs/marketing/video/preset-demo.yaml", import.meta.url));
const run = (...args) => spawnSync(process.execPath, [cli, ...args], { encoding: "utf8", timeout: 5000 });

test("the shipped take can be validated without native permissions or app activation", () => {
  const result = run(starter, "--dry-run");
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, "");
  const script = JSON.parse(result.stdout);
  assert.equal(script.bpm, 120);
  assert.equal(script.steps.at(-1).marker, "presets-end");
});

test("missing inspect aliases and invalid countdowns fail before native execution", () => {
  for (const [args, message] of [
    [[starter, "--inspect"], /--inspect requires an app alias/],
    [[starter, "--countdown", "-1"], /Countdown must be between/],
  ]) {
    const result = run(...args);
    assert.equal(result.status, 1);
    assert.match(result.stderr, message);
    assert.equal(result.stdout, "");
  }
});
