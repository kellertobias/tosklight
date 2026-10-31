import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	assembleArchitectMac,
	installLocations,
	keepPreviousVersion,
	PRODUCTS,
	previousVersionPath,
	revertToPreviousVersion,
	selectedProducts,
} from "./install-all.mjs";

test("Architect installation assembles matching helpers and retains its packaged resources", {
	skip: process.platform !== "darwin",
}, (t) => {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const root = fs.mkdtempSync(path.join(artifactPaths.tmp, "install-architect-"));
	t.after(() => fs.rmSync(root, { recursive: true, force: true }));
	const release = path.join(root, "release");
	const tmp = path.join(root, "staging");
	const app = path.join(release, "bundle/macos/ToskLight Architect.app");
	const macos = path.join(app, "Contents/MacOS");
	const resources = path.join(app, "Contents/Resources");
	const mcp = path.join(root, ".artifacts/build/patch-mcp/tosklight-patch-mcp.mjs");
	for (const directory of [macos, tmp, path.dirname(mcp),
		path.join(resources, "fixture-library"), path.join(resources, "demo-show")])
		fs.mkdirSync(directory, { recursive: true });
	fs.writeFileSync(path.join(macos, "viz-editor"), "matching editor");
	fs.writeFileSync(path.join(release, "viz-renderer"), "matching renderer");
	fs.writeFileSync(path.join(release, "light-headless"), "matching private show server");
	fs.writeFileSync(mcp, "export const matchingMcp = true;\n");
	fs.writeFileSync(path.join(resources, "fixture-library/lamp.toskfixture"), "matching profile");
	fs.writeFileSync(path.join(resources, "demo-show/demo-show.show"), "matching demo");
	const expected = [
		["Contents/MacOS/ToskLight Architect", "matching editor"],
		["Contents/MacOS/viz-renderer", "matching renderer"],
		["Contents/MacOS/light-headless", "matching private show server"],
		["Contents/Resources/tosklight-patch-mcp.mjs", "export const matchingMcp = true;\n"],
		["Contents/Resources/fixture-library/lamp.toskfixture", "matching profile"],
		["Contents/Resources/demo-show/demo-show.show", "matching demo"],
	];
	const assertComplete = () => {
		for (const [relative, contents] of expected)
			assert.equal(fs.readFileSync(path.join(app, relative), "utf8"), contents, relative);
	};
	let sealed = false;
	const run = (command, args) => {
		assert.equal(command, "bash");
		if (args[0] === "tools/seal-macos-app.sh") {
			assert.deepEqual(args.slice(1), [app]);
			assertComplete();
			sealed = true;
			return;
		}
		assert.equal(args[0], "tools/bundle-visualizer-macos.sh");
		const result = spawnSync(command, args, { encoding: "utf8" });
		assert.equal(result.status, 0, result.stderr);
	};
	assert.equal(assembleArchitectMac(run, release, tmp, mcp), app);
	assertComplete();
	assert.equal(sealed, true, "the complete app is sealed after its resources are restored");
});

test("macOS installs each product as an app in the user's Applications folder", () => {
	const locations = installLocations("darwin", { home: "/Users/operator" });
	assert.deepEqual(
		PRODUCTS.map((product) => locations[product].directory),
		[
			"/Users/operator/Applications/ToskLight.app",
			"/Users/operator/Applications/ToskLight Architect.app",
			"/Users/operator/Applications/ToskLight Pixel.app",
		],
	);
});

test("Windows installs per user under Programs, where the Pixel installer puts Pixel", () => {
	const locations = installLocations("win32", { localAppData: "C:\\Users\\operator\\AppData\\Local" });
	assert.equal(locations.pixel.directory, "C:\\Users\\operator\\AppData\\Local\\Programs\\ToskLight Pixel");
	assert.equal(locations.architect.executable, "ToskLight Architect.exe");
});

test("Linux installs under ~/.local/opt with a desktop launcher", () => {
	const locations = installLocations("linux", { home: "/home/operator" });
	assert.equal(locations.control.directory, "/home/operator/.local/opt/tosklight");
	assert.equal(
		locations.architect.launcher,
		"/home/operator/.local/share/applications/tosklight-architect.desktop",
	);
});

test("--only narrows the products and refuses unknown names", () => {
	assert.deepEqual(selectedProducts([]), PRODUCTS);
	assert.deepEqual(selectedProducts(["--only", "pixel,control"]), ["control", "pixel"]);
	assert.throws(() => selectedProducts(["--only", "desk"]), /unknown: desk/u);
	assert.throws(() => installLocations("freebsd", {}), /not freebsd/u);
});

test("--self-contained places portable copies on the Desktop", () => {
	const mac = installLocations("darwin", { home: "/Users/operator", selfContained: true });
	assert.equal(mac.architect.directory, "/Users/operator/Desktop/ToskLight Architect.app");
	const windows = installLocations("win32", {
		desktop: "C:\\Users\\operator\\OneDrive\\Desktop",
		selfContained: true,
	});
	assert.equal(windows.control.directory, "C:\\Users\\operator\\OneDrive\\Desktop\\ToskLight");
	assert.equal(windows.control.portable, true);
	const linux = installLocations("linux", { home: "/home/operator", selfContained: true });
	assert.equal(linux.control.directory, "/home/operator/Desktop/ToskLight.AppImage");
	assert.equal(linux.control.appImage, true);
	assert.equal(linux.pixel.directory, "/home/operator/Desktop/ToskLight Pixel");
});

test("one previous version is kept beside each install and a revert restores it", () => {
	fs.mkdirSync(artifactPaths.tmp, { recursive: true });
	const root = fs.mkdtempSync(path.join(artifactPaths.tmp, "install-all-"));
	const installed = path.join(root, "ToskLight.app");
	const write = (version) => {
		fs.mkdirSync(installed, { recursive: true });
		fs.writeFileSync(path.join(installed, "version"), version);
	};
	const read = (folder) => fs.readFileSync(path.join(folder, "version"), "utf8");
	assert.equal(previousVersionPath(installed), path.join(root, ".old", "ToskLight.app"));
	assert.equal(previousVersionPath("C:\\Apps\\ToskLight"), "C:\\Apps\\.old\\ToskLight");

	assert.equal(keepPreviousVersion(installed), null, "a first install keeps nothing");
	write("1");
	keepPreviousVersion(installed);
	write("2");
	keepPreviousVersion(installed);
	write("3");
	assert.equal(read(previousVersionPath(installed)), "2", "only the version before is kept");
	assert.deepEqual(fs.readdirSync(path.join(root, ".old")), ["ToskLight.app"]);

	revertToPreviousVersion(installed);
	assert.equal(read(installed), "2");
	assert.equal(fs.existsSync(path.join(root, ".old")), false, "the emptied .old folder goes");
	assert.throws(() => revertToPreviousVersion(installed), /no previous version/u);
	fs.rmSync(root, { recursive: true, force: true });
});
