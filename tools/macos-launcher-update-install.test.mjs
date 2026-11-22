import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { after, before, describe, test } from "node:test";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const products = [
	["Control", "de.tokenet.light"],
	["Architect", "de.tokenet.tosklight.visualizer"],
	["Pixel", "de.tokenet.tosklight.media"],
	["Launcher", "de.tokenet.tosklight.launcher"],
];

function checked(command, args) {
	const result = spawnSync(command, args, { encoding: "utf8", timeout: 120_000 });
	assert.equal(result.status, 0, result.stderr || result.stdout || result.error?.message);
	return result;
}

describe("the macOS launcher update installer", { skip: process.platform !== "darwin" }, () => {
	let root, installer, executable;
	before(() => {
		fs.mkdirSync(artifactPaths.tmp, { recursive: true });
		root = fs.realpathSync(fs.mkdtempSync(path.join(artifactPaths.tmp, "launcher-update-installer-")));
		installer = path.join(root, "InstallUpdate");
		checked("xcrun", [
			"swiftc", "-parse-as-library", "-swift-version", "5", "-framework", "AppKit",
			"-module-cache-path", path.join(root, "swift-modules"),
			path.join(repositoryRoot, "apps/macos-launcher/InstallUpdate.swift"), "-o", installer,
		]);
		const source = path.join(root, "fixture.c");
		fs.writeFileSync(source, "int main(void) { return 0; }\n");
		executable = path.join(root, "fixture");
		checked("/usr/bin/clang", [source, "-o", executable]);
	});
	after(() => { if (root) fs.rmSync(root, { recursive: true, force: true }); });

	function app(directory, name, identifier, generation) {
		const bundle = path.join(directory, `${name}.app`);
		fs.mkdirSync(path.join(bundle, "Contents/MacOS"), { recursive: true });
		fs.mkdirSync(path.join(bundle, "Contents/Resources"), { recursive: true });
		fs.copyFileSync(executable, path.join(bundle, "Contents/MacOS/fixture"));
		fs.writeFileSync(path.join(bundle, "Contents/Resources/generation"), generation);
		fs.writeFileSync(path.join(bundle, "Contents/Info.plist"),
			`<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>${identifier}</string><key>CFBundleExecutable</key><string>fixture</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>`);
		checked("/usr/bin/codesign", ["--force", "--sign", "-", "--timestamp=none", bundle]);
		return bundle;
	}

	function fixture(t) {
		const directory = fs.mkdtempSync(path.join(root, "suite-"));
		t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
		const downloads = path.join(directory, "downloaded apps");
		const applications = path.join(directory, "Applications");
		const items = products.map(([name, bundleIdentifier]) => ({
			source: app(downloads, `${name} downloaded`, bundleIdentifier, "new"),
			destination: app(applications, `ToskLight ${name}`, bundleIdentifier, "old"),
			bundleIdentifier,
		}));
		const show = path.join(directory, "Show Data", "live.show");
		fs.mkdirSync(path.dirname(show));
		fs.writeFileSync(show, "operator show and settings remain untouched");
		return { directory, applications, items, show };
	}

	function install(suite, overrides = {}) {
		const resultURL = path.join(suite.directory, "result.json");
		const manifest = {
			launcherPID: 0, launcherURL: suite.items[3].destination, resultURL,
			items: suite.items, reopen: false, ...overrides,
		};
		const input = path.join(suite.directory, "manifest.json");
		fs.writeFileSync(input, JSON.stringify(manifest));
		const process = spawnSync(installer, [input], { encoding: "utf8", timeout: 45_000 });
		assert.equal(process.signal, null, process.error?.message || process.stderr);
		assert.ok(fs.existsSync(resultURL), process.stderr || process.stdout);
		return { process, result: JSON.parse(fs.readFileSync(resultURL, "utf8")) };
	}

	function verifyGeneration(suite, expected) {
		for (const { destination } of suite.items) {
			assert.equal(fs.readFileSync(path.join(destination, "Contents/Resources/generation"), "utf8"), expected);
			checked("/usr/bin/codesign", ["--verify", "--deep", "--strict", destination]);
		}
		assert.equal(fs.readFileSync(suite.show, "utf8"), "operator show and settings remain untouched");
		assert.equal(fs.readdirSync(suite.applications).filter((name) => name.startsWith(".tosklight-")).length, 0);
	}

	test("replaces all four signed apps while preserving external shows and settings", (t) => {
		const suite = fixture(t);
		const { process, result } = install(suite);
		assert.equal(process.status, 0, result.message);
		assert.equal(result.success, true);
		assert.deepEqual(result.backups, []);
		verifyGeneration(suite, "new");
	});

	test("accepts local file URLs and installs an application missing from the destination", (t) => {
		const suite = fixture(t);
		fs.rmSync(suite.items[2].destination, { recursive: true });
		const items = suite.items.map((item) => ({
			...item, source: new URL(`file://${item.source}`).href,
			destination: new URL(`file://${item.destination}`).href,
		}));
		const { result } = install(suite, { items, launcherURL: items[3].destination });
		assert.equal(result.success, true, result.message);
		verifyGeneration(suite, "new");
	});

	test("rejects an incomplete four-product manifest before staging anything", (t) => {
		const suite = fixture(t);
		const { process, result } = install(suite, { items: suite.items.slice(0, 3) });
		assert.equal(process.status, 1);
		assert.equal(result.success, false);
		assert.match(result.message, /exactly one Control, Architect, Pixel, and Launcher/);
		verifyGeneration(suite, "old");
	});

	test("waits for the original launcher after an early validation failure", async (t) => {
		const suite = fixture(t);
		const previousLauncher = spawn("/bin/sleep", ["1"]);
		let exited = false;
		previousLauncher.on("exit", () => { exited = true; });
		t.after(() => { if (!exited) previousLauncher.kill(); });
		const resultURL = path.join(suite.directory, "early-result.json");
		const manifest = path.join(suite.directory, "early-manifest.json");
		fs.writeFileSync(manifest, JSON.stringify({
			launcherPID: previousLauncher.pid, launcherURL: suite.items[3].destination,
			resultURL, items: suite.items.slice(0, 3), reopen: false,
		}));
		const helper = spawn(installer, [manifest], { stdio: "ignore" });
		const status = await new Promise((resolve, reject) => {
			helper.on("error", reject);
			helper.on("exit", resolve);
		});
		assert.equal(status, 1);
		assert.equal(exited, true, "The helper must not reopen a launcher still waiting to quit");
		assert.equal(JSON.parse(fs.readFileSync(resultURL)).success, false);
		verifyGeneration(suite, "old");
	});

	test("rejects a source identity mismatch before changing installed applications", (t) => {
		const suite = fixture(t);
		checked("/usr/libexec/PlistBuddy", ["-c", "Set :CFBundleIdentifier com.example.unrelated", path.join(suite.items[2].source, "Contents/Info.plist")]);
		const { result } = install(suite);
		assert.equal(result.success, false);
		assert.match(result.message, /identity does not match/);
		verifyGeneration(suite, "old");
	});

	test("refuses to overwrite an installed app with another bundle identity", (t) => {
		const suite = fixture(t);
		const destination = suite.items[1].destination;
		fs.rmSync(destination, { recursive: true });
		app(suite.applications, "ToskLight Architect", "com.example.unrelated", "old");
		const { result } = install(suite);
		assert.equal(result.success, false);
		assert.match(result.message, /identity does not match/);
		verifyGeneration(suite, "old");
	});

	test("never writes an update result inside an application bundle", (t) => {
		const suite = fixture(t);
		const resultURL = path.join(suite.items[0].destination, "Contents/Resources/result.json");
		const input = path.join(suite.directory, "manifest.json");
		fs.writeFileSync(input, JSON.stringify({
			launcherPID: 0, launcherURL: suite.items[3].destination, resultURL,
			items: suite.items, reopen: false,
		}));
		const result = spawnSync(installer, [input], { encoding: "utf8", timeout: 45_000 });
		assert.equal(result.status, 1);
		assert.match(result.stderr, /outside application bundles/);
		assert.equal(fs.existsSync(resultURL), false);
		verifyGeneration(suite, "old");
	});

	test("rejects a missing source and an invalid signature without changing installed apps", async (t) => {
		for (const failure of ["missing", "signature"]) {
			await t.test(failure, (t) => {
				const suite = fixture(t);
				if (failure === "missing") fs.rmSync(suite.items[2].source, { recursive: true });
				else fs.writeFileSync(path.join(suite.items[2].source, "Contents/Resources/generation"), "tampered");
				const { result } = install(suite);
				assert.equal(result.success, false);
				assert.match(result.message, failure === "missing" ? /missing an application/ : /codesign failed/);
				verifyGeneration(suite, "old");
			});
		}
	});

	test("refuses symlinks in a source bundle, destination, or destination parent", async (t) => {
		for (const position of ["source bundle", "destination", "destination parent"]) {
			await t.test(position, (t) => {
				const suite = fixture(t);
				let items = suite.items;
				if (position === "source bundle") {
					fs.symlinkSync("/usr/bin/true", path.join(items[1].source, "Contents/Resources/outside"));
				} else if (position === "destination") {
					const original = items[1].destination;
					const link = path.join(suite.directory, "Architect alias.app");
					fs.symlinkSync(original, link);
					items = items.map((item, index) => index === 1 ? { ...item, destination: link } : item);
				} else {
					const alias = path.join(suite.directory, "Applications alias");
					fs.symlinkSync(suite.applications, alias);
					items = items.map((item) => ({ ...item, destination: path.join(alias, path.basename(item.destination)) }));
				}
				const { result } = install(suite, { items, launcherURL: items[3].destination });
				assert.equal(result.success, false);
				assert.match(result.message, /Symbolic links/);
				verifyGeneration(suite, "old");
			});
		}
	});

	test("reports a nonwritable Applications folder before changing any applications", (t) => {
		const suite = fixture(t);
		fs.chmodSync(suite.applications, 0o555);
		try {
			const { result } = install(suite);
			assert.equal(result.success, false);
			assert.match(result.message, /writable Applications folder.*Homebrew/);
			verifyGeneration(suite, "old");
		} finally { fs.chmodSync(suite.applications, 0o755); }
	});

	test("rolls back earlier replacements when a later application cannot be moved", (t) => {
		const suite = fixture(t);
		const locked = suite.items[1].destination;
		checked("/usr/bin/chflags", ["uchg", locked]);
		try {
			const { process, result } = install(suite);
			assert.equal(process.status, 1);
			assert.equal(result.success, false);
			assert.match(result.message, /All replaced applications were restored/);
			assert.deepEqual(result.backups, []);
			verifyGeneration(suite, "old");
		} finally { checked("/usr/bin/chflags", ["nouchg", locked]); }
	});
});
