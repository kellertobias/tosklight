import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	extractArchive,
	findApplications,
	nonSystemDependencies,
	validateArchiveEntries,
} from "./ci-smoke-macos-download.mjs";

test("download archives accept application names with spaces and contained framework links", () => {
	validateArchiveEntries([
		{
			path: "Suite Download/ToskLight Architect.app/Contents/MacOS/ToskLight Architect",
		},
		{
			path: "Suite Download/ToskLight Architect.app/Contents/Frameworks/A.framework/Versions/Current",
			linkTarget: "A",
		},
		{
			path: "Suite Download/ToskLight Architect.app/Contents/Frameworks/A.framework/Versions/A/A",
		},
	]);
});

test("download archives refuse traversal, external symlinks and writes through symlinks", () => {
	for (const entry of [
		"../outside",
		"/outside",
		"Suite/../../outside",
		"C:\\outside",
	])
		assert.throws(
			() => validateArchiveEntries([{ path: entry }]),
			/Unsafe ZIP entry/u,
		);
	assert.throws(
		() =>
			validateArchiveEntries([
				{ path: "Suite/link", linkTarget: "../../outside" },
			]),
		/leaves/u,
	);
	assert.throws(
		() => validateArchiveEntries([{ path: "Suite/link", linkTarget: "/tmp" }]),
		/leaves/u,
	);
	assert.throws(
		() =>
			validateArchiveEntries([
				{ path: "Suite/link", linkTarget: "safe" },
				{ path: "Suite/link/payload" },
			]),
		/writes through/u,
	);
});

test("Mach-O checks permit system and bundled dependencies but reject build-machine paths", () => {
	const output = `Suite/ToskLight Architect.app/Contents/MacOS/ToskLight Architect:
    /System/Library/Frameworks/AppKit.framework/Versions/C/AppKit (compatibility version 45.0.0, current version 1.0.0)
    /usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1.0.0)
    @rpath/Bundled Library.framework/Bundled Library (compatibility version 1.0.0, current version 1.0.0)
    /opt/homebrew/lib/libexample.dylib (compatibility version 1.0.0, current version 1.0.0)
    /Users/build agent/libexample.dylib (compatibility version 1.0.0, current version 1.0.0)
`;
	assert.deepEqual(nonSystemDependencies(output), [
		"/opt/homebrew/lib/libexample.dylib",
		"/Users/build agent/libexample.dylib",
	]);
});

test("actual ZIP extraction identifies the three applications by ID despite names with spaces", {
	skip: process.platform !== "darwin",
}, async (t) => {
	await fs.mkdir(artifactPaths.tmp, { recursive: true });
	const root = await fs.mkdtemp(
		path.join(artifactPaths.tmp, "download-smoke-test-"),
	);
	t.after(() => fs.rm(root, { recursive: true, force: true }));
	const suite = path.join(root, "Suite Download With Spaces");
	const products = [
		["The Desk", "de.tokenet.light", "light-desktop"],
		["The Architect", "de.tokenet.tosklight.visualizer", "ToskLight Architect"],
		["The Pixel", "de.tokenet.tosklight.media", "ToskLight Pixel"],
	];
	for (const [name, id, executable] of products) {
		const contents = path.join(suite, `${name}.app/Contents`);
		await fs.mkdir(path.join(contents, "MacOS"), { recursive: true });
		await fs.writeFile(
			path.join(contents, "Info.plist"),
			`<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>${id}</string><key>CFBundleExecutable</key><string>${executable}</string></dict></plist>`,
		);
		await fs.writeFile(
			path.join(contents, "MacOS", executable),
			"fixture only; never launched",
		);
	}
	const archive = path.join(root, "Downloaded Suite With Spaces.zip");
	await promisify(execFile)("/usr/bin/ditto", [
		"-c",
		"-k",
		"--keepParent",
		suite,
		archive,
	]);
	const extracted = path.join(root, "Extracted With Spaces");
	await extractArchive(archive, extracted);
	const found = await findApplications(extracted);
	assert.deepEqual([...found.keys()].sort(), ["architect", "control", "pixel"]);
	assert.equal(
		path.basename(found.get("architect").executable),
		"ToskLight Architect",
	);
	assert.match(
		found.get("architect").app,
		/Suite Download With Spaces\/The Architect\.app$/u,
	);
});
