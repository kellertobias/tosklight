#!/usr/bin/env node
// Builds ToskLight Control, ToskLight Architect and ToskLight Pixel for this machine and installs
// them where applications live on its operating system:
//
//   macOS    ~/Applications/<Product>.app
//   Windows  %LOCALAPPDATA%\Programs\<Product>, with a Start menu shortcut
//   Linux    ~/.local/opt/<product>, with a launcher in ~/.local/share/applications
//
// --self-contained places portable copies on the Desktop instead: the .app bundles on macOS, a
// folder that keeps its data beside itself on Windows (portable.txt), and on Linux the desk as one
// AppImage and Architect and Pixel as portable folders.
//
// Installing moves the version already there into a `.old` folder beside it, replacing whatever
// was kept there before, so each product keeps exactly one previous version. --revert deletes the
// current version and puts that previous one back.
//
// Every build is a release build for the host target, assembled the way the release workflow
// assembles it, so what gets installed is what ships.
//
// usage: npm run install-all -- [--only control,architect,pixel] [--self-contained] [--revert] [--dry-run]

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const PRODUCTS = ["control", "architect", "pixel"];

const PRODUCT_NAMES = {
	control: "ToskLight",
	architect: "ToskLight Architect",
	pixel: "ToskLight Pixel",
};

/** Where each product is installed on `platform`, for a user whose home is `home`. */
export function installLocations(
	platform,
	{ home, localAppData, desktop, selfContained = false } = {},
) {
	if (selfContained) return portableLocations(platform, { home, desktop });
	if (platform === "darwin") {
		const applications = path.posix.join(home, "Applications");
		return Object.fromEntries(
			PRODUCTS.map((product) => [
				product,
				{
					directory: path.posix.join(applications, `${PRODUCT_NAMES[product]}.app`),
				},
			]),
		);
	}
	if (platform === "win32") {
		const programs = path.win32.join(localAppData, "Programs");
		return Object.fromEntries(
			PRODUCTS.map((product) => [
				product,
				{
					directory: path.win32.join(programs, PRODUCT_NAMES[product]),
					executable: `${PRODUCT_NAMES[product]}.exe`,
				},
			]),
		);
	}
	if (platform === "linux") {
		const slug = (product) => PRODUCT_NAMES[product].toLowerCase().replaceAll(" ", "-");
		return Object.fromEntries(
			PRODUCTS.map((product) => [
				product,
				{
					directory: path.posix.join(home, ".local", "opt", slug(product)),
					executable: slug(product),
					launcher: path.posix.join(
						home,
						".local",
						"share",
						"applications",
						`${slug(product)}.desktop`,
					),
				},
			]),
		);
	}
	throw new Error(`install-all supports macOS, Windows and Linux, not ${platform}`);
}

/** Where `--self-contained` places each product: on the user's Desktop. */
function portableLocations(platform, { home, desktop }) {
	if (platform === "darwin") {
		const folder = desktop ?? path.posix.join(home, "Desktop");
		return Object.fromEntries(
			PRODUCTS.map((product) => [
				product,
				{ directory: path.posix.join(folder, `${PRODUCT_NAMES[product]}.app`) },
			]),
		);
	}
	if (platform === "win32") {
		const folder = desktop ?? path.win32.join(home, "Desktop");
		return Object.fromEntries(
			PRODUCTS.map((product) => [
				product,
				{
					directory: path.win32.join(folder, PRODUCT_NAMES[product]),
					executable: `${PRODUCT_NAMES[product]}.exe`,
					portable: true,
				},
			]),
		);
	}
	if (platform === "linux") {
		const folder = desktop ?? path.posix.join(home, "Desktop");
		return {
			control: { directory: path.posix.join(folder, "ToskLight.AppImage"), appImage: true },
			architect: {
				directory: path.posix.join(folder, "ToskLight Architect"),
				executable: "tosklight-architect",
				portable: true,
			},
			pixel: {
				directory: path.posix.join(folder, "ToskLight Pixel"),
				executable: "tosklight-pixel",
				portable: true,
			},
		};
	}
	throw new Error(`install-all supports macOS, Windows and Linux, not ${platform}`);
}

