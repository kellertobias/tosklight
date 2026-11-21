import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";
import { renderCask, verifyBundle } from "./update-homebrew-cask.mjs";

function scratch(t) {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const directory = fs.mkdtempSync(path.join(artifactPaths.tmp, "homebrew-test-"));
	t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
	return directory;
}

test("a cask update rejects altered downloads and incomplete Architect installations", (t) => {
	const archive = path.join(scratch(t), "bundle.zip");
	fs.writeFileSync(archive, "downloaded archive");
	const digest = createHash("sha256").update(fs.readFileSync(archive)).digest("hex");
	const entries = [
		"sign-macos-apps-locally.sh",
		"tosklight-desk-macos_arm64.app/Contents/MacOS/light-desktop",
		"tosklight-desk-macos_arm64.app/Contents/MacOS/light-headless",
		"tosklight-architect-macos_arm64.app/Contents/MacOS/ToskLight Architect",
		"tosklight-architect-macos_arm64.app/Contents/MacOS/viz-renderer",
		"tosklight-architect-macos_arm64.app/Contents/MacOS/light-headless",
		"tosklight-media-macos_arm64.app/Contents/MacOS/ToskLight Pixel",
	].map((entry) => `tosklight-bundle-macos_arm64/${entry}`);
	assert.equal(verifyBundle(archive, digest, entries), digest);
	assert.throws(() => verifyBundle(archive, "0".repeat(64), entries), /SHA-256/);
	assert.throws(() => verifyBundle(archive, digest, entries.filter((entry) => !entry.endsWith("viz-renderer"))), /viz-renderer/);
	assert.throws(() => verifyBundle(archive, digest, entries.filter((entry) => !entry.endsWith("sign-macos-apps-locally.sh"))), /sign-macos-apps-locally/);
	assert.throws(() => renderCask('1.0.0"; system("bad")', digest), /semantic version/);
	assert.throws(() => renderCask("1.0.0", "bad"), /SHA-256/);
});

test("the committed cask is reproducible from its pinned release and checksum", () => {
	const source = fs.readFileSync(path.join(repositoryRoot, "Casks/tosklight.rb"), "utf8");
	const version = source.match(/version "([^"]+)"/)[1];
	const checksum = source.match(/sha256 "([^"]+)"/)[1];
	assert.equal(source, renderCask(version, checksum));
});

function installer(t, { installed = false, failInstall = false, machine = "arm64" } = {}) {
	const directory = scratch(t);
	const log = path.join(directory, "commands");
	fs.writeFileSync(path.join(directory, "uname"), `#!/bin/bash\nif [[ "$1" == -s ]]; then echo Darwin; else echo ${machine}; fi\n`, { mode: 0o755 });
	fs.writeFileSync(path.join(directory, "brew"), `#!/bin/bash
printf '%s\\n' "$*" >> "$HOMEBREW_TEST_LOG"
if [[ "$1" == list ]]; then exit ${installed ? 0 : 1}; fi
if [[ "$1" == install || "$1" == reinstall ]]; then exit ${failInstall ? 1 : 0}; fi
`, { mode: 0o755 });
	const result = spawnSync("/bin/bash", [path.join(repositoryRoot, "tools/install-homebrew.sh")], {
		encoding: "utf8",
		env: { ...process.env, PATH: `${directory}:/usr/bin:/bin`, HOMEBREW_TEST_LOG: log },
	});
	return { ...result, commands: fs.existsSync(log) ? fs.readFileSync(log, "utf8") : "" };
}

test("the installer selects the GitHub tap and refreshes signatures on a repeated run", (t) => {
	const first = installer(t);
	assert.equal(first.status, 0, first.stderr);
	assert.match(first.commands, /^tap kellertobias\/tosklight https:\/\/github.com\/kellertobias\/tosklight.git\nupdate\ntrust --cask kellertobias\/tosklight\/tosklight\n/);
	assert.match(first.commands, /install --cask kellertobias\/tosklight\/tosklight\n$/);
	const repeat = installer(t, { installed: true });
	assert.equal(repeat.status, 0, repeat.stderr);
	assert.match(repeat.commands, /reinstall --cask kellertobias\/tosklight\/tosklight\n$/);
});

test("failed installs report failure and unsupported Macs never change Homebrew", (t) => {
	for (const installed of [false, true]) {
		const failed = installer(t, { installed, failInstall: true });
		assert.notEqual(failed.status, 0);
		assert.doesNotMatch(failed.stdout, /ToskLight is installed/);
	}
	const intel = installer(t, { machine: "x86_64" });
	assert.notEqual(intel.status, 0);
	assert.match(intel.stderr, /Apple Silicon/);
	assert.equal(intel.commands, "");
});
