#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import postcss from "postcss";
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
import {
  controlStateLabelWarnings,
} from "./check-control-state-labels.mjs";
import {
  capabilityStateBoundaryFailures,
  readCapabilityStateBoundarySources,
} from "./capability-state-boundaries.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const failures = [];

function fail(message) {
  failures.push(message);
}

function relative(file) {
  return path.relative(repositoryRoot, file).split(path.sep).join("/");
}

function withoutInlineRustTests(source) {
  const inlineTests =
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/u.exec(source);
  return inlineTests ? source.slice(0, inlineTests.index) : source;
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

  mediaDependencyDirections(workspacePackages);

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
    // Capability-owned SQLite adapter. It exposes persistence operations but no raw ShowStore;
    // the calling ActiveShowResource/service still owns loaded-show mutation ordering.
    "crates/light/adapters/headless/src/runtime/capabilities/active_show/repository.rs",
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
    if (directMutation.test(withoutInlineRustTests(fs.readFileSync(file, "utf8"))))
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

function importSpecifiers(source) {
  const imports = [];
  const expression = /(?:import|export)\s+(?:type\s+)?(?:[^"']*?\s+from\s+)?["']([^"']+)["']/gu;
  for (const match of source.matchAll(expression)) imports.push(match[1]);
  const dynamicExpression = /\b(?:import|require)\s*\(\s*["']([^"']+)["']\s*\)/gu;
  for (const match of source.matchAll(dynamicExpression)) imports.push(match[1]);
  return imports;
}

function localImports(source) {
  return importSpecifiers(source).filter((specifier) => specifier.startsWith("."));
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

function sharedUiDependencyDirections() {
  const packageRoot = path.join(repositoryRoot, "apps/ui-library");
  const sourceRoot = path.join(packageRoot, "src");
  const appsRoot = path.join(repositoryRoot, "apps");
  const desktopSourceRoot = path.join(repositoryRoot, "apps/light-desktop/src");
  const manifestPath = path.join(packageRoot, "package.json");
  if (!fs.existsSync(manifestPath) || !fs.existsSync(sourceRoot)) {
    fail("apps/ui-library must contain tracked source and a package manifest");
    return;
  }

  const packageCssRules = new Set();
  const cssRuleContext = (rule) => {
    const ancestors = [];
    for (let parent = rule.parent; parent && parent.type !== "root"; parent = parent.parent)
      if (parent.type === "atrule")
        ancestors.unshift(`@${parent.name} ${parent.params}`);
    return ancestors.join("|");
  };
  const cssRuleSignature = (rule) => {
    const declarations = rule.nodes
      .filter((node) => node.type === "decl")
      .map((node) => `${node.prop}:${node.value}${node.important ? "!important" : ""}`)
      .sort()
      .join(";");
    return declarations
      ? `${cssRuleContext(rule)}\0${rule.selector}\0${declarations}`
      : null;
  };
  for (const file of walk(sourceRoot).filter((candidate) => candidate.endsWith(".css"))) {
    const stylesheet = postcss.parse(fs.readFileSync(file, "utf8"), { from: file });
    stylesheet.walkRules((rule) => {
      const signature = cssRuleSignature(rule);
      if (signature) packageCssRules.add(signature);
    });
  }
  for (const file of walk(desktopSourceRoot).filter((candidate) => candidate.endsWith(".css"))) {
    const stylesheet = postcss.parse(fs.readFileSync(file, "utf8"), { from: file });
    stylesheet.walkRules((rule) => {
      const signature = cssRuleSignature(rule);
      if (signature && packageCssRules.has(signature))
        fail(`${relative(file)} duplicates the package-owned ${rule.selector} rule verbatim`);
    });
  }

  const retiredDesktopCompatibilityModules = [
    "components/common/FaderControls.tsx",
    "components/common/ModalPortal.tsx",
    "components/common/ModalTitleBar.tsx",
    "components/common/SearchBar.tsx",
    "components/common/TouchSelect.tsx",
    "components/common/controls.tsx",
    "components/common/controls/InputModal.tsx",
    "components/common/controls/choices.tsx",
    "components/common/controls/formFields.tsx",
    "components/common/controls/foundation.tsx",
    "components/common/controls/pickers.tsx",
    "components/common/controls/textInputs.tsx",
    "components/common/index.ts",
    "components/control/HorizontalTouchFader.tsx",
    "components/control/TouchEncoder.tsx",
    "components/input/ModalEscapeManager.tsx",
    "components/input/ModalInputControls.tsx",
    "components/window-kit/SelectionList.tsx",
    "components/window-kit/UiKitCatalog.tsx",
    "components/window-kit/WindowKit.tsx",
    "components/window-kit/index.ts",
    "windows/FixtureSheetTable.tsx",
  ];
  for (const retired of retiredDesktopCompatibilityModules) {
    const file = path.join(desktopSourceRoot, retired);
    if (fs.existsSync(file))
      fail(`${relative(file)} is retired migration scaffolding; import @tosklight/ui directly`);
  }

  for (const registeredOverlay of [
    "components/modals/DeskLockOverlay.tsx",
    "components/modals/QuitConfirmOverlay.tsx",
    "components/modals/ShowRecoveryModal.tsx",
    "components/modals/WindowPicker.tsx",
    "windows/FileManagerPickerHost.tsx",
  ]) {
    const file = path.join(desktopSourceRoot, registeredOverlay);
    if (!fs.readFileSync(file, "utf8").includes("<ModalRegistration"))
      fail(`${relative(file)} must participate in the shared modal stack`);
  }

  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  for (const reactPackage of ["react", "react-dom"]) {
    if (!manifest.peerDependencies?.[reactPackage])
      fail(`apps/ui-library must declare ${reactPackage} as a peer dependency`);
    if (manifest.dependencies?.[reactPackage])
      fail(`apps/ui-library must not install a second ${reactPackage} runtime`);
  }

  const forbiddenSourcePatterns = [
    ["application context", /\b(?:useApp|useServer)\b/u],
    ["WindowRegistry", /\bWindowRegistry\b/u],
    ["Tauri integration", /@tauri-apps|src-tauri/u],
    ["server fetch", /\bfetch\s*\(/u],
    ["WebSocket integration", /\b(?:new\s+)?WebSocket\s*\(/u],
    ["server API path", /["'`]\/api(?:\/|["'`])/u],
  ];
  for (const file of walk(sourceRoot).filter((candidate) => /\.[cm]?tsx?$/u.test(candidate))) {
    const source = fs.readFileSync(file, "utf8");
    for (const specifier of localImports(source)) {
      const resolved = path.resolve(path.dirname(file), specifier);
      if (resolved.startsWith(appsRoot) && !resolved.startsWith(packageRoot))
        fail(`${relative(file)} imports application source through ${specifier}`);
    }
    if (/(?:from|import)\s*["'][^"']*light-desktop/u.test(source))
      fail(`${relative(file)} imports the desktop application; keep that dependency one-way`);
    for (const [description, pattern] of forbiddenSourcePatterns) {
      if (pattern.test(source))
        fail(`${relative(file)} imports or embeds ${description}; keep it in an application adapter`);
    }
  }

  for (const file of walk(desktopSourceRoot).filter((candidate) => /\.[cm]?tsx?$/u.test(candidate))) {
    const name = relative(file);
    const source = fs.readFileSync(file, "utf8");
    const production = !/\.(?:stories|test|spec)\.[cm]?tsx?$/u.test(name);
    if (!production) continue;
    for (const specifier of localImports(source)) {
      const resolved = path.resolve(path.dirname(file), specifier);
      if (resolved.startsWith(packageRoot))
        fail(`${name} imports the UI library by relative path; use @tosklight/ui`);
      if (
        resolved.includes(`${path.sep}storybook${path.sep}`) ||
        /\.stories$/u.test(resolved)
      )
        fail(`${name} imports Storybook-only source through ${specifier}`);
    }
    if (/from\s*["'][^"']*(?:storybook|\.stories)["']/u.test(source))
      fail(`${name} imports Storybook-only source`);
    if (name === "apps/light-desktop/src/main.tsx" && /ui-kit|UiKitCatalog/u.test(source))
      fail(`${name} must use the contained Storybook instead of the retired UI Kit route`);
    if (/search\s*=\s*\{\s*<SearchBar\b/u.test(source))
      fail(`${name} renders SearchBar through arbitrary window/modal chrome; use typed search props`);

    const rawModalLayers = source.match(
      /className=(?:"[^"]*(?:modal-backdrop|stacked-modal-layer)[^"]*"|\{[^}\n]*(?:modal-backdrop|stacked-modal-layer)[^}\n]*\})/gu,
    )?.length ?? 0;
    const registeredModalLayers =
      (source.match(/<ModalRegistration\b/gu)?.length ?? 0) +
      (source.match(/<ModalPortal\b[^>]*\bonClose=/gu)?.length ?? 0);
    if (rawModalLayers > 0 && rawModalLayers !== registeredModalLayers)
      fail(`${name} has ${rawModalLayers} application modal layers but ${registeredModalLayers} shared-stack registrations`);
  }

  for (const file of walk(sourceRoot).filter((candidate) => /\.[cm]?tsx?$/u.test(candidate))) {
    const source = fs.readFileSync(file, "utf8");
    if (/search\s*=\s*\{\s*<SearchBar\b/u.test(source))
      fail(`${relative(file)} renders SearchBar through arbitrary window/modal chrome; use typed search props`);
  }
}

function applicationSourceDirections() {
  const applications = [
    {
      packageName: "@tosklight/light-desktop",
      packageRoot: path.join(repositoryRoot, "apps/light-desktop"),
    },
    {
      packageName: "@tosklight/light-hardware-controls",
      packageRoot: path.join(repositoryRoot, "apps/light-hardware-controls"),
    },
  ];
  for (const application of applications) {
    const sourceRoot = path.join(application.packageRoot, "src");
    const otherApplications = applications.filter((candidate) => candidate !== application);
    const manifest = JSON.parse(
      fs.readFileSync(path.join(application.packageRoot, "package.json"), "utf8"),
    );
    const dependencies = {
      ...manifest.dependencies,
      ...manifest.devDependencies,
      ...manifest.peerDependencies,
    };
    for (const other of otherApplications)
      if (dependencies[other.packageName])
        fail(`${application.packageName} must not depend on ${other.packageName}`);
    for (const file of walk(sourceRoot).filter((candidate) => /\.[cm]?tsx?$/u.test(candidate))) {
      const source = fs.readFileSync(file, "utf8");
      for (const specifier of importSpecifiers(source)) {
        for (const other of otherApplications)
          if (
            specifier === other.packageName ||
            specifier.startsWith(`${other.packageName}/`)
          )
            fail(
              `${relative(file)} imports ${other.packageName}; move proven shared contracts to ` +
                "@tosklight/ui and keep application adapters private",
            );
        if (!specifier.startsWith(".")) continue;
        const resolved = path.resolve(path.dirname(file), specifier);
        const importedApplication = otherApplications.find(
          ({ packageRoot }) =>
            resolved === packageRoot || resolved.startsWith(`${packageRoot}${path.sep}`),
        );
        if (importedApplication)
          fail(
            `${relative(file)} imports ${relative(importedApplication.packageRoot)} through ${specifier}; ` +
              "move proven shared contracts to @tosklight/ui and keep application adapters private",
          );
      }
    }
  }
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

function desktopStylesheetEntrypoints() {
  const fileManagerStylesheet = path.join(
    repositoryRoot,
    "apps/light-desktop/src/windows/FileManagerWindow.css",
  );
  const source = fs.readFileSync(fileManagerStylesheet, "utf8");
  const orderedImports = [
    '@import "./fileManager/chrome.css";',
    '@import "./fileManager/browser.css";',
    '@import "./fileManager/operations.css";',
  ];
  let cursor = -1;
  for (const stylesheetImport of orderedImports) {
    const next = source.indexOf(stylesheetImport);
    if (next < 0) {
      fail(`${relative(fileManagerStylesheet)} must import ${stylesheetImport}`);
      continue;
    }
    if (next < cursor)
      fail(`${relative(fileManagerStylesheet)} must preserve its stylesheet cascade order`);
    cursor = next;
  }
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
}

function capabilityStateOwnershipBoundaries() {
  for (const failure of capabilityStateBoundaryFailures(
    readCapabilityStateBoundarySources(repositoryRoot),
  )) fail(failure);
}

// The Media Server is a second product in this workspace. Its layering is the same shape as the
// desk's — domain, application, adapters, composition root — and Light and Media meet only in
// `crates/shared/*`. These checks keep that true as the rebuild fills the crates in.
const MEDIA_ALLOWED_WORKSPACE_DEPENDENCIES = new Map([
  ["media-domain", new Set()],
  ["media-application", new Set(["media-domain"])],
  ["media-codec", new Set(["media-domain"])],
  ["media-http", new Set(["media-domain", "media-application"])],
  ["media-library", new Set(["media-domain", "media-codec"])],
  ["media-net", new Set(["media-domain"])],
  ["media-playback", new Set(["media-domain", "media-codec", "media-library", "media-render"])],
  ["media-render", new Set(["media-domain", "media-codec"])],
  ["media-runtime", new Set(["media-application", "media-codec", "media-domain", "media-net", "media-playback", "media-render"])],
  ["media-server", new Set(["media-runtime"])],
]);

function mediaDependencyDirections(workspacePackages) {
  const sharedNames = new Set(
    workspacePackages
      .filter((candidate) => relative(candidate.manifest_path).startsWith("crates/shared/"))
      .map((candidate) => candidate.name),
  );

  for (const packageMetadata of workspacePackages) {
    const dependencies = packageMetadata.dependencies
      .map((dependency) => dependency.name)
      .filter((name) => workspacePackages.some((candidate) => candidate.name === name));
    const isMedia = MEDIA_ALLOWED_WORKSPACE_DEPENDENCIES.has(packageMetadata.name);

    if (isMedia) {
      const allowed = MEDIA_ALLOWED_WORKSPACE_DEPENDENCIES.get(packageMetadata.name);
      const forbidden = dependencies.filter(
        (name) => !allowed.has(name) && !sharedNames.has(name),
      );
      if (forbidden.length > 0) {
        fail(
          `${packageMetadata.name} may depend only on ${[...allowed].sort().join(", ") || "the shared kernel"}` +
            `, not ${[...new Set(forbidden)].sort().join(", ")}`,
        );
      }
      continue;
    }

    // Nothing outside Media may reach into it. Light's CITP client stays Light's.
    const reachesIntoMedia = dependencies.filter((name) =>
      MEDIA_ALLOWED_WORKSPACE_DEPENDENCIES.has(name),
    );
    if (reachesIntoMedia.length > 0) {
      fail(
        `${packageMetadata.name} depends on Media packages ${reachesIntoMedia.sort().join(", ")}; ` +
          "Light and Media meet only through crates/shared",
      );
    }
  }

  for (const name of MEDIA_ALLOWED_WORKSPACE_DEPENDENCIES.keys()) {
    if (!workspacePackages.some((candidate) => candidate.name === name))
      fail(`${name} is missing from the Rust workspace`);
  }
}

function mediaDomainIsPure() {
  const domainRoot = path.join(repositoryRoot, "crates/media/domain/src");
  if (!fs.existsSync(domainRoot)) {
    fail("crates/media/domain/src is missing");
    return;
  }
  // Protocol, HTTP, filesystem, decoder, GPU, and operating-system types never enter domain
  // state; adapters translate them at the boundary.
  const forbidden = [
    ["std::net", "network types"],
    ["std::fs", "filesystem types"],
    ["std::path", "filesystem paths"],
    ["std::process", "process control"],
    ["std::env", "the process environment"],
    ["std::thread", "threads"],
  ];
  for (const file of walk(domainRoot).filter((candidate) => candidate.endsWith(".rs"))) {
    const source = withoutInlineRustTests(fs.readFileSync(file, "utf8"));
    for (const [needle, description] of forbidden)
      if (source.includes(needle))
        fail(`${relative(file)} brings ${description} into the Media domain`);
  }
}

function mediaEntrypointIsThin() {
  const entrypoint = path.join(repositoryRoot, "apps/media/src/main.rs");
  if (!fs.existsSync(entrypoint)) {
    fail("apps/media/src/main.rs is missing");
    return;
  }
  const source = fs.readFileSync(entrypoint, "utf8");
  const nonEmptyLines = source.split(/\r?\n/u).filter((line) => line.trim()).length;
  if (nonEmptyLines > 10) fail("apps/media/src/main.rs must remain a thin lifecycle entry point");
  for (const forbidden of ["Router", "TcpListener", "tokio::spawn", "EnvFilter"])
    if (source.includes(forbidden)) fail(`Media entry point must not own ${forbidden}`);
  if (!source.includes("media_runtime::run()"))
    fail("Media entry point must delegate lifecycle ownership to the runtime adapter");
}

/// The migration into this repository is one-way. Nothing here may read the C++ application's
/// checkout: not a path dependency, not a build step, not a runtime lookup.
function noLegacyMediaCheckoutReferences() {
  const legacyCheckout = ["/Users/keller/repos/", "media"].join("");
  const roots = ["apps/media", "crates/media", "tools"].map((candidate) =>
    path.join(repositoryRoot, candidate),
  );
  const scanned = /\.(?:mjs|rs|sh|toml|ts|tsx)$/u;
  for (const root of roots) {
    if (!fs.existsSync(root)) continue;
    for (const file of walk(root).filter((candidate) => scanned.test(candidate))) {
      if (path.resolve(file) === path.resolve(import.meta.filename)) continue;
      if (fs.readFileSync(file, "utf8").includes(legacyCheckout))
        fail(`${relative(file)} references the legacy Media checkout at ${legacyCheckout}`);
    }
  }
}

rustDependencyDirections();
mediaDomainIsPure();
mediaEntrypointIsThin();
noLegacyMediaCheckoutReferences();
serverEntrypointIsThin();
desktopHostIsCompositionRoot();
activeShowMutationDirections();
playbackOwnershipBoundaries();
capabilityStateOwnershipBoundaries();
typeScriptDependencyDirections();
sharedUiDependencyDirections();
desktopStylesheetEntrypoints();
applicationSourceDirections();
legacyPlaybackSnapshotBoundaries();
testCommandBoundaries();
privateTestBoundaries();
semanticWorldBoundaries();

for (const warning of controlStateLabelWarnings(repositoryRoot)) {
  console.warn(`architecture warning: ${warning}`);
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`architecture error: ${failure}`);
  process.exitCode = 1;
} else {
  console.log("Rust and TypeScript dependency directions are valid.");
}
