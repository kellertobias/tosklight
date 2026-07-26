#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  evaluateTestCommandBoundaries,
  readTestSources,
  scanTestCommandBoundaries,
} from "./test-command-boundaries.mjs";
import {
  readPrivateBoundarySources,
  scanPrivateTestBoundaries,
} from "./test-private-boundaries.mjs";
import {
  scanSemanticWorldFiles,
} from "./test-semantic-world-boundaries.mjs";
import {
  workspaceLintInheritanceFailures,
} from "./cargo-workspace-lints.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];

function fail(message) {
  failures.push(message);
}

function relative(file) {
  return path.relative(repositoryRoot, file).split(path.sep).join("/");
}

function rustDependencyDirections() {
  let metadata;
  try {
    metadata = JSON.parse(
      execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
        cwd: repositoryRoot,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
      }),
    );
  } catch (error) {
    const stderr = error?.stderr?.toString().trim() || error.message;
    fail(`cargo metadata failed: ${stderr}`);
    return;
  }

  const workspaceIds = new Set(metadata.workspace_members);
  const workspacePackages = metadata.packages.filter((candidate) => workspaceIds.has(candidate.id));
  const workspaceNames = new Set(workspacePackages.map((candidate) => candidate.name));

  for (const failure of workspaceLintInheritanceFailures(metadata)) fail(failure);

  for (const packageMetadata of workspacePackages) {
    const manifest = relative(packageMetadata.manifest_path);
    const workspaceDependencies = new Set(
      packageMetadata.dependencies
        .map((dependency) => dependency.name)
        .filter((name) => workspaceNames.has(name)),
    );
    const forbidden = new Set();

    if (packageMetadata.name === "light-wire") {
      for (const dependency of workspaceDependencies) forbidden.add(dependency);
    } else if (packageMetadata.name === "light-application") {
      for (const dependency of [
        "light-wire",
        "light-headless-runtime",
        "light-headless",
        "light-desktop",
        "light-hardware-controls",
      ])
        if (workspaceDependencies.has(dependency)) forbidden.add(dependency);
    } else if (
      (manifest.startsWith("crates/light/domain/") || manifest.startsWith("crates/shared/")) &&
      packageMetadata.name !== "light-headless-runtime"
    ) {
      for (const dependency of [
        "light-application",
        "light-wire",
        "light-headless-runtime",
        "light-headless",
      ])
        if (workspaceDependencies.has(dependency)) forbidden.add(dependency);
    }

    if (forbidden.size > 0) {
      fail(`${packageMetadata.name} has forbidden workspace dependencies: ${[...forbidden].sort().join(", ")}`);
    }
  }

  const runtime = workspacePackages.find((candidate) => candidate.name === "light-headless-runtime");
  if (!runtime) {
    fail("light-headless-runtime is missing from the Rust workspace");
  } else {
    const dependencies = new Set(runtime.dependencies.map((dependency) => dependency.name));
    for (const required of ["light-application", "light-wire"])
      if (!dependencies.has(required)) fail(`light-headless-runtime must compose ${required}`);
  }

  const headless = workspacePackages.find((candidate) => candidate.name === "light-headless");
  if (!headless) {
    fail("light-headless is missing from the Rust workspace");
  } else {
    const dependencies = new Set(headless.dependencies.map((dependency) => dependency.name));
    if (!dependencies.has("light-headless-runtime"))
      fail("light-headless must bootstrap light-headless-runtime");
  }
}

function serverEntrypointIsThin() {
  const entrypoint = path.join(repositoryRoot, "apps/light-headless/src/main.rs");
  const source = fs.readFileSync(entrypoint, "utf8");
  const nonEmptyLines = source.split(/\r?\n/u).filter((line) => line.trim()).length;
  if (nonEmptyLines > 10) fail("apps/light-headless/src/main.rs must remain a thin lifecycle entry point");
  for (const forbidden of ["Router", "AppState", "TcpListener", "tokio::spawn"])
    if (source.includes(forbidden)) fail(`server entry point must not own ${forbidden}`);
  if (!source.includes("light_headless_runtime::run().await"))
    fail("headless entry point must delegate lifecycle ownership to the runtime adapter");
}

function desktopHostIsCompositionRoot() {
  const entrypoint = path.join(
    repositoryRoot,
    "apps/light-desktop/src-tauri/src/main.rs",
  );
  const source = fs.readFileSync(entrypoint, "utf8");
  const nonEmptyLines = source.split(/\r?\n/u).filter((line) => line.trim()).length;
  if (nonEmptyLines > 55)
    fail("apps/light-desktop/src-tauri/src/main.rs must remain a thin composition root");
  for (const forbidden of [
    "AtomicBool",
    "Command::new",
    "MenuItemBuilder",
    "TcpStream",
    "WebviewWindowBuilder",
    "thread::spawn",
  ])
    if (source.includes(forbidden))
      fail(`desktop composition root must not own ${forbidden}`);
  for (const required of [
    "lifecycle::setup(app)",
    "menu::install(app)",
    "server::setup(app)",
    "lifecycle::handle_run_event",
  ])
    if (!source.includes(required))
      fail(`desktop composition root must delegate through ${required}`);
}

