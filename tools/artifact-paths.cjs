const fs = require("node:fs");
const path = require("node:path");

function findRepositoryRoot(start) {
  let candidate = path.resolve(start);
  while (true) {
    if (fs.existsSync(path.join(candidate, "tools", "artifact-layout.conf"))) return candidate;
    const parent = path.dirname(candidate);
    if (parent === candidate) throw new Error(`Could not find tools/artifact-layout.conf above ${start}`);
    candidate = parent;
  }
}

const repositoryRoot = findRepositoryRoot(process.env.LIGHT_REPOSITORY_ROOT || process.cwd());
const layout = Object.fromEntries(
  fs.readFileSync(path.join(repositoryRoot, "tools", "artifact-layout.conf"), "utf8")
    .split(/\r?\n/)
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => {
      const separator = line.indexOf("=");
      return [line.slice(0, separator), line.slice(separator + 1)];
    }),
);
const configured = (environment, fallback) => {
  const value = Object.hasOwn(process.env, environment) ? process.env[environment] : fallback;
  if (!value) throw new Error(`${environment} cannot be empty`);
  return value;
};
const absolute = (value) => path.resolve(repositoryRoot, value);
const artifactRoot = absolute(configured("LIGHT_ARTIFACTS_DIR", ".artifacts"));
const artifact = (environment, key) => absolute(configured(environment, path.join(artifactRoot, layout[key])));
const artifactPaths = Object.freeze({
  root: artifactRoot,
  cargo: absolute(configured("CARGO_TARGET_DIR", path.join(artifactRoot, layout.BUILD_CARGO))),
  controlFrontend: artifact("LIGHT_CONTROL_FRONTEND_DIR", "FRONTEND_CONTROL"),
  hardwareFrontend: artifact("LIGHT_HARDWARE_FRONTEND_DIR", "FRONTEND_HARDWARE"),
  manual: artifact("LIGHT_MANUAL_ROOT", "MANUAL_ROOT"),
  release: artifact("LIGHT_RELEASE_DIR", "RELEASE_ROOT"),
  runtime: absolute(configured(
    Object.hasOwn(process.env, "LIGHT_DATA_DIR") ? "LIGHT_DATA_DIR" : "LIGHT_RUNTIME_DATA_DIR",
    path.join(artifactRoot, layout.RUNTIME_DATA),
  )),
  coverage: artifact("LIGHT_TEST_COVERAGE_DIR", "TEST_COVERAGE"),
  report: artifact("LIGHT_PLAYWRIGHT_REPORT_DIR", "TEST_REPORT"),
  results: artifact("LIGHT_TEST_RESULTS_DIR", "TEST_RESULTS"),
  visual: artifact("LIGHT_VISUAL_INSPECTION_DIR", "TEST_VISUAL"),
  tmp: artifact("LIGHT_TMP_DIR", "TMP_ROOT"),
});

module.exports = { artifactPaths, artifactRoot, repositoryRoot };
