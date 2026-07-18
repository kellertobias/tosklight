#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { LIMITS } from "./source-size/config.mjs";
import { baselineFor, evaluateRatcheting, serializeBaseline } from "./source-size/ratchet.mjs";
import { scanRepository } from "./source-size/repository.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = path.join(repositoryRoot, "tools/source-size/baseline.json");
const command = process.argv[2];

function readBaseline() {
  try {
    return JSON.parse(fs.readFileSync(baselinePath, "utf8"));
  } catch (error) {
    console.error(`size error: cannot read ${path.relative(repositoryRoot, baselinePath)}: ${error.message}`);
    process.exit(1);
  }
}

function reportFailures(failures) {
  for (const failure of failures) console.error(`size error: ${failure}`);
  console.error("size error: split new/grown violations; after reducing legacy violations, run with --ratchet");
}

function reportSuccess(result, scan) {
  console.log(
    `Source size ratchet is valid: ${result.violations.files.length} legacy files above ${LIMITS.file} lines; ` +
    `${result.violations.functions.length} legacy functions above ${LIMITS.function} lines.`,
  );
  console.log(
    `Source size goals: ${result.goals.files} files above ${LIMITS.fileGoal} lines; ` +
    `${result.goals.functions} functions above ${LIMITS.functionGoal} lines. ` +
    `${scan.exemptions.length} machine-managed files exempt.`,
  );
}

const scan = scanRepository(repositoryRoot);
if (command === "--print-baseline") {
  process.stdout.write(serializeBaseline(baselineFor(scan)));
  process.exit(0);
}

const baseline = readBaseline();
const result = evaluateRatcheting(scan, baseline, { allowStale: command === "--ratchet" });
if (result.failures.length > 0) {
  reportFailures(result.failures);
  process.exit(1);
}
if (command === "--ratchet") {
  fs.writeFileSync(baselinePath, serializeBaseline(baselineFor(scan)));
  console.log(`Reduced ${path.relative(repositoryRoot, baselinePath)} to current legacy violations.`);
} else if (command !== undefined) {
  console.error(`size error: unknown argument ${command}`);
  process.exit(2);
}
reportSuccess(result, scan);
