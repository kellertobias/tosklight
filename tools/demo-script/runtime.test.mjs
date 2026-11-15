import assert from "node:assert/strict";
import test from "node:test";
import { executeScript, parseDuration, parseScript, ScriptExecutionError, validateScript } from "./runtime.mjs";

const base = () => ({ version: 1, name: "Preset take", bpm: 120,
  apps: { control: { bundleId: "de.tokenet.light" }, architect: { bundleId: "de.tokenet.architect", window: "CAD" } },
  steps: [{ app: "control" }, { click: "Groups" }, { wait: "8 beats" }] });

test("YAML and JSON scripts validate to the same detached, data-only schema", () => {
  const yaml = `version: 1
name: Preset take
apps:
  control:
    bundleId: de.tokenet.light
steps:
  - app: control
  - click:
      name: Beam Show
      match: contains
      role: AXCheckBox
      within: Groups
  - fill:
      name: Show name
      text: ""
  - wait: 4 bars
  - marker: final-look
  - pause: Finish placing fixtures, then press Enter.
`;
  const script = parseScript(yaml);
  assert.equal(script.bpm, 120);
  assert.deepEqual(script.steps[1].click, { name: "Beam Show", match: "contains", role: "AXCheckBox", within: "Groups" });
  assert.deepEqual(parseScript(JSON.stringify(script)), script);
  const source = base();
  const validated = validateScript(source);
  source.apps.control.bundleId = "de.other.app";
  source.steps[1].click = "Different";
  assert.equal(validated.apps.control.bundleId, "de.tokenet.light");
  assert.equal(validated.steps[1].click.name, "Groups");
});

test("durations use seconds or explicit units and beat/bar timing is exact", () => {
  assert.equal(parseDuration(2), 2000);
  assert.equal(parseDuration("500ms"), 500);
  assert.equal(parseDuration(".5s"), 500);
  assert.equal(parseDuration("8 beats", 120), 4000);
  assert.equal(parseDuration("4 bars", 120), 8000);
  assert.equal(parseDuration("1 beat", 60), 1000);
  assert.equal(parseDuration("1 bar", 60), 4000);
  assert.equal(parseDuration(0), 0);
  for (const value of [-1, NaN, Infinity, "-1s", "2", "one second", "1h", "3601s", null, {}, true]) {
    assert.throws(() => parseDuration(value), /wait:/, String(value));
  }
});

test("schema rejects malformed, ambiguous, empty, and unknown input", () => {
  for (const value of [null, [], {}, { ...base(), version: 2 }, { ...base(), name: "" },
    { ...base(), extra: true }, { ...base(), apps: {} }, { ...base(), steps: [] },
    { ...base(), bpm: 0 }, { ...base(), bpm: Infinity }, { ...base(), bpm: "120" }, { ...base(), bpm: null },
    { ...base(), steps: [{ app: "missing" }] }, { ...base(), steps: [{ click: "Groups" }] },
    { ...base(), steps: [{ app: "control", wait: 1 }] }, { ...base(), steps: [{ unknown: "x" }] },
    { ...base(), steps: [{ app: "control" }, { click: { name: "x", match: "regex" } }] },
    { ...base(), steps: [{ app: "control" }, { click: { name: "x", match: null } }] },
    { ...base(), steps: [{ app: "control" }, { click: { name: "x", within: { name: "pane" } } }] },
    { ...base(), steps: [{ app: "control" }, { fill: { name: "x", text: "x", typo: true } }] },
    { ...base(), steps: [{ app: "control" }, { key: "PageDown" }] },
    { ...base(), steps: [{ app: "control" }, { key: "hyper+a" }] },
    { ...base(), steps: [{ app: "control" }, { key: "cmd+super+a" }] },
    { ...base(), steps: [{ app: "control" }, { key: "cmd+" }] },
    { ...base(), steps: [{ app: "control" }, { fill: { name: "x", text: "x".repeat(4097) } }] },
    { ...base(), steps: [{ clickAt: { x: 0, y: 0 } }] },
    { ...base(), steps: [{ app: "control" }, { clickAt: { x: -1, y: 2 } }] },
    { ...base(), steps: [{ app: "control" }, { clickAt: { x: 1, y: Infinity } }] },
    { ...base(), steps: [{ app: "control" }, { drag: { from: { x: 1, y: 2 }, to: { x: 3, y: 4 }, duration: 11 } }] },
    { ...base(), steps: [{ app: "control" }, { drag: { from: { x: 1, y: 2 }, to: { x: 3, y: 4 }, duration: 0 } }] },
    { ...base(), steps: [{ app: "control" }, { drag: { from: { x: 1, y: 2 }, to: { x: 3, y: 4 }, duration: null } }] },
    { ...base(), steps: [{ pause: true }] }, { ...base(), apps: { control: { bundleId: "not a bundle" } } }]) {
    assert.throws(() => validateScript(value));
  }
  assert.throws(() => parseScript(""), /expected an object/);
  assert.throws(() => parseScript("version: 1\nversion: 1"), /duplicated mapping key/);
  assert.throws(() => parseScript("!!js/function 'function(){}'"), /unknown tag/);
  assert.throws(() => validateScript({ ...base(), steps: Array.from({ length: 13 }, () => ({ wait: 3600 })) }), /twelve hours/);
});

