import assert from "node:assert/strict";
import test from "node:test";
import { installLocations, PRODUCTS, selectedProducts } from "./install-all.mjs";

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
