#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
      for (const dependency of ["light-wire", "light-server", "light-control-ui", "light-hardware-controls"])
        if (workspaceDependencies.has(dependency)) forbidden.add(dependency);
    } else if (
      manifest.startsWith("crates/") &&
      packageMetadata.name !== "light-server"
    ) {
      for (const dependency of ["light-application", "light-wire", "light-server"])
        if (workspaceDependencies.has(dependency)) forbidden.add(dependency);
    }

    if (forbidden.size > 0) {
      fail(`${packageMetadata.name} has forbidden workspace dependencies: ${[...forbidden].sort().join(", ")}`);
    }
  }

  const server = workspacePackages.find((candidate) => candidate.name === "light-server");
  if (!server) {
    fail("light-server is missing from the Rust workspace");
  } else {
    const dependencies = new Set(server.dependencies.map((dependency) => dependency.name));
    for (const required of ["light-application", "light-wire"])
      if (!dependencies.has(required)) fail(`light-server must compose ${required}`);
  }
}

function serverEntrypointIsThin() {
  const entrypoint = path.join(repositoryRoot, "crates/server/src/main.rs");
  const source = fs.readFileSync(entrypoint, "utf8");
  const nonEmptyLines = source.split(/\r?\n/u).filter((line) => line.trim()).length;
  if (nonEmptyLines > 10) fail("crates/server/src/main.rs must remain a thin lifecycle entry point");
  for (const forbidden of ["Router", "AppState", "TcpListener", "tokio::spawn"])
    if (source.includes(forbidden)) fail(`server entry point must not own ${forbidden}`);
  if (!source.includes("light_server::run().await"))
    fail("server entry point must delegate lifecycle ownership to the server library");
}

function activeShowMutationDirections() {
  const updateAdapter = path.join(repositoryRoot, "crates/server/src/runtime/update_plans.rs");
  const source = fs.readFileSync(updateAdapter, "utf8");
  for (const forbidden of [".put_object(", "refresh_command_show", "load_engine_snapshot"])
    if (source.includes(forbidden))
      fail(`Update must route active-show writes through ActiveShowService, not ${forbidden}`);
}

function playbackOwnershipBoundaries() {
  const engineSources = walk(path.join(repositoryRoot, "crates/engine/src"))
    .filter((candidate) => candidate.endsWith(".rs"));
  for (const file of engineSources) {
    const source = fs.readFileSync(file, "utf8");
    if (/\bpub\s+fn\s+playback\s*\(/u.test(source))
      fail(`${relative(file)} exposes a public Playback lock instead of typed commands and projections`);
  }

  const applicationPlayback = walk(path.join(repositoryRoot, "crates/application/src/playback"))
    .filter((candidate) => candidate.endsWith(".rs"));
  for (const file of applicationPlayback) {
    const source = fs.readFileSync(file, "utf8");
    if (/\bpub\s+fn\s+operation_lock\s*\(/u.test(source))
      fail(`${relative(file)} returns PlaybackService ordering ownership to an adapter`);
  }

  const externalRoots = [
    path.join(repositoryRoot, "crates/application/src"),
    path.join(repositoryRoot, "crates/server/src"),
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
  const sourceRoot = path.join(repositoryRoot, "apps/control-ui/src");
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
        relative(file) === "apps/control-ui/src/api/ServerContext.tsx" &&
        specifier === "../components/control/softwareKeypad";
      if (importsUiFromApi && !legacySoftwareKeypad)
        fail(`${relative(file)} imports presentation code through ${specifier}`);
    }
  }
  if (generatedConsumers === 0)
    fail("generated wire DTOs must be consumed and validated by the frontend API boundary");
}

rustDependencyDirections();
serverEntrypointIsThin();
activeShowMutationDirections();
playbackOwnershipBoundaries();
typeScriptDependencyDirections();

if (failures.length > 0) {
  for (const failure of failures) console.error(`architecture error: ${failure}`);
  process.exitCode = 1;
} else {
  console.log("Rust and TypeScript dependency directions are valid.");
}
