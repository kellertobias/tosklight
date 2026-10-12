import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const helper = path.join(repositoryRoot, "tools/stage-architect-resources.sh");
const quote = (value) => `'${value.replaceAll("'", "'\\''")}'`;
function fixture(t) {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const root = fs.mkdtempSync(
		path.join(artifactPaths.tmp, "architect-resources-"),
	);
	t.after(() => fs.rmSync(root, { recursive: true, force: true }));
	const demo = path.join(root, "generated/demo/demo-show.show");
	const library = path.join(root, "assets/fixture-library");
	const app = path.join(root, "bundle/ToskLight Architect.app");
	fs.mkdirSync(path.dirname(demo), { recursive: true });
	fs.writeFileSync(demo, Buffer.from([0, 1, 2, 255]));
	fs.mkdirSync(path.join(library, "nested"), { recursive: true });
	fs.writeFileSync(
		path.join(library, "lamp.toskfixture"),
		"exact fixture bytes",
	);
	fs.writeFileSync(
		path.join(library, "nested/model.glb"),
		Buffer.from([9, 0, 255]),
	);
	fs.mkdirSync(path.join(app, "Contents/Resources"), { recursive: true });
	fs.writeFileSync(path.join(app, "Contents/Resources/icon.icns"), "keep icon");
	return { root, demo, library, app };
}
const stage = ({ app, demo, library }) =>
	spawnSync("bash", [helper, app, demo, library], { encoding: "utf8" });
function exactResources(f) {
	assert.deepEqual(
		fs.readFileSync(
			path.join(f.app, "Contents/Resources/demo-show/demo-show.show"),
		),
		fs.readFileSync(f.demo),
	);
	for (const file of ["lamp.toskfixture", "nested/model.glb"])
		assert.deepEqual(
			fs.readFileSync(
				path.join(f.app, "Contents/Resources/fixture-library", file),
			),
			fs.readFileSync(path.join(f.library, file)),
		);
}

test("canonical local bundle stages exact resources before sealing", (t) => {
	const f = fixture(t);
	const source = fs.readFileSync(
		path.join(repositoryRoot, "tools/build.sh"),
		"utf8",
	);
	const start = source.indexOf("build_visualizer() {");
	const end = source.indexOf("\n# On macOS", start);
	assert.ok(start >= 0 && end > start);
	const tools = path.join(f.root, "tools");
	fs.mkdirSync(tools);
	for (const name of [
		"bundle-visualizer-macos.sh",
		"stage-architect-resources.sh",
	])
		if (fs.existsSync(path.join(repositoryRoot, "tools", name)))
			fs.symlinkSync(
				path.join(repositoryRoot, "tools", name),
				path.join(tools, name),
			);
	fs.writeFileSync(
		path.join(tools, "seal-macos-app.sh"),
		`#!/bin/bash
if [[ -f "$1/Contents/Resources/demo-show/demo-show.show" && -f "$1/Contents/Resources/fixture-library/lamp.toskfixture" ]]; then touch "$1/Contents/Resources/sealed-after-resources"; fi
exit 0
`,
	);
	const target = path.join(f.root, "build/cargo");
	fs.mkdirSync(path.join(target, "release"), { recursive: true });
	for (const name of ["viz-editor", "viz-renderer", "light-headless"])
		fs.writeFileSync(path.join(target, "release", name), name);
	fs.mkdirSync(path.join(f.root, "apps/viz-editor/src-tauri/icons"), {
		recursive: true,
	});
	fs.writeFileSync(
		path.join(f.root, "apps/viz-editor/src-tauri/icons/icon.icns"),
		"icon",
	);
	fs.mkdirSync(path.join(f.root, "build/patch-mcp"), { recursive: true });
	fs.writeFileSync(
		path.join(f.root, "build/patch-mcp/tosklight-patch-mcp.mjs"),
		"export {}; ",
	);
	const script = path.join(f.root, "run-build-boundary.sh");
	fs.writeFileSync(
		script,
		`set -euo pipefail
ROOT=${quote(f.root)}
TARGET_DIR=${quote(target)}
LIGHT_ARTIFACTS_DIR=${quote(f.root)}
LIGHT_DEMO_SHOW_DIR=${quote(path.dirname(f.demo))}
FIXTURE_LIBRARY_DIR=${quote(f.library)}
require() { :; }
cargo() { :; }
build_visualizer_headless() { :; }
build_viz_editor() { :; }
uname() { printf Darwin; }
export -f uname
${source.slice(start, end)}
build_visualizer
`,
	);
	const outcome = spawnSync("bash", [script], { encoding: "utf8" });
	assert.equal(outcome.status, 0, outcome.stderr);
	f.app = path.join(target, "release/bundle/macos/ToskLight Architect.app");
	exactResources(f);
	assert.ok(
		fs.existsSync(
			path.join(f.app, "Contents/Resources/sealed-after-resources"),
		),
	);
	assert.equal(
		fs.readFileSync(
			path.join(f.app, "Contents/Resources/tosklight-patch-mcp.mjs"),
			"utf8",
		),
		"export {}; ",
	);
});

test("resource staging preserves exact nested content and unrelated bundle resources", (t) => {
	const f = fixture(t);
	const outcome = stage(f);
	assert.equal(outcome.status, 0, outcome.stderr);
	exactResources(f);
	assert.equal(
		fs.readFileSync(path.join(f.app, "Contents/Resources/icon.icns"), "utf8"),
		"keep icon",
	);
});

for (const [name, remove, expected] of [
	["missing demo", (f) => fs.unlinkSync(f.demo), /demo show/],
	["empty demo", (f) => fs.writeFileSync(f.demo, ""), /demo show/],
	[
		"missing fixture directory",
		(f) => fs.rmSync(f.library, { recursive: true }),
		/fixture library/,
	],
	[
		"empty fixture library",
		(f) => {
			fs.rmSync(f.library, { recursive: true });
			fs.mkdirSync(f.library);
		},
		/fixture packages/,
	],
	[
		"missing bundle",
		(f) => fs.rmSync(f.app, { recursive: true }),
		/application resources/,
	],
])
	test(`resource staging rejects ${name} before modifying the bundle`, (t) => {
		const f = fixture(t);
		remove(f);
		const outcome = stage(f);
		assert.notEqual(outcome.status, 0);
		assert.match(outcome.stderr, expected);
		assert.equal(
			fs.existsSync(path.join(f.app, "Contents/Resources/demo-show")),
			false,
		);
	});

test("restaging removes obsolete fixture packages without replacing other resources", (t) => {
	const f = fixture(t);
	assert.equal(stage(f).status, 0);
	const obsolete = path.join(
		f.app,
		"Contents/Resources/fixture-library/obsolete.toskfixture",
	);
	fs.writeFileSync(obsolete, "stale package");
	fs.writeFileSync(f.demo, "new exact demo");
	const outcome = stage(f);
	assert.equal(outcome.status, 0, outcome.stderr);
	assert.equal(fs.existsSync(obsolete), false);
	exactResources(f);
	assert.equal(
		fs.readFileSync(path.join(f.app, "Contents/Resources/icon.icns"), "utf8"),
		"keep icon",
	);
});
