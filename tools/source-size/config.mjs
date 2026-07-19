export const LIMITS = Object.freeze({
  file: 1_200,
  fileGoal: 400,
  function: 150,
  functionGoal: 20,
});

// These are the only text files excluded from line limits. They are rewritten by
// package managers or deterministic generators, so splitting them would corrupt their format.
const MACHINE_MANAGED_LOCKFILES = new Set([
  "Cargo.lock",
  "bun.lock",
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
]);
const TAURI_SCHEMA = /^apps\/[^/]+\/src-tauri\/gen\/schemas\/[^/]+\.json$/u;
const WIRE_SCHEMA = /^crates\/wire\/schemas\/[^/]+\/[^/]+\.schema\.json$/u;
const TEST_DIRECTORY = /(^|\/)(?:__tests__|e2e|tests)(?:\/|$)/u;
const TEST_FILENAME = /(?:^|\.)\b(?:spec|test)\.[^.]+$/u;

export function exemptionReason(repositoryPath) {
  const basename = repositoryPath.split("/").at(-1);
  if (MACHINE_MANAGED_LOCKFILES.has(basename)) return "machine-managed lockfile";
  if (TAURI_SCHEMA.test(repositoryPath)) return "Tauri-generated schema JSON";
  if (WIRE_SCHEMA.test(repositoryPath)) return "Rust-generated wire schema JSON";
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
