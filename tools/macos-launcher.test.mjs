import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { after, before, describe, test } from "node:test";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

describe("macOS suite launcher", { skip: process.platform !== "darwin" }, () => {
	let root;
	let app;
	let executable;
	before(() => {
		fs.mkdirSync(artifactPaths.tmp, { recursive: true });
		root = fs.mkdtempSync(path.join(artifactPaths.tmp, "launcher-tests-"));
		const built = spawnSync("bash", [path.join(repositoryRoot, "tools/bundle-launcher-macos.sh"), root, "1.8.0"], {
			encoding: "utf8", timeout: 120_000,
		});
		assert.equal(built.status, 0, built.stderr || built.stdout);
		app = path.join(root, "ToskLight Launcher.app");
		executable = path.join(app, "Contents/MacOS/ToskLight Launcher");
	});
	after(() => { if (root) fs.rmSync(root, { recursive: true, force: true }); });
	function fixture(folder, name, identifier) {
		const bundle = path.join(folder, name);
		fs.mkdirSync(path.join(bundle, "Contents"), { recursive: true });
		fs.writeFileSync(path.join(bundle, "Contents/Info.plist"),
			`<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>${identifier}</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>`);
		return bundle;
	}
	function verify(directory, binary = executable) {
		const env = { ...process.env };
		delete env.TOSKLIGHT_LAUNCHER_APPLICATIONS_DIR;
		if (directory) env.TOSKLIGHT_LAUNCHER_APPLICATIONS_DIR = directory;
		const result = spawnSync(binary, ["--verify"], { encoding: "utf8", env, timeout: 10_000 });
		assert.equal(result.status, 0, result.stderr);
		return JSON.parse(result.stdout);
	}
	test("resolves renamed download, Homebrew and source apps by bundle identity", () => {
		const folder = path.join(root, "Renamed suite");
		const expected = {
			control: fixture(folder, "tosklight-desk-macos_arm64.app", "de.tokenet.light"),
			architect: fixture(folder, "Venue Planner.app", "de.tokenet.tosklight.visualizer"),
			pixel: fixture(folder, "ToskLight Pixel.app", "de.tokenet.tosklight.media"),
		};
		fixture(folder, "ToskLight Control.app", "com.example.unrelated");
		assert.deepEqual(Object.fromEntries(verify(folder).map(({ id, application }) => [id, application])), expected);
	});
	test("an explicit suite directory never substitutes applications from the user's installation", () => {
		const folder = path.join(root, "Missing apps");
		fs.mkdirSync(folder);
		fixture(folder, "ToskLight Architect.app", "com.example.unrelated");
		assert.ok(verify(folder).every((entry) => entry.application === ""));
	});
	test("prefers the launcher sibling over other installed copies", () => {
		const folder = path.join(root, "Sibling suite");
		fs.mkdirSync(folder);
		const copied = path.join(folder, "ToskLight Launcher.app");
		fs.cpSync(app, copied, { recursive: true });
		const expected = fixture(folder, "Matching Architect.app", "de.tokenet.tosklight.visualizer");
		assert.equal(verify(undefined, path.join(copied, "Contents/MacOS/ToskLight Launcher"))
			.find((entry) => entry.id === "architect").application, expected);
	});
	test("ignores retained update backups and staging bundles", () => {
		const folder = path.join(root, "Retained update backups");
		const expected = fixture(folder, "ToskLight Control.app", "de.tokenet.light");
		fixture(folder, ".tosklight-backup-old-Control.app", "de.tokenet.light");
		fixture(folder, ".tosklight-update-pending.app", "de.tokenet.light");
		assert.equal(verify(folder).find((entry) => entry.id === "control").application, expected);
	});
	test("has its own signed application identity and regular Dock behavior", () => {
		const plist = path.join(app, "Contents/Info.plist");
		const read = (key) => spawnSync("/usr/libexec/PlistBuddy", ["-c", `Print :${key}`, plist], { encoding: "utf8" });
		assert.equal(read("CFBundleIdentifier").stdout.trim(), "de.tokenet.tosklight.launcher");
		assert.equal(read("CFBundleShortVersionString").stdout.trim(), "1.8.0");
		assert.notEqual(read("LSUIElement").stdout.trim(), "true");
		const signed = spawnSync("codesign", ["--verify", "--deep", "--strict", app], { encoding: "utf8" });
		assert.equal(signed.status, 0, signed.stderr);
	});
});