/** The single previous version kept for an installed product: `.old/<name>` beside it. */
export function previousVersionPath(installed) {
	const pathApi = installed.includes("\\") ? path.win32 : path.posix;
	return pathApi.join(pathApi.dirname(installed), ".old", pathApi.basename(installed));
}

/**
 * Moves the installed version aside before a new one takes its place. The previous version kept
 * before is deleted first: there is only ever one.
 */
export function keepPreviousVersion(installed, fileSystem = fs) {
	if (!fileSystem.existsSync(installed)) return null;
	const previous = previousVersionPath(installed);
	fileSystem.rmSync(previous, { recursive: true, force: true });
	fileSystem.mkdirSync(path.dirname(previous), { recursive: true });
	fileSystem.renameSync(installed, previous);
	return previous;
}

/** Deletes the installed version and restores the previous one kept by the last install. */
export function revertToPreviousVersion(installed, fileSystem = fs) {
	const previous = previousVersionPath(installed);
	if (!fileSystem.existsSync(previous))
		throw new Error(`there is no previous version to restore at ${previous}`);
	fileSystem.rmSync(installed, { recursive: true, force: true });
	fileSystem.renameSync(previous, installed);
	const folder = path.dirname(previous);
	if (fileSystem.existsSync(folder) && fileSystem.readdirSync(folder).length === 0)
		fileSystem.rmdirSync(folder);
	return previous;
}

/** The products named by `--only`, in install order. */
export function selectedProducts(argv) {
	const index = argv.indexOf("--only");
	if (index === -1) return [...PRODUCTS];
	const named = (argv[index + 1] ?? "").split(",").map((name) => name.trim()).filter(Boolean);
	const unknown = named.filter((name) => !PRODUCTS.includes(name));
	if (!named.length || unknown.length)
		throw new Error(
			`--only takes a comma-separated list of ${PRODUCTS.join(", ")}${unknown.length ? `; unknown: ${unknown.join(", ")}` : ""}`,
		);
	return PRODUCTS.filter((product) => named.includes(product));
}

// ---------------------------------------------------------------------------------------------

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