function harness() {
  let now = 100;
  const calls = [];
  const adapter = Object.fromEntries(["focus", "click", "clickAt", "drag", "fill", "key", "waitFor"].map((method) => [method,
    async (...args) => { calls.push([method, ...args]); now += 25; }]));
  const events = [];
  const options = { clock: () => now, sleep: async (milliseconds) => { calls.push(["sleep", milliseconds]); now += milliseconds; },
    log: (entry) => events.push(entry), pause: async (message) => { calls.push(["pause", message]); now += 1000; } };
  return { adapter, calls, events, options };
}

test("executes visible operations, app switches, manual pauses, and extra holds in order", async () => {
  const h = harness();
  const script = { ...base(), steps: [{ marker: "opening" }, { wait: "500ms" }, { app: "control" },
    { click: "Groups" }, { wait: "8 beats" }, { fill: { name: "Show name", text: "Demo" } },
    { key: "Return" }, { app: "architect" }, { waitFor: { name: "Synced", role: "AXStaticText" } },
    { pause: "Place the other fixtures" }, { marker: "complete" }] };
  const report = await executeScript(script, h.adapter, h.options);
  assert.deepEqual(h.calls.map((call) => call[0]), ["sleep", "focus", "click", "sleep", "fill", "key", "focus", "waitFor", "pause"]);
  assert.equal(h.calls[3][1], 4000);
  assert.equal(h.calls[2][1].alias, "control");
  assert.equal(h.calls[7][1].alias, "architect");
  assert.equal(h.calls[7][1].window, "CAD");
  assert.equal(report.status, "completed");
  assert.equal(report.steps[4].startedAtMs, 550);
  assert.equal(report.steps[4].elapsedMs, 4000);
  assert.equal(report.steps.length, script.steps.length);
  assert.ok(h.events.every((event) => ["step-start", "step-end"].includes(event.event)));
  assert.equal(h.events.length, script.steps.length * 2);
  assert.ok(h.events.every((event) => !event.description.includes("Demo")), "typed text is omitted from progress logs");
});

test("canvas actions retain window-relative points and normalize bounded drag timing", async () => {
  const h = harness();
  const script = { ...base(), steps: [{ app: "architect" }, { clickAt: { x: 0, y: 250.5 } },
    { drag: { from: { x: 100, y: 200 }, to: { x: 300, y: 400 }, duration: "2 beats" } },
    { drag: { from: { x: 300, y: 400 }, to: { x: 400, y: 400 } } }] };
  const validated = validateScript(script);
  assert.deepEqual(validateScript(validated), validated, "validated scripts can be executed without schema changes");
  await executeScript(validated, h.adapter, h.options);
  assert.deepEqual(h.calls[1], ["clickAt", { bundleId: "de.tokenet.architect", window: "CAD", alias: "architect" }, { x: 0, y: 250.5 }]);
  assert.equal(h.calls[2][2].durationMs, 1000);
  assert.equal(h.calls[3][2].durationMs, 1000);
});

