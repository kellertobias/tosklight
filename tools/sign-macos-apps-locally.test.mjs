import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { artifactPaths } from "./artifact-paths.mjs";

const script = path.resolve("tools/sign-macos-apps-locally.sh");
const mac = { skip: process.platform !== "darwin" };
function run(command, args, options = {}) {
	return spawnSync(command, args, { encoding: "utf8", ...options });
}
function checked(command, args, options) {
	const result = run(command, args, options);
	assert.equal(result.status, 0, result.stderr || result.stdout);
	return result;
}
function fixture(t, name = "ToskLight.app", identifier = "de.tokenet.light") {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const root = fs.mkdtempSync(path.join(artifactPaths.tmp, "local-signing-"));
	t.after(() => fs.rmSync(root, { recursive: true, force: true }));
	const app = path.join(root, name);
	fs.mkdirSync(path.join(app, "Contents/MacOS"), { recursive: true });
	fs.mkdirSync(path.join(app, "Contents/Resources/tools"), { recursive: true });
	fs.writeFileSync(
		path.join(app, "Contents/Info.plist"),
		`<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>${identifier}</string><key>CFBundleExecutable</key><string>test-app</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>`,
	);
	const source = path.join(root, "helper.c");
	fs.writeFileSync(source, "int main(void) { return 0; }\n");
	const executable = path.join(app, "Contents/MacOS/test-app");
	checked("/usr/bin/clang", [source, "-o", executable]);
	const helper = path.join(app, "Contents/Resources/tools/private-helper");
	fs.copyFileSync(executable, helper);
	return { root, app, executable, helper };
}

test(
	"individual downloaded app is signed with every Mach-O helper without changing quarantine",
	mac,
	(t) => {
		const { app, executable, helper } = fixture(t);
		const quarantine = "0081;65000000;ToskLightSigningAcceptance;";
		checked("/usr/bin/xattr", ["-w", "com.apple.quarantine", quarantine, app]);
		const result = checked("bash", [script, app]);
		assert.match(result.stdout, /Download quarantine is unchanged/);
		for (const item of [executable, helper, app]) {
			checked("/usr/bin/codesign", ["--verify", "--deep", "--strict", item]);
			assert.match(
				checked("/usr/bin/codesign", ["--display", "--verbose=4", item]).stderr,
				/Signature=adhoc/,
			);
		}
		assert.equal(
			checked("/usr/bin/xattr", [
				"-p",
				"com.apple.quarantine",
				app,
			]).stdout.trim(),
			quarantine,
		);
	},
);

test(
	"combined downloads support all suite identifiers independent of app filenames",
	mac,
	(t) => {
		const { root, app } = fixture(t, "tosklight-desk-macos_arm64.app");
		for (const [name, identifier] of [
			["Architect.app", "de.tokenet.tosklight.visualizer"],
			["Pixel.app", "de.tokenet.tosklight.media"],
			["Hardware.app", "de.tokenet.tosklight.hardware-controls"],
		]) {
			const destination = path.join(root, name);
			fs.cpSync(app, destination, { recursive: true });
			checked("/usr/libexec/PlistBuddy", [
				"-c",
				`Set :CFBundleIdentifier ${identifier}`,
				path.join(destination, "Contents/Info.plist"),
			]);
		}
		checked("bash", [script, root]);
		for (const name of fs
			.readdirSync(root)
			.filter((name) => name.endsWith(".app")))
			checked("/usr/bin/codesign", [
				"--verify",
				"--deep",
				"--strict",
				path.join(root, name),
			]);
	},
);

test(
	"refuses symlinked apps, outside helper links, and unrelated apps before signing",
	mac,
	(t) => {
		const { root, app, executable } = fixture(t);
		const alias = path.join(root, "alias.app");
		fs.symlinkSync(app, alias);
		assert.notEqual(run("bash", [script, alias]).status, 0);
		fs.unlinkSync(alias);
		fs.symlinkSync(
			"/usr/bin/true",
			path.join(app, "Contents/Resources/outside"),
		);
		assert.notEqual(run("bash", [script, app]).status, 0);
		fs.unlinkSync(path.join(app, "Contents/Resources/outside"));
		checked("/usr/libexec/PlistBuddy", [
			"-c",
			"Set :CFBundleIdentifier com.example.unrelated",
			path.join(app, "Contents/Info.plist"),
		]);
		assert.notEqual(run("bash", [script, root]).status, 0);
		assert.equal(
			run("/usr/bin/codesign", ["--verify", app]).status === 0,
			false,
		);
		assert.ok(fs.existsSync(executable));
	},
);

test(
	"valid Developer ID apps are verified and kept; invalid ones fail without ad-hoc replacement",
	mac,
	(t) => {
		const { root, app } = fixture(t);
		checked("bash", [script, app]);
		const wrapper = path.join(root, "bin");
		fs.mkdirSync(wrapper);
		const log = path.join(root, "codesign-arguments.txt");
		fs.writeFileSync(
			path.join(wrapper, "codesign"),
			// biome-ignore lint/suspicious/noTemplateCurlyInString: These are literal Bash expansions in the mocked command.
			'#!/bin/bash\nprintf "%s\\n" "$*" >> "$SIGNING_CALL_LOG"\nif [[ "$1" == --display ]]; then echo "Authority=Developer ID Application: Test Fixture (TESTTEAM)" >&2; exit 0; fi\nif [[ "$1" == --verify && "${INVALID_SIGNATURE:-}" == 1 ]]; then exit 1; fi\nexec /usr/bin/codesign "$@"\n',
			{ mode: 0o755 },
		);
		const env = {
			...process.env,
			PATH: `${wrapper}:${process.env.PATH}`,
			SIGNING_CALL_LOG: log,
		};
		assert.match(
			checked("bash", [script, app], { env }).stdout,
			/Keeping the valid Developer ID signature/,
		);
		assert.doesNotMatch(fs.readFileSync(log, "utf8"), /--force|--sign/);
		fs.writeFileSync(log, "");
		assert.notEqual(
			run("bash", [script, app], { env: { ...env, INVALID_SIGNATURE: "1" } })
				.status,
			0,
		);
		assert.doesNotMatch(fs.readFileSync(log, "utf8"), /--force|--sign/);
	},
);