async function main() {
	const argv = process.argv.slice(2);
	const dryRun = argv.includes("--dry-run");
	const selfContained = argv.includes("--self-contained");
	const revert = argv.includes("--revert");
	const products = selectedProducts(argv);
	const { artifactPaths } = await import("./artifact-paths.mjs");
	const platform = process.platform;
	const locations = installLocations(platform, {
		home: os.homedir(),
		localAppData: process.env.LOCALAPPDATA ?? path.join(os.homedir(), "AppData", "Local"),
		desktop: desktopFolder(platform),
		selfContained,
	});
	if (revert) {
		for (const product of products) {
			const installed = locations[product].directory;
			console.log(`Reverting ${PRODUCT_NAMES[product]} in ${installed}`);
			if (dryRun) continue;
			quitInstalled(platform, installed);
			revertToPreviousVersion(installed);
			console.log(`Restored the previous ${PRODUCT_NAMES[product]}`);
		}
		return;
	}
	const target = hostTarget();
	const exe = platform === "win32" ? ".exe" : "";
	const release = path.join(artifactPaths.cargo, target, "release");
	const tmp = path.join(artifactPaths.tmp, "install-all");

	console.log(
		`Installing ${selfContained ? "self-contained " : ""}${products.map((product) => PRODUCT_NAMES[product]).join(", ")} for ${target}:`,
	);
	for (const product of products) console.log(`  ${PRODUCT_NAMES[product]} → ${locations[product].directory}`);
	if (dryRun) return;

	const run = (command, args, options = {}) => {
		console.log(`$ ${[command, ...args].join(" ")}`);
		const result = spawnSync(command, args, {
			cwd: repositoryRoot,
			stdio: "inherit",
			shell: platform === "win32",
			...options,
			env: { ...process.env, ...STATIC_RUNTIME, ...options.env },
		});
		if (result.status !== 0)
			throw new Error(`${command} ${args.join(" ")} failed with exit code ${result.status}`);
	};
	const cargo = (packages) =>
		run("cargo", [
			"build",
			"--release",
			"--locked",
			"--target",
			target,
			...packages.flatMap(([name, bin]) => ["-p", name, ...(bin ? ["--bin", bin] : [])]),
		]);
	fs.mkdirSync(tmp, { recursive: true });
	run("node", ["tools/ensure-workspace-dependencies.mjs"]);

	const built = {};
	if (products.includes("control") || products.includes("architect"))
		cargo([
			["light-headless", "light-headless"],
			["viz-renderer", "viz-renderer"],
		]);

	if (products.includes("control")) {
		run("node", ["tools/ensure-control-frontend.mjs"]);
		const sidecars = path.join(tmp, "desktop-sidecars");
		fs.mkdirSync(sidecars, { recursive: true });
		for (const binary of ["light-headless", "viz-renderer"])
			fs.copyFileSync(path.join(release, `${binary}${exe}`), path.join(sidecars, `${binary}-${target}${exe}`));
		const config = path.join(tmp, "tauri-control.json");
		const env = { LIGHT_DESKTOP_SIDECAR_DIR: sidecars, LIGHT_DESKTOP_SIDECAR_TARGET: target };
		run("node", ["tools/write-tauri-artifact-config.mjs", "control", config], { env });
		run(
			"npm",
			["run", "--prefix", "apps/light-desktop", "tauri:build", "--", "--target", target, "--config", config,
				...controlBundles(platform, selfContained)],
			{ env },
		);
		if (platform === "darwin") {
			const app = path.join(release, "bundle", "macos", "ToskLight.app");
			run("bash", ["tools/seal-macos-app.sh", app]);
			built.control = app;
		}
		if (locations.control.appImage) {
			const images = path.join(release, "bundle", "appimage");
			const image = fs.readdirSync(images).find((name) => name.endsWith(".AppImage"));
			if (!image) throw new Error(`the desk build produced no AppImage in ${images}`);
			built.control = path.join(images, image);
		}
	}

	if (products.includes("architect")) {
		run("npm", ["run", "build:patch-mcp"]);
		fs.mkdirSync(artifactPaths.demoShow, { recursive: true });
		fs.copyFileSync(path.join(repositoryRoot, "assets", "demo.show"), path.join(artifactPaths.demoShow, "demo-show.show"));
		run("npm", ["run", "--prefix", "apps/viz-editor", "build"]);
		const config = path.join(tmp, "tauri-architect.json");
		run("node", ["tools/write-tauri-artifact-config.mjs", "viz-editor", config]);
		run("npm", [
			"run", "--prefix", "apps/viz-editor", "tauri:build", "--", "--target", target, "--config", config,
			...(platform === "darwin" ? ["--bundles", "app"] : ["--no-bundle"]),
		]);
		if (platform === "darwin") built.architect = assembleArchitectMac(run, release, tmp);
	}

	if (products.includes("pixel")) {
		run("node", ["tools/ensure-media-frontend.mjs"]);
		cargo([
			["media-server", "media-server"],
			...(platform === "win32" ? [["pixel-launcher", "pixel-launcher"]] : []),
		]);
		if (platform === "darwin") {
			const bundles = path.join(release, "bundle", "macos");
			run("bash", ["tools/bundle-media-macos.sh", path.join(release, "media-server"), bundles]);
			const app = path.join(bundles, "ToskLight Pixel.app");
			run("bash", ["tools/seal-macos-app.sh", app]);
			built.pixel = app;
		}
	}

	for (const product of products) {
		const location = locations[product];
		quitInstalled(platform, location.directory);
		const previous = keepPreviousVersion(location.directory);
		if (previous) console.log(`Kept the previous ${PRODUCT_NAMES[product]} in ${previous}`);
		if (platform === "darwin") installMac(run, built[product], location.directory, PRODUCT_NAMES[product]);
		else if (location.appImage) installAppImage(built[product], location.directory);
		else installDirectory(product, location, { release, exe, platform, artifactPaths, run, selfContained });
		console.log(`Installed ${PRODUCT_NAMES[product]} in ${location.directory}`);
	}
	if (products.length)
		console.log("Revert to the previous versions with: npm run install-all -- --revert" +
			(selfContained ? " --self-contained" : "") +
			(products.length < PRODUCTS.length ? ` --only ${products.join(",")}` : ""));
}

