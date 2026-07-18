import { LIMITS } from "./config.mjs";

function hardViolations(scan) {
  return {
    files: scan.files.filter((candidate) => candidate.lines > LIMITS.file),
    functions: scan.functions.filter((candidate) => candidate.lines > LIMITS.function),
  };
}

function baselineMap(entries, kind, failures) {
  if (!entries || typeof entries !== "object" || Array.isArray(entries)) {
    failures.push(`baseline ${kind} must be an object`);
    return new Map();
  }
  const result = new Map();
  for (const [id, lines] of Object.entries(entries)) {
    if (!Number.isInteger(lines) || lines < 1) failures.push(`baseline ${kind} entry ${id} has an invalid line count`);
    else result.set(id, lines);
  }
  return result;
}

export function validateBaseline(baseline) {
  const failures = [];
  if (baseline?.version !== 1) failures.push("baseline version must be 1");
  if (baseline?.limits?.file !== LIMITS.file) failures.push(`baseline file limit must be ${LIMITS.file}`);
  if (baseline?.limits?.function !== LIMITS.function)
    failures.push(`baseline function limit must be ${LIMITS.function}`);
  const files = baselineMap(baseline?.files, "files", failures);
  const functions = baselineMap(baseline?.functions, "functions", failures);
  return { failures, files, functions };
}

function compareCandidates(candidates, allowances, kind, failures) {
  const currentIds = new Set();
  for (const candidate of candidates) {
    currentIds.add(candidate.id);
    const allowance = allowances.get(candidate.id);
    const location = kind === "function"
      ? `${candidate.id.split("::")[0]}:${candidate.startLine} ${candidate.display}`
      : candidate.path;
    if (allowance === undefined) failures.push(`new ${kind} violation: ${location} is ${candidate.lines} lines`);
    else if (candidate.lines > allowance)
      failures.push(`${kind} violation grew: ${location} is ${candidate.lines} lines (baseline ${allowance})`);
  }
  return currentIds;
}

function staleEntries(allowances, currentIds, kind) {
  return [...allowances.keys()]
    .filter((id) => !currentIds.has(id))
    .map((id) => `stale ${kind} baseline entry: ${id}`);
}

export function evaluateRatcheting(scan, baseline, { allowStale = false } = {}) {
  const validation = validateBaseline(baseline);
  const failures = [...validation.failures];
  const violations = hardViolations(scan);
  const fileIds = compareCandidates(violations.files, validation.files, "file", failures);
  const functionIds = compareCandidates(violations.functions, validation.functions, "function", failures);
  const stale = [
    ...staleEntries(validation.files, fileIds, "file"),
    ...staleEntries(validation.functions, functionIds, "function"),
  ];
  if (!allowStale) failures.push(...stale);
  return {
    failures,
    stale,
    violations,
    goals: {
      files: scan.files.filter((candidate) => candidate.lines > LIMITS.fileGoal).length,
      functions: scan.functions.filter((candidate) => candidate.lines > LIMITS.functionGoal).length,
    },
  };
}

function sortedEntries(entries) {
  return Object.fromEntries(entries.sort(([left], [right]) => left.localeCompare(right)));
}

export function baselineFor(scan) {
  const violations = hardViolations(scan);
  return {
    version: 1,
    limits: { file: LIMITS.file, function: LIMITS.function },
    files: sortedEntries(violations.files.map((candidate) => [candidate.id, candidate.lines])),
    functions: sortedEntries(violations.functions.map((candidate) => [candidate.id, candidate.lines])),
  };
}

export function serializeBaseline(baseline) {
  const lines = [
    "{",
    `  \"version\": ${baseline.version},`,
    `  \"limits\": ${JSON.stringify(baseline.limits)},`,
    "  \"files\": {",
    ...Object.entries(baseline.files).map(([id, count], index, all) =>
      `    ${JSON.stringify(id)}: ${count}${index === all.length - 1 ? "" : ","}`),
    "  },",
    "  \"functions\": {",
    ...Object.entries(baseline.functions).map(([id, count], index, all) =>
      `    ${JSON.stringify(id)}: ${count}${index === all.length - 1 ? "" : ","}`),
    "  }",
    "}",
  ];
  return `${lines.join("\n")}\n`;
}