function activeShowMutationDirections() {
  const updateAdapter = path.join(repositoryRoot, "crates/light/adapters/headless/src/runtime/update_plans.rs");
  const source = fs.readFileSync(updateAdapter, "utf8");
  for (const forbidden of [".put_object(", "refresh_command_show", "load_engine_snapshot"])
    if (source.includes(forbidden))
      fail(`Update must route active-show writes through ActiveShowService, not ${forbidden}`);

  const adapterRoot = path.join(repositoryRoot, "crates/light/adapters/headless/src");
  const directMutation = /\.(?:put_object|delete_object|mutate_objects_atomically|apply_portable_transaction|put_user_layout)\s*\(/u;
  const deliberateBoundaries = new Set([
    // The ActiveShowService unit of work is the one production active-show commit owner.
    "crates/light/adapters/headless/src/runtime/active_show_adapter.rs",
    // Default-show creation writes a new library file, never the loaded active show.
    "crates/light/adapters/headless/src/default_show/seed.rs",
    // MVR apply is a deliberately separate whole-import boundary.
    "crates/light/adapters/headless/src/runtime/mvr_apply_store.rs",
    // Show loading commits compatibility migrations before installing the loaded show.
    "crates/light/adapters/headless/src/runtime/show_compile.rs",
    // The generic object endpoint retains inactive-library and isolated test-seed writes.
    "crates/light/adapters/headless/src/runtime/object_api.rs",
    // Preload writes directly only when its target show is not active.
    "crates/light/adapters/headless/src/runtime/store_api.rs",
  ]);
  for (const file of walk(adapterRoot).filter((candidate) => candidate.endsWith(".rs"))) {
    const name = relative(file);
    if (
      name.includes("/tests/") ||
      name.endsWith("/tests.rs") ||
      name.endsWith("_tests.rs") ||
      deliberateBoundaries.has(name)
    )
      continue;
    if (directMutation.test(fs.readFileSync(file, "utf8")))
      fail(`${name} writes ShowStore directly outside the ActiveShowService or a deliberate library/import boundary`);
  }
}

function playbackOwnershipBoundaries() {
  const engineSources = walk(path.join(repositoryRoot, "crates/light/domain/engine/src"))
    .filter((candidate) => candidate.endsWith(".rs"));
  for (const file of engineSources) {
    const source = fs.readFileSync(file, "utf8");
    if (/\bpub\s+fn\s+playback\s*\(/u.test(source))
      fail(`${relative(file)} exposes a public Playback lock instead of typed commands and projections`);
  }

  const applicationPlayback = walk(path.join(repositoryRoot, "crates/light/src/playback"))
    .filter((candidate) => candidate.endsWith(".rs"));
  for (const file of applicationPlayback) {
    const source = fs.readFileSync(file, "utf8");
    if (/\bpub\s+fn\s+operation_lock\s*\(/u.test(source))
      fail(`${relative(file)} returns PlaybackService ordering ownership to an adapter`);
  }

  const externalRoots = [
    path.join(repositoryRoot, "crates/light/src"),
    path.join(repositoryRoot, "crates/light/adapters/headless/src"),
  ];
  for (const file of externalRoots.flatMap(walk).filter((candidate) => candidate.endsWith(".rs"))) {
    const name = relative(file);
    if (name.includes("/tests/") || name.endsWith("_tests.rs")) continue;
    const source = fs.readFileSync(file, "utf8");
    if (/\bengine\s*\.\s*playback\s*\(\s*\)/u.test(source))
      fail(`${name} bypasses the typed Engine Playback boundary`);
    if (source.includes("playback_action_lock"))
      fail(`${name} duplicates ordering which belongs to PlaybackService`);
  }
}

function walk(directory) {
  if (!fs.existsSync(directory)) return [];
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(target) : [target];
  });
}

function localImports(source) {
  const imports = [];
  const expression = /(?:import|export)\s+(?:type\s+)?(?:[^"']*?\s+from\s+)?["']([^"']+)["']/gu;
  for (const match of source.matchAll(expression)) imports.push(match[1]);
  return imports.filter((specifier) => specifier.startsWith("."));
}

function typeScriptDependencyDirections() {
  const sourceRoot = path.join(repositoryRoot, "apps/light-desktop/src");
  const apiRoot = path.join(sourceRoot, "api");
  const generatedFile = path.join(apiRoot, "generated/light-wire.ts");
  if (!fs.existsSync(generatedFile)) {
    fail(`${relative(generatedFile)} is missing; regenerate the Rust wire contracts`);
    return;
  }

  const generatedSource = fs.readFileSync(generatedFile, "utf8");
  if (!generatedSource.startsWith("// This file is generated"))
    fail(`${relative(generatedFile)} must remain a generated artifact`);
  if (localImports(generatedSource).length > 0)
    fail(`${relative(generatedFile)} must be a self-contained transport contract`);

  let generatedConsumers = 0;
  for (const file of walk(sourceRoot).filter((candidate) => /\.[cm]?tsx?$/u.test(candidate))) {
    const source = fs.readFileSync(file, "utf8");
    for (const specifier of localImports(source)) {
      const resolved = path.resolve(path.dirname(file), specifier);
      const importsGenerated = resolved === generatedFile.slice(0, -3) || resolved === generatedFile;
      if (importsGenerated) {
        generatedConsumers += 1;
        if (!file.startsWith(`${apiRoot}${path.sep}`))
          fail(`${relative(file)} imports wire DTOs directly; map them at the API boundary`);
      }

      const importsUiFromApi =
        file.startsWith(`${apiRoot}${path.sep}`) &&
        (resolved.startsWith(path.join(sourceRoot, "components")) ||
          resolved.startsWith(path.join(sourceRoot, "windows")));
      const legacySoftwareKeypad =
        relative(file) === "apps/light-desktop/src/api/ServerContext.tsx" &&
        specifier === "../components/control/softwareKeypad";
      if (importsUiFromApi && !legacySoftwareKeypad)
        fail(`${relative(file)} imports presentation code through ${specifier}`);
    }
  }
  if (generatedConsumers === 0)
    fail("generated wire DTOs must be consumed and validated by the frontend API boundary");
}

const legacyPlaybackPatterns = [
  ["server.playbacks", /\bserver\s*\.\s*playbacks\b/u],
  ["state.playbacks", /\bstate\s*\.\s*playbacks\b/u],
  ["setPlaybacks", /\bsetPlaybacks\b/u],
  ["client.playbacks()", /\bclient\s*\.\s*playbacks\s*\(/u],
  ["a retired /api/v1 endpoint", /["'`]\/api\/v1(?:[/?"'`])/u],
  ["the legacy useGroups helper", /\buseGroups\b/u],
];

function isProductionTypeScript(file) {
  const name = relative(file);
  return (
    /\.[cm]?tsx?$/u.test(file) &&
    !name.includes("/__tests__/") &&
    !/\.(?:test|spec)\.[cm]?tsx?$/u.test(name)
  );
}

function legacyPlaybackSnapshotBoundaries() {
  const sourceRoot = path.join(repositoryRoot, "apps/light-desktop/src");
  for (const file of walk(sourceRoot).filter(isProductionTypeScript)) {
    const source = fs.readFileSync(file, "utf8");
    for (const [description, pattern] of legacyPlaybackPatterns)
      if (pattern.test(source))
        fail(`${relative(file)} reintroduces ${description}; use scoped Playback authority`);
  }
}

function testCommandBoundaries() {
  const baselinePath = path.join(repositoryRoot, "tools/test-command-boundaries.baseline.json");
  if (!fs.existsSync(baselinePath)) {
    fail(`${relative(baselinePath)} is missing; regenerate the test command boundary baseline`);
    return;
  }
  const baseline = JSON.parse(fs.readFileSync(baselinePath, "utf8"));
  const scan = scanTestCommandBoundaries(readTestSources(repositoryRoot));
  for (const failure of evaluateTestCommandBoundaries(scan, baseline)) fail(failure);
}

function privateTestBoundaries() {
  for (const failure of scanPrivateTestBoundaries(
    readPrivateBoundarySources(repositoryRoot),
  )) fail(failure);
}

function semanticWorldBoundaries() {
  for (const failure of scanSemanticWorldFiles(repositoryRoot)) fail(failure);
  try {
    execFileSync("node", [
      path.join(repositoryRoot, "tools/test-bench-migration-inventory.mjs"),
      "--check",
    ], { cwd: repositoryRoot, stdio: ["ignore", "pipe", "pipe"] });
  } catch (error) {
    fail(error?.stderr?.toString().trim() || error.message);
  }
  try {
    execFileSync("node", [
      path.join(repositoryRoot, "tools/semantic-test-docs/cli.mjs"),
      "--check",
    ], { cwd: repositoryRoot, stdio: ["ignore", "pipe", "pipe"] });
  } catch (error) {
    fail(error?.stderr?.toString().trim() || error.message);
  }
}

rustDependencyDirections();
serverEntrypointIsThin();
desktopHostIsCompositionRoot();
activeShowMutationDirections();
playbackOwnershipBoundaries();
typeScriptDependencyDirections();
legacyPlaybackSnapshotBoundaries();
testCommandBoundaries();
privateTestBoundaries();
semanticWorldBoundaries();

if (failures.length > 0) {
  for (const failure of failures) console.error(`architecture error: ${failure}`);
  process.exitCode = 1;
} else {
  console.log("Rust and TypeScript dependency directions are valid.");
}
