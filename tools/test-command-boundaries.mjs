// Ratchets the public acceptance-test command boundary.
//
// Acceptance scenarios should state operator intent through typed v2 HTTP helpers, the v2
// command-line HTTP contract, visible software keys, or exact OSC keys and phases. Three shapes
// are deliberately bounded:
//
//   1. The raw `executeLegacyCommandLine` helper is gone. It must not come back.
//   2. Every direct literal v1 WebSocket action is inventoried by file and action family. The
//      centralized sender is excluded; retained compatibility calls are explicit in the baseline.
//   3. Command families without a production boundary remain explicit compatibility intents.
//
// Command families whose production boundary does not exist yet still route through
// `executeCompatibilityProgrammerCommand`, which names the missing owner. That surface is allowed
// but ratcheted, so it can only shrink as typed services land.

import fs from "node:fs";
import path from "node:path";

export const COMPATIBILITY_FAMILIES = Object.freeze([
  "cue_navigation",
  "speed_group",
  "cue_delete",
  "preset_delete",
  "preset_transfer",
  "update",
]);

/** The bench API driver owns the centralized v1 senders, so it is not a scenario call site. */
export const CENTRALIZED_SENDER = "apps/control-ui/e2e/bench/api.ts";

const LEGACY_HELPER = /\bexecuteLegacyCommandLine\b/gu;
const DIRECT_ACTION_COMMAND =
  /\.command\s*(?:<[^>]*>)?\s*\(\s*(["'])([a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+)\1/gu;
const COMPATIBILITY_HELPER = /\bexecuteCompatibilityProgrammerCommand\s*\(/gu;

function occurrences(source, expression) {
  return source.match(expression)?.length ?? 0;
}

function familyOccurrences(source, family) {
  return occurrences(source, new RegExp(`family:\\s*["']${family}["']`, "gu"));
}

function directActions(source) {
  return [...source.matchAll(DIRECT_ACTION_COMMAND)].map((match) => match[2]);
}

/**
 * Counts the bounded command shapes across the supplied acceptance-test sources.
 *
 * `files` is a list of `{ path, source }` records using repository-relative paths, so the scan
 * itself stays free of filesystem access and is directly unit-testable.
 */
export function scanTestCommandBoundaries(files) {
  const scan = {
    legacyHelperCalls: 0,
    directActionCommands: {},
    directActionFamilies: {},
    compatibilityCommands: {},
    compatibilityFamilies: {},
  };
  for (const { path: name, source } of files) {
    scan.legacyHelperCalls += occurrences(source, LEGACY_HELPER);
    // The centralized sender declares the families and owns the raw envelope; only scenario call
    // sites are ratcheted.
    if (name === CENTRALIZED_SENDER) continue;
    const actions = directActions(source);
    if (actions.length > 0) scan.directActionCommands[name] = actions.length;
    for (const action of actions)
      scan.directActionFamilies[action] = (scan.directActionFamilies[action] ?? 0) + 1;
    const compatibility = occurrences(source, COMPATIBILITY_HELPER);
    if (compatibility > 0) scan.compatibilityCommands[name] = compatibility;
    for (const family of COMPATIBILITY_FAMILIES) {
      const count = familyOccurrences(source, family);
      if (count > 0)
        scan.compatibilityFamilies[family] = (scan.compatibilityFamilies[family] ?? 0) + count;
    }
  }
  return scan;
}

function compareCounts(current, allowances, label, failures) {
  for (const [key, count] of Object.entries(current)) {
    const allowance = allowances?.[key];
    if (allowance === undefined) {
      failures.push(`new ${label}: ${key} has ${count}; route it through a typed intent helper`);
    } else if (count > allowance) {
      failures.push(`${label} grew: ${key} has ${count} (baseline ${allowance})`);
    } else if (count < allowance) {
      failures.push(
        `${label} shrank: ${key} has ${count} (baseline ${allowance}); lower or regenerate the baseline`,
      );
    }
  }
  for (const key of Object.keys(allowances ?? {})) {
    if (current[key] === undefined) failures.push(`stale ${label} baseline entry: ${key}`);
  }
}

/** Fails on legacy calls and any bounded count mismatch until its baseline is lowered exactly. */
export function evaluateTestCommandBoundaries(scan, baseline) {
  const failures = [];
  if (baseline?.version !== 2) failures.push("test command boundary baseline version must be 2");
  if (scan.legacyHelperCalls > 0)
    failures.push(
      `executeLegacyCommandLine is retired but appears ${scan.legacyHelperCalls} time(s); ` +
        "use executeCommandLine or executeCompatibilityProgrammerCommand",
    );
  compareCounts(
    scan.directActionCommands,
    baseline?.directActionCommands,
    "direct v1 WebSocket action call site",
    failures,
  );
  compareCounts(
    scan.directActionFamilies,
    baseline?.directActionFamilies,
    "direct v1 WebSocket action family use",
    failures,
  );
  compareCounts(
    scan.compatibilityCommands,
    baseline?.compatibilityCommands,
    "compatibility command call site",
    failures,
  );
  compareCounts(
    scan.compatibilityFamilies,
    baseline?.compatibilityFamilies,
    "compatibility command family use",
    failures,
  );
  return failures;
}

function walk(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(target) : [target];
  });
}

/** Reads every acceptance-test source under the scanned roots. */
export function readTestSources(repositoryRoot, roots = ["tests", "apps/control-ui/e2e"]) {
  return roots
    .flatMap((root) => walk(path.join(repositoryRoot, root)))
    .filter((candidate) => /\.[cm]?tsx?$/u.test(candidate))
    .map((candidate) => ({
      path: path.relative(repositoryRoot, candidate).split(path.sep).join("/"),
      source: fs.readFileSync(candidate, "utf8"),
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
}

function sortedEntries(entries) {
  return Object.fromEntries(
    Object.entries(entries).sort(([left], [right]) => left.localeCompare(right)),
  );
}

export function baselineFor(scan) {
  return {
    version: 2,
    directActionCommands: sortedEntries(scan.directActionCommands),
    directActionFamilies: sortedEntries(scan.directActionFamilies),
    compatibilityCommands: sortedEntries(scan.compatibilityCommands),
    compatibilityFamilies: sortedEntries(scan.compatibilityFamilies),
  };
}
