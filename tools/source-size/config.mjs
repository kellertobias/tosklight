export const LIMITS = Object.freeze({
  file: 1_200,
  fileGoal: 400,
  function: 150,
  functionGoal: 20,
});

// These are the only text files excluded from line limits. They are rewritten by
// package managers or deterministic generators, so splitting them would corrupt their format,
// or they are isolated prototypes that no production code imports.
const MACHINE_MANAGED_LOCKFILES = new Set([
  "Cargo.lock",
  "bun.lock",
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
]);
const TAURI_SCHEMA = /^apps\/[^/]+\/src-tauri\/gen\/schemas\/[^/]+\.json$/u;
const WIRE_SCHEMA = /^crates\/wire\/schemas\/[^/]+\/[^/]+\.schema\.json$/u;
// Standalone clickable prototypes under `experiments/`. They are deliberately isolated from
// `apps/` and `crates/`: no imports, packages, API calls, persistence, or backend behavior, and
// nothing in production references them. They explore layouts, so splitting them by responsibility
// would defeat their purpose. Promote an experiment into `apps/` before it earns the limits.
const EXPERIMENT = /^experiments\//u;
const TEST_DIRECTORY = /(^|\/)(?:__tests__|e2e|tests)(?:\/|$)/u;
const TEST_FILENAME = /(?:^|\.)\b(?:spec|test)\.[^.]+$/u;

export function exemptionReason(repositoryPath) {
  const basename = repositoryPath.split("/").at(-1);
  if (MACHINE_MANAGED_LOCKFILES.has(basename)) return "machine-managed lockfile";
  if (TAURI_SCHEMA.test(repositoryPath)) return "Tauri-generated schema JSON";
  if (WIRE_SCHEMA.test(repositoryPath)) return "Rust-generated wire schema JSON";
  if (EXPERIMENT.test(repositoryPath)) return "isolated experiment prototype";
  return undefined;
}

export function isTestSource(repositoryPath) {
  const basename = repositoryPath.split("/").at(-1) ?? "";
  return TEST_DIRECTORY.test(repositoryPath) || TEST_FILENAME.test(basename);
}

export function functionLanguage(repositoryPath, source) {
  const extension = repositoryPath.split(".").at(-1)?.toLowerCase();
  if (extension === "rs") return "rust";
  if (["cjs", "cts", "js", "jsx", "mjs", "mts", "ts", "tsx"].includes(extension))
    return "javascript";
  if (extension === "py") return "python";
  if (extension === "sh") return "shell";
  if (/^#!.*\b(?:ba|z|k)?sh\b/u.test(source.slice(0, 200))) return "shell";
  return undefined;
}
