import { performance } from "node:perf_hooks";
import { setTimeout as delay } from "node:timers/promises";
import { JSON_SCHEMA, load } from "js-yaml";

const actions = new Set(["app", "click", "clickAt", "drag", "fill", "key", "wait", "waitFor", "pause", "marker"]);
const maximumWaitMs = 60 * 60 * 1000;
const maximumTotalWaitMs = 12 * maximumWaitMs;

function fail(where, message) {
  throw new Error(`${where}: ${message}`);
}

function record(value, where) {
  if (value === null || typeof value !== "object" || Array.isArray(value) ||
      ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    fail(where, "expected an object");
  }
  return value;
}

function knownKeys(value, allowed, where) {
  for (const key of Object.keys(value)) {
    if (!allowed.includes(key)) fail(where, `unknown field ${JSON.stringify(key)}`);
  }
}

function text(value, where, { empty = false, maximum = 4096 } = {}) {
  if (typeof value !== "string" || (!empty && !value.trim()) || value.length > maximum) {
    fail(where, `expected ${empty ? "a" : "a non-empty"} string of at most ${maximum} characters`);
  }
  return value;
}

function validBpm(value) {
  if (typeof value !== "number" || !Number.isFinite(value) || value <= 0 || value > 1000) {
    fail("bpm", "expected a finite number greater than zero and at most 1000");
  }
  return value;
}

/** Numeric durations are seconds. Beat and bar strings use the script's BPM and 4/4 time. */
export function parseDuration(value, bpm = 120) {
  validBpm(bpm);
  let milliseconds;
  if (typeof value === "number") {
    milliseconds = value * 1000;
  } else if (typeof value === "string") {
    const match = value.trim().match(/^(\d+(?:\.\d+)?|\.\d+)\s*(ms|s|beats?|bars?)$/i);
    if (!match) fail("wait", "use seconds, 500ms, 2s, 8 beats, or 4 bars");
    const amount = Number(match[1]);
    const unit = match[2].toLowerCase();
    milliseconds = amount * (unit === "ms" ? 1 : unit === "s" ? 1000 :
      unit.startsWith("beat") ? 60_000 / bpm : 240_000 / bpm);
  } else {
    fail("wait", "expected a number of seconds or a duration string");
  }
  if (!Number.isFinite(milliseconds) || milliseconds < 0 || milliseconds > maximumWaitMs) {
    fail("wait", "duration must be finite, non-negative, and at most one hour");
  }
  return milliseconds;
}

function selector(value, where) {
  const input = typeof value === "string" ? { name: value } : record(value, where);
  knownKeys(input, ["name", "match", "role", "within"], where);
  const result = { name: text(input.name, `${where}.name`), match: Object.hasOwn(input, "match") ? input.match : "exact" };
  if (!["exact", "prefix", "contains"].includes(result.match)) {
    fail(`${where}.match`, "expected exact, prefix, or contains");
  }
  if (Object.hasOwn(input, "role")) result.role = text(input.role, `${where}.role`, { maximum: 128 });
  if (Object.hasOwn(input, "within")) result.within = text(input.within, `${where}.within`);
  return result;
}

function point(value, where) {
  const input = record(value, where);
  knownKeys(input, ["x", "y"], where);
  for (const coordinate of ["x", "y"]) {
    const number = input[coordinate];
    if (typeof number !== "number" || !Number.isFinite(number) || number < 0 || number > 100_000) {
      fail(`${where}.${coordinate}`, "expected finite, non-negative window-relative points, at most 100000");
    }
  }
  return { x: input.x, y: input.y };
}

function keyChord(value, where) {
  const chord = text(value, where, { maximum: 128 });
  const parts = chord.split("+").map((part) => part.trim().toLowerCase());
  const key = parts.pop();
  const namedKeys = new Set(["return", "enter", "tab", "space", "backspace", "escape", "esc", "left", "right", "down", "up"]);
  if (!/^[a-z0-9]$/.test(key) && !namedKeys.has(key)) fail(where, "unsupported key; use a letter, digit, Return, Tab, Space, Backspace, Escape, or an arrow key");
  const modifiers = new Map([["cmd", "command"], ["command", "command"], ["super", "command"],
    ["ctrl", "control"], ["control", "control"], ["alt", "option"], ["option", "option"], ["shift", "shift"]]);
  const used = new Set();
  for (const part of parts) {
    if (!modifiers.has(part)) fail(where, `unsupported modifier ${JSON.stringify(part)}`);
    const normalized = modifiers.get(part);
    if (used.has(normalized)) fail(where, `duplicate ${normalized} modifier`);
    used.add(normalized);
  }
  return [...parts, key].join("+");
}