test("supported key chords validate before native execution", () => {
  for (const key of ["a", "9", "Return", "Enter", "super+a", "Cmd+Shift+r", "ctrl+alt+Left", "option+Escape", "Space", "backspace"]) {
    const script = { ...base(), steps: [{ app: "control" }, { key }] };
    assert.equal(validateScript(script).steps[1].key, key.toLowerCase());
  }
  assert.equal(validateScript({ ...base(), steps: [{ app: "control" }, { key: "Cmd + Shift + a" }] }).steps[1].key, "cmd+shift+a");
});

test("beforeStep can gate each operation before its start event and measured duration", async () => {
  const h = harness();
  const gated = [];
  const report = await executeScript(base(), h.adapter, { ...h.options,
    beforeStep: async (step, index) => {
      gated.push([Object.keys(step)[0], index]);
      assert.equal(h.events.filter((event) => event.event === "step-start").length, index);
      await h.options.sleep(500);
    } });
  assert.deepEqual(gated, [["app", 0], ["click", 1], ["wait", 2]]);
  assert.equal(report.steps[0].startedAtMs, 500);
  assert.equal(report.steps[0].elapsedMs, 25);
  assert.equal(report.steps[2].elapsedMs, 4000);
});

test("validates the complete script and adapter before performing any operation", async () => {
  const h = harness();
  const invalid = { ...base(), steps: [...base().steps, { fill: { name: "Field" } }] };
  await assert.rejects(executeScript(invalid, h.adapter, h.options));
  assert.deepEqual(h.calls, []);
  assert.deepEqual(h.events, []);
  await assert.rejects(executeScript(base(), { focus() {} }, h.options), /missing click/);
  await assert.rejects(executeScript({ ...base(), steps: [{ app: "control" }, { pause: "Continue" }] }, h.adapter), /pause handler/);
  assert.deepEqual(h.calls, []);
});

test("a failed UI action reports the step and prevents all later actions", async () => {
  const h = harness();
  h.adapter.click = async () => { h.calls.push(["failed-click"]); throw new Error("2 matching controls"); };
  await assert.rejects(executeScript(base(), h.adapter, h.options), (error) => {
    assert.ok(error instanceof ScriptExecutionError);
    assert.match(error.message, /Step 2.*Groups.*2 matching controls/);
    assert.equal(error.step.index, 2);
    assert.equal(error.report.status, "failed");
    assert.equal(error.report.steps.length, 2);
    return true;
  });
  assert.deepEqual(h.calls.map((call) => call[0]), ["focus", "failed-click"]);
  assert.equal(h.events.at(-1).event, "step-error");
});

test("cancellation before a take and during a hold prevents later UI actions", async () => {
  const first = harness();
  const alreadyStopped = new AbortController();
  alreadyStopped.abort();
  await assert.rejects(executeScript(base(), first.adapter, { ...first.options, signal: alreadyStopped.signal }),
    (error) => error.report.status === "aborted");
  assert.deepEqual(first.calls, []);

  const second = harness();
  const controller = new AbortController();
  const script = { ...base(), steps: [{ app: "control" }, { wait: 1 }, { click: "Groups" }] };
  await assert.rejects(executeScript(script, second.adapter, { ...second.options, signal: controller.signal,
    sleep: async (_milliseconds, { signal }) => { assert.equal(signal, controller.signal); controller.abort(); } }),
    (error) => error.report.status === "aborted" && error.step.index === 2);
  assert.deepEqual(second.calls.map((call) => call[0]), ["focus"]);
});