/** Link the MSVC C runtime statically, so nothing needs the Visual C++ redistributable. */
const STATIC_RUNTIME =
	process.platform === "win32"
		? { CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: "-C target-feature=+crt-static" }
		: {};

function controlBundles(platform, selfContained) {
	if (platform === "darwin") return ["--bundles", "app"];
	if (platform === "linux" && selfContained) return ["--bundles", "appimage"];
	return ["--no-bundle"];
}

/** The user's Desktop folder, which Windows may have redirected (to OneDrive, for example). */
function desktopFolder(platform) {
	if (platform !== "win32") return undefined;
	const result = spawnSync(
		"powershell",
		["-NoProfile", "-NonInteractive", "-Command", "[Environment]::GetFolderPath('Desktop')"],
		{ encoding: "utf8" },
	);
	const folder = result.stdout?.trim();
	return folder || undefined;
}

/** A running copy keeps its executable open; it is asked to quit before it is moved aside. */
function quitInstalled(platform, installed) {
	if (platform === "darwin" && fs.existsSync(installed))
		spawnSync("osascript", ["-e", `quit app "${installed}"`], { stdio: "ignore" });
}

function installAppImage(image, destination) {
	if (!image || !fs.existsSync(image)) throw new Error(`no built AppImage at ${image}`);
	fs.mkdirSync(path.dirname(destination), { recursive: true });
	fs.copyFileSync(image, destination);
	fs.chmodSync(destination, 0o755);
}

function hostTarget() {
	const result = spawnSync("rustc", ["-vV"], { encoding: "utf8", shell: process.platform === "win32" });
	const host = result.stdout?.match(/^host: (.+)$/mu)?.[1]?.trim();
	if (!host) throw new Error("could not read the host target from `rustc -vV`; is Rust installed?");
	return host;
}

/** Architect is one bundle: the editor as its executable and the renderer as the helper it starts. */
function assembleArchitectMac(run, release, tmp) {
	const bundles = path.join(release, "bundle", "macos");
	const app = path.join(bundles, "ToskLight Architect.app");
	const accessory = fs.mkdtempSync(path.join(tmp, "architect-"));
	fs.copyFileSync(path.join(app, "Contents", "MacOS", "viz-editor"), path.join(accessory, "viz-editor"));
	fs.cpSync(path.join(app, "Contents", "Resources"), path.join(accessory, "resources"), { recursive: true });
	run("bash", [
		"tools/bundle-visualizer-macos.sh",
		path.join(accessory, "viz-editor"),
		path.join(release, "viz-renderer"),
		path.join(repositoryRoot, ".artifacts", "build", "patch-mcp", "tosklight-patch-mcp.mjs"),
		bundles,
	]);
	fs.copyFileSync(path.join(release, "light-headless"), path.join(app, "Contents", "MacOS", "light-headless"));
	fs.cpSync(path.join(accessory, "resources"), path.join(app, "Contents", "Resources"), { recursive: true });
	fs.rmSync(accessory, { recursive: true, force: true });
	run("bash", ["tools/seal-macos-app.sh", app]);
	return app;
}

function installMac(run, app, destination, name) {
	if (!app || !fs.existsSync(app)) throw new Error(`no built ${name}.app at ${app}`);
	fs.mkdirSync(path.dirname(destination), { recursive: true });
	run("ditto", [app, destination]);
}