/** Validate the entire take before any UI or timing side effects. Return a detached script. */
export function validateScript(value) {
  const input = record(value, "script");
  knownKeys(input, ["version", "name", "bpm", "apps", "steps"], "script");
  if (input.version !== 1) fail("version", "expected 1");
  const name = text(input.name, "name");
  const bpm = validBpm(Object.hasOwn(input, "bpm") ? input.bpm : 120);
  const appInputs = record(input.apps, "apps");
  const aliases = Object.keys(appInputs);
  if (aliases.length === 0 || aliases.length > 32) fail("apps", "define between 1 and 32 app aliases");
  const apps = {};
  for (const alias of aliases) {
    if (!/^[A-Za-z][A-Za-z0-9_-]*$/.test(alias)) fail("apps", `invalid alias ${JSON.stringify(alias)}`);
    const where = `apps.${alias}`;
    const app = record(appInputs[alias], where);
    knownKeys(app, ["bundleId", "path", "window"], where);
    const bundleId = text(app.bundleId, `${where}.bundleId`, { maximum: 255 });
    if (!/^[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+$/.test(bundleId)) fail(`${where}.bundleId`, "expected a macOS bundle identifier");
    const normalized = { bundleId };
    for (const key of ["path", "window"]) {
      if (Object.hasOwn(app, key)) normalized[key] = text(app[key], `${where}.${key}`);
    }
    apps[alias] = normalized;
  }
  if (!Array.isArray(input.steps) || input.steps.length === 0 || input.steps.length > 10_000) {
    fail("steps", "expected between 1 and 10000 steps");
  }
  let activeAlias = null;
  let totalWaitMs = 0;
  const steps = input.steps.map((item, index) => {
    const where = `step ${index + 1}`;
    const step = record(item, where);
    const keys = Object.keys(step);
    if (keys.length !== 1 || !actions.has(keys[0])) {
      fail(where, `expected exactly one action: ${[...actions].join(", ")}`);
    }
    const action = keys[0];
    const operand = step[action];
    if (action === "app") {
      const alias = text(operand, `${where}.app`, { maximum: 128 });
      if (!Object.hasOwn(apps, alias)) fail(where, `unknown app alias ${JSON.stringify(alias)}`);
      activeAlias = alias;
      return { app: alias };
    }
    if (["click", "clickAt", "drag", "fill", "key", "waitFor"].includes(action) && activeAlias === null) {
      fail(where, `choose an app before ${action}`);
    }
    if (action === "click" || action === "waitFor") return { [action]: selector(operand, `${where}.${action}`) };
    if (action === "clickAt") return { clickAt: point(operand, `${where}.clickAt`) };
    if (action === "drag") {
      const drag = record(operand, `${where}.drag`);
      knownKeys(drag, ["from", "to", "duration"], `${where}.drag`);
      const duration = Object.hasOwn(drag, "duration") ? drag.duration : "1s";
      const durationMs = parseDuration(duration, bpm);
      if (durationMs <= 0 || durationMs > 10_000) fail(`${where}.drag.duration`, "drag duration must be greater than zero and at most ten seconds");
      return { drag: { from: point(drag.from, `${where}.drag.from`), to: point(drag.to, `${where}.drag.to`), duration } };
    }
    if (action === "fill") {
      const field = record(operand, `${where}.fill`);
      knownKeys(field, ["name", "text", "match"], `${where}.fill`);
      const target = selector({ name: field.name, ...(Object.hasOwn(field, "match") ? { match: field.match } : {}) }, `${where}.fill`);
      return { fill: { ...target, text: text(field.text, `${where}.fill.text`, { empty: true, maximum: 4096 }) } };
    }
    if (action === "wait") {
      totalWaitMs += parseDuration(operand, bpm);
      if (totalWaitMs > maximumTotalWaitMs) fail("steps", "total explicit waits exceed twelve hours");
      return { wait: operand };
    }
    if (action === "key") return { key: keyChord(operand, `${where}.key`) };
    return { [action]: text(operand, `${where}.${action}`) };
  });
  return { version: 1, name, bpm, apps, steps };
}

/** YAML and JSON use the same deliberately small data-only schema. */
export function parseScript(source, { filename = "demo script" } = {}) {
  if (typeof source !== "string") fail(filename, "expected script text");
  let input;
  try {
    input = load(source, { schema: JSON_SCHEMA, filename });
  } catch (error) {
    throw new Error(`${filename}: ${error.message}`, { cause: error });
  }
  return validateScript(input);
}

export class ScriptExecutionError extends Error {
  constructor(message, step, report, cause) {
    super(message, { cause });
    this.name = "ScriptExecutionError";
    this.step = step;
    this.report = report;
  }
}

function describe(action, operand) {
  if (["click", "fill", "waitFor"].includes(action)) return `${action} ${JSON.stringify(operand.name)}`;
  return `${action} ${JSON.stringify(operand)}`;
}

function throwIfAborted(signal) {
  if (signal?.aborted) {
    const error = new Error(signal.reason?.message ?? "Script stopped", { cause: signal.reason });
    error.name = "AbortError";
    throw error;
  }
}

/** Execute visible UI operations in order. Waits start only after the preceding operation ends. */
export async function executeScript(value, adapter, options = {}) {
  const script = validateScript(value);
  const needed = new Set(script.steps.map((step) => Object.keys(step)[0]).map((action) => action === "app" ? "focus" : action));
  for (const method of ["focus", "click", "clickAt", "drag", "fill", "key", "waitFor"]) {
    if (needed.has(method) && typeof adapter?.[method] !== "function") fail("adapter", `missing ${method} method`);
  }
  if (needed.has("pause") && typeof options.pause !== "function") fail("pause", "this script requires an interactive pause handler");
  const clock = options.clock ?? (() => performance.now());
  const sleep = options.sleep ?? ((milliseconds, { signal }) => delay(milliseconds, undefined, { signal }));
  const log = options.log ?? (() => {});
  const pause = options.pause ?? (() => { throw new Error("This script needs an interactive pause handler"); });
  const origin = clock();
  const report = { name: script.name, status: "running", elapsedMs: 0, steps: [] };
  let activeAlias = null;
  for (const [index, step] of script.steps.entries()) {
    const action = Object.keys(step)[0];
    const operand = step[action];
    const entry = { index: index + 1, action, app: action === "app" ? operand : activeAlias,
      description: describe(action, operand), status: "pending" };
    report.steps.push(entry);
    try {
      throwIfAborted(options.signal);
      // The zero-based hook lets a CLI require Enter before each operation. Its manual hold is
      // part of take time, but is excluded from this operation's measured execution duration.
      await options.beforeStep?.(step, index);
      throwIfAborted(options.signal);
      entry.startedAtMs = clock() - origin;
      entry.status = "running";
      log({ event: "step-start", ...entry });
      const app = activeAlias === null ? null : { ...script.apps[activeAlias], alias: activeAlias };
      if (action === "app") {
        await adapter.focus({ ...script.apps[operand], alias: operand });
        activeAlias = operand;
      } else if (action === "drag") {
        await adapter.drag(app, { from: operand.from, to: operand.to, durationMs: parseDuration(operand.duration, script.bpm) });
      } else if (["click", "clickAt", "fill", "key", "waitFor"].includes(action)) {
        await adapter[action](app, operand);
      } else if (action === "wait") {
        await sleep(parseDuration(operand, script.bpm), { signal: options.signal });
      } else if (action === "pause") {
        await pause(operand, { step: index + 1, app, signal: options.signal });
      }
      throwIfAborted(options.signal);
      entry.endedAtMs = clock() - origin;
      entry.elapsedMs = entry.endedAtMs - entry.startedAtMs;
      entry.status = "completed";
      log({ event: "step-end", ...entry });
    } catch (error) {
      entry.endedAtMs = clock() - origin;
      entry.startedAtMs ??= entry.endedAtMs;
      entry.elapsedMs = entry.endedAtMs - entry.startedAtMs;
      entry.status = options.signal?.aborted || error?.name === "AbortError" ? "aborted" : "failed";
      entry.error = error instanceof Error ? error.message : String(error);
      report.status = entry.status;
      report.elapsedMs = entry.endedAtMs;
      // A failing observer must not replace the actual UI/step failure.
      try { log({ event: "step-error", ...entry }); } catch {}
      throw new ScriptExecutionError(`Step ${entry.index} (${entry.description}) failed: ${entry.error}`, entry, report, error);
    }
  }
  report.status = "completed";
  report.elapsedMs = clock() - origin;
  return report;
}
