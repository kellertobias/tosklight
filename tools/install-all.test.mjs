import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
	installLocations,
	keepPreviousVersion,
	PRODUCTS,
	previousVersionPath,
	revertToPreviousVersion,
	selectedProducts,
} from "./install-all.mjs";

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
	const root = fs.mkdtempSync(path.join(os.tmpdir(), "install-all-"));
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