/** Windows and Linux: the executable with its helpers and resources beside it, and a launcher. */
function installDirectory(product, location, { release, exe, platform, artifactPaths, selfContained }) {
	const directory = location.directory;
	const staging = `${directory}.installing`;
	fs.rmSync(staging, { recursive: true, force: true });
	fs.mkdirSync(staging, { recursive: true });
	const copy = (from, to = path.basename(from)) => {
		if (!fs.existsSync(from)) throw new Error(`expected build output is missing: ${from}`);
		fs.cpSync(from, path.join(staging, to), { recursive: true });
	};
	const fixtureLibrary = path.join(repositoryRoot, "assets", "fixture-library");
	if (product === "control") {
		copy(path.join(release, `light-desktop${exe}`), location.executable);
		copy(path.join(release, `light-headless${exe}`));
		copy(path.join(release, `viz-renderer${exe}`));
		copy(fixtureLibrary, "fixture-library");
	} else if (product === "architect") {
		copy(path.join(release, `viz-editor${exe}`), location.executable);
		copy(path.join(release, `viz-renderer${exe}`));
		copy(path.join(release, `light-headless${exe}`));
		copy(path.join(repositoryRoot, ".artifacts", "build", "patch-mcp", "tosklight-patch-mcp.mjs"));
		copy(fixtureLibrary, "fixture-library");
		copy(path.join(artifactPaths.demoShow, "demo-show.show"), path.join("demo-show", "demo-show.show"));
	} else {
		copy(path.join(release, `media-server${exe}`));
		if (platform === "win32") copy(path.join(release, "pixel-launcher.exe"), location.executable);
		else fs.symlinkSync("media-server", path.join(staging, location.executable));
	}
	// A portable folder keeps what the application writes beside it. Pixel always does: it reads its
	// configuration relative to the folder it is started from.
	if (location.portable && product !== "pixel")
		fs.copyFileSync(path.join(repositoryRoot, "docs", "release", "portable.txt"), path.join(staging, "portable.txt"));
	fs.rmSync(directory, { recursive: true, force: true });
	fs.renameSync(staging, directory);
	const executable = path.join(directory, location.executable);
	if (platform !== "win32") fs.chmodSync(fs.realpathSync(executable), 0o755);
	if (selfContained) return;
	if (platform === "win32") return startMenuShortcut(PRODUCT_NAMES[product], executable);
	writeDesktopEntry(product, location, executable);
}

function startMenuShortcut(name, executable) {
	const folder = path.join(process.env.APPDATA ?? "", "Microsoft", "Windows", "Start Menu", "Programs", "ToskLight");
	fs.mkdirSync(folder, { recursive: true });
	const link = path.join(folder, `${name}.lnk`).replaceAll("'", "''");
	const script = `$s=(New-Object -ComObject WScript.Shell).CreateShortcut('${link}');$s.TargetPath='${executable.replaceAll("'", "''")}';$s.WorkingDirectory='${path.dirname(executable).replaceAll("'", "''")}';$s.Save()`;
	const result = spawnSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", script], { stdio: "inherit" });
	if (result.status !== 0) console.warn(`warning: could not create the Start menu shortcut for ${name}`);
}

function writeDesktopEntry(product, location, executable) {
	const icon = {
		control: "apps/light-desktop/src-tauri/icons/128x128.png",
		architect: "apps/viz-editor/src-tauri/icons/128x128.png",
		pixel: "assets/branding/ToskLight Pixel.png",
	}[product];
	const installedIcon = path.join(location.directory, "icon.png");
	fs.copyFileSync(path.join(repositoryRoot, icon), installedIcon);
	fs.mkdirSync(path.dirname(location.launcher), { recursive: true });
	fs.writeFileSync(
		location.launcher,
		[
			"[Desktop Entry]",
			"Type=Application",
			`Name=${PRODUCT_NAMES[product]}`,
			`Exec="${executable}"`,
			`Path=${location.directory}`,
			`Icon=${installedIcon}`,
			"Terminal=false",
			"Categories=AudioVideo;",
			"",
		].join("\n"),
	);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
	main().catch((error) => {
		console.error(`error: ${error.message}`);
		process.exit(1);
	});
