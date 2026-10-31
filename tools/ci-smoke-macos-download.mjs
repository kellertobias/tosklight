#!/usr/bin/env node
// Verify and run the applications extracted from the supplied ZIP, never checkout executables.
import { execFile, spawn } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import dgram from "node:dgram";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const execute = promisify(execFile);
const PRODUCTS = new Map([
	["de.tokenet.light", "control"],
	["de.tokenet.tosklight.visualizer", "architect"],
	["de.tokenet.tosklight.media", "pixel"],
]);

/** Reject path traversal and archives that write through a previously extracted symlink. */
export function validateArchiveEntries(entries) {
	const links = entries.filter((entry) => entry.linkTarget != null);
	for (const entry of entries) {
		if (
			!entry.path ||
			entry.path.includes("\\") ||
			entry.path.includes("\0") ||
			entry.path.startsWith("/") ||
			/^[A-Za-z]:/u.test(entry.path) ||
			entry.path.split("/").includes("..")
		)
			throw new Error(`Unsafe ZIP entry: ${JSON.stringify(entry.path)}`);
		if (entry.linkTarget != null) {
			const destination = path.posix.resolve(
				"/archive",
				path.posix.dirname(entry.path),
				entry.linkTarget,
			);
			if (
				!destination.startsWith("/archive/") ||
				path.posix.isAbsolute(entry.linkTarget)
			)
				throw new Error(
					`ZIP symlink leaves its extraction folder: ${entry.path}`,
				);
		}
		if (
			links.some((link) =>
				entry.path.startsWith(`${link.path.replace(/\/$/u, "")}/`),
			)
		)
			throw new Error(`ZIP entry writes through a symlink: ${entry.path}`);
	}
}

/** System libraries and bundle-relative install names are the only portable dependencies. */
export function nonSystemDependencies(output) {
	return output.split("\n").flatMap((line) => {
		const dependency = /^\s+(.+?)\s+\(compatibility version /u.exec(line)?.[1];
		return dependency?.startsWith("/") &&
			!dependency.startsWith("/System/Library/") &&
			!dependency.startsWith("/usr/lib/")
			? [dependency]
			: [];
	});
}

async function run(command, args, options = {}) {
	return execute(command, args, {
		cwd: repositoryRoot,
		maxBuffer: 16 * 1024 * 1024,
		timeout: 120_000,
		...options,
	});
}

export async function extractArchive(archive, directory) {
	// Python reads ZIP metadata without executing its content; ditto then preserves macOS metadata.
	const inspected = await run("python3", [
		"-c",
		[
			"import json,stat,sys,zipfile",
			"with zipfile.ZipFile(sys.argv[1]) as z:",
			" print(json.dumps([{'path':i.filename, **({'linkTarget':z.read(i).decode('utf-8')} if stat.S_ISLNK(i.external_attr>>16) else {})} for i in z.infolist()]))",
		].join("\n"),
		archive,
	]);
	validateArchiveEntries(JSON.parse(inspected.stdout));
	await fs.mkdir(directory, { recursive: true });
	await run("/usr/bin/ditto", ["-x", "-k", archive, directory]);
}

export async function findApplications(directory) {
	const found = new Map();
	async function visit(folder) {
		for (const entry of await fs.readdir(folder, { withFileTypes: true })) {
			if (!entry.isDirectory() || entry.name === "__MACOSX") continue;
			const location = path.join(folder, entry.name);
			if (!entry.name.endsWith(".app")) {
				await visit(location);
				continue;
			}
			const plist = path.join(location, "Contents/Info.plist");
			const read = async (key) =>
				(
					await run("/usr/libexec/PlistBuddy", ["-c", `Print :${key}`, plist])
				).stdout.trim();
			const id = await read("CFBundleIdentifier");
			const product = PRODUCTS.get(id);
			if (!product)
				throw new Error(
					`Unexpected application in suite ZIP: ${location} (${id})`,
				);
			if (found.has(product))
				throw new Error(`ZIP contains more than one ${product} application`);
			const name = await read("CFBundleExecutable");
			if (name !== path.basename(name))
				throw new Error(`Invalid bundle executable: ${name}`);
			found.set(product, {
				id,
				app: location,
				executable: path.join(location, "Contents/MacOS", name),
			});
		}
	}
	await visit(directory);
	for (const product of PRODUCTS.values())
		if (!found.has(product))
			throw new Error(`ZIP is missing the ${product} application`);
	return found;
}

async function machoFiles(app) {
	const boundary = await fs.realpath(app);
	const files = [],
		visited = new Set();
	async function visit(file) {
		const real = await fs.realpath(file);
		if (real !== boundary && !real.startsWith(`${boundary}${path.sep}`))
			throw new Error(`Bundle symlink leaves the application: ${file}`);
		if (visited.has(real)) return;
		visited.add(real);
		if ((await fs.stat(real)).isDirectory()) {
			for (const entry of await fs.readdir(real))
				await visit(path.join(real, entry));
			return;
		}
		const handle = await fs.open(real);
		const magic = Buffer.alloc(4);
		try {
			await handle.read(magic, 0, 4, 0);
		} finally {
			await handle.close();
		}
		if (
			[
				"feedface",
				"feedfacf",
				"cefaedfe",
				"cffaedfe",
				"cafebabe",
				"bebafeca",
				"cafebabf",
				"bfbafeca",
			].includes(magic.toString("hex"))
		)
			files.push(real);
	}
	await visit(app);
	return files;
}

async function verifyMachO(file, mode, host, expectedTeam) {
	await run("/usr/bin/codesign", ["--verify", "--deep", "--strict", file]);
	const architectures = (await run("/usr/bin/lipo", ["-archs", file])).stdout
		.trim()
		.split(/\s+/u);
	if (!architectures.includes(host))
		throw new Error(
			`${file} cannot run on ${host}: ${architectures.join(", ")}`,
		);
	const external = nonSystemDependencies(
		(await run("/usr/bin/otool", ["-L", file])).stdout,
	);
	if (external.length)
		throw new Error(
			`${file} depends on unbundled libraries: ${external.join(", ")}`,
		);
	const signature = await run("/usr/bin/codesign", [
		"--display",
		"--verbose=4",
		file,
	]);
	const metadata = signature.stderr + signature.stdout;
	const team = /^TeamIdentifier=(.+)$/mu.exec(metadata)?.[1];
	if (
		mode === "developer-id" &&
		(!/^Authority=Developer ID Application:/mu.test(metadata) ||
			!team ||
			team === "not set")
	)
		throw new Error(
			`${file} is not signed with a Developer ID Application identity`,
		);
	if (mode === "developer-id" && expectedTeam && team !== expectedTeam)
		throw new Error(
			`${file} has Developer ID team ${team}, expected ${expectedTeam}`,
		);
	return { file, architectures, team: team ?? null };
}

async function verifyApplication(application, mode, host, expectedTeam) {
	await run("/usr/bin/codesign", [
		"--verify",
		"--deep",
		"--strict",
		application.app,
	]);
	const binaries = await machoFiles(application.app);
	if (!binaries.includes(await fs.realpath(application.executable)))
		throw new Error(`No Mach-O bundle executable: ${application.executable}`);
	const verified = [];
	for (const file of binaries) {
		const verdict = await verifyMachO(file, mode, host, expectedTeam);
		if (mode === "developer-id") expectedTeam ??= verdict.team;
		verified.push(verdict);
	}
	if (mode === "developer-id") {
		await run("/usr/bin/xcrun", ["stapler", "validate", application.app]);
		await run("/usr/sbin/spctl", [
			"--assess",
			"--type",
			"execute",
			"--verbose=2",
			application.app,
		]);
	}
	return {
		...application,
		binaries: verified,
		architecturesInclude: host,
		team: expectedTeam ?? null,
	};
}

function isolatedEnvironment() {
	const env = { ...process.env };
	for (const key of Object.keys(env))
		if (
			/^(LIGHT_DESKTOP_TEST_|LIGHT_(DESKTOP|MEDIA)_SMOKE_|TOSKLIGHT_VIZ_)/u.test(
				key,
			) ||
			key === "MEDIA_CONFIG"
		)
			delete env[key];
	return env;
}

async function processSmoke(executable, args, env, log) {
	const child = spawn(executable, args, {
		cwd: path.dirname(log),
		env,
		detached: true,
		stdio: ["ignore", "pipe", "pipe"],
	});
	const output = [];
	let bytes = 0;
	const append = (chunk) => {
		output.push(chunk);
		bytes += chunk.length;
		while (bytes > 8 * 1024 * 1024) bytes -= output.shift().length;
	};
	child.stdout.on("data", append);
	child.stderr.on("data", append);
	let timer;
	try {
		await new Promise((resolve, reject) => {
			child.once("error", reject);
			child.once("exit", (code, signal) =>
				code === 0
					? resolve()
					: reject(new Error(`${executable} exited with ${code ?? signal}`)),
			);
			timer = setTimeout(
				() =>
					reject(new Error(`${executable} did not finish within 150 seconds`)),
				150_000,
			);
		});
	} finally {
		clearTimeout(timer);
		try {
			process.kill(-child.pid, "SIGTERM");
		} catch {}
		await new Promise((resolve) => setTimeout(resolve, 200));
		try {
			process.kill(-child.pid, "SIGKILL");
		} catch {}
		await fs.writeFile(log, Buffer.concat(output));
	}
	return Buffer.concat(output).toString();
}

async function freePort(udp = false) {
	const socket = udp ? dgram.createSocket("udp4") : net.createServer();
	await new Promise((resolve, reject) => {
		socket.once("error", reject);
		udp
			? socket.bind(0, "127.0.0.1", resolve)
			: socket.listen(0, "127.0.0.1", resolve);
	});
	const port = socket.address().port;
	await new Promise((resolve) => socket.close(resolve));
	return port;
}

async function smokeApplications(apps, directory) {
	const env = isolatedEnvironment();
	await processSmoke(
		process.execPath,
		[path.join(repositoryRoot, "tools/ci-smoke-built-desktop.mjs")],
		{
			...env,
			LIGHT_DESKTOP_SMOKE_EXECUTABLE: apps.get("control").executable,
			LIGHT_DESKTOP_TEST_OSC_BIND: `127.0.0.1:${await freePort(true)}`,
		},
		path.join(directory, "control.log"),
	);
	const architect = apps.get("architect");
	const data = path.join(directory, "architect");
	await fs.mkdir(data);
	const show = path.join(data, "demo.show"),
		preferences = path.join(data, "renderer.conf");
	await fs.copyFile(
		path.join(architect.app, "Contents/Resources/demo-show/demo-show.show"),
		show,
	);
	await fs.writeFile(
		preferences,
		"quality standard\nart_net_interface lo0\nsacn_interface lo0\n",
	);
	const cleanRunner =
		env.GITHUB_ACTIONS === "true" && env.RUNNER_ENVIRONMENT === "github-hosted";
	const freshPaths = [
		"de.tokenet.tosklight.visualizer",
		"de.tokenet.tosklight.viz-editor",
	].map((identifier) =>
		path.join(os.homedir(), "Library/Application Support", identifier),
	);
	if (cleanRunner) {
		// GitHub's virtual display is not built in. Only its disposable, previously unused
		// account may use ordinary startup; a local user's recent shows are never consulted.
		for (const candidate of freshPaths)
			if (
				await fs.stat(candidate).then(
					() => true,
					(error) => (error.code === "ENOENT" ? false : Promise.reject(error)),
				)
			)
				throw new Error(
					`Architect smoke requires an unused GitHub-hosted account; state already exists: ${candidate}`,
				);
	}
	const result = await processSmoke(
		architect.executable,
		["--verify"],
		{
			...env,
			TOSKLIGHT_VIZ_PREFERENCES: preferences,
			...(cleanRunner
				? {}
				: {
						TOSKLIGHT_VIZ_CAPTURE_DATA_DIR: data,
						TOSKLIGHT_VIZ_CAPTURE_SHOW: show,
						TOSKLIGHT_VIZ_CAPTURE_DISPLAY: "internal",
					}),
		},
		path.join(directory, "architect.log"),
	);
	const shown =
		cleanRunner ||
		JSON.parse(
			await fs.readFile(path.join(data, "capture-window.json"), "utf8"),
		).windowPlacedAndShown === true;
	if (!result.includes("The Viz editor drew its interface.") || !shown)
		throw new Error(
			"Architect did not prove its extracted frontend mounted in the isolated window",
		);
	const pixel = path.join(directory, "pixel");
	await fs.mkdir(pixel);
	const port = await freePort();
	const configuration = path.join(pixel, "media-server.json");
	await fs.writeFile(
		configuration,
		JSON.stringify(
			{
				version: 10,
				configuration: {
					instanceId: "download-smoke",
					library: { root: path.join(pixel, "library"), targetCodec: "h264" },
					network: {
						sameComputerPreset: false,
						httpListen: `127.0.0.1:${port}`,
						artNetListen: `127.0.0.1:${await freePort(true)}`,
						sacnListen: `127.0.0.1:${await freePort(true)}`,
						citpListen: `127.0.0.1:${await freePort()}`,
					},
					outputs: [
						{
							id: randomUUID(),
							name: "Download smoke",
							enabled: true,
							target: {
								kind: "monitor",
								monitor: { by: "index", value: 0 },
								fullscreen: false,
							},
							resolution: { width: 640, height: 360 },
							personality: "two-layers",
							protocol: "art-net",
							universe: 1,
							startAddress: 1,
							soundOutput: { kind: "disabled" },
						},
					],
				},
			},
			null,
			2,
		),
	);
	await processSmoke(
		process.execPath,
		[path.join(repositoryRoot, "tools/ci-smoke-media-server.mjs")],
		{
			...env,
			LIGHT_MEDIA_SMOKE_EXECUTABLE: apps.get("pixel").executable,
			LIGHT_MEDIA_SMOKE_CONFIG: configuration,
			LIGHT_MEDIA_SMOKE_PORT: String(port),
		},
		path.join(directory, "pixel.log"),
	);
	return {
		control: "private runtime directory",
		pixel: "private configuration and library",
		architect: cleanRunner
			? "unused disposable GitHub-hosted account (state paths verified absent)"
			: "private capture data, show and preferences on built-in display",
	};
}

async function main() {
	if (process.platform !== "darwin")
		throw new Error("The downloaded-app smoke test requires macOS");
	const arguments_ = process.argv.slice(2);
	const verifyOnly = arguments_.includes("--verify-only");
	const positional = arguments_.filter(
		(argument) => argument !== "--verify-only",
	);
	if (
		positional.length !== 1 ||
		positional[0].startsWith("--") ||
		arguments_.length !== (verifyOnly ? 2 : 1)
	)
		throw new Error(
			"Usage: node tools/ci-smoke-macos-download.mjs ARCHIVE.zip [--verify-only]",
		);
	const archive = path.resolve(positional[0]);
	const mode = process.env.LIGHT_MACOS_SIGNING_MODE ?? "adhoc";
	if (!["adhoc", "developer-id"].includes(mode))
		throw new Error(`Unknown LIGHT_MACOS_SIGNING_MODE: ${mode}`);
	await fs.mkdir(artifactPaths.tmp, { recursive: true });
	const directory = await fs.mkdtemp(
		path.join(artifactPaths.tmp, "macos-download-smoke-"),
	);
	const hash = createHash("sha256");
	for await (const chunk of createReadStream(archive)) hash.update(chunk);
	const report = {
		archive,
		archiveSha256: hash.digest("hex"),
		mode,
		quarantineRemoved: false,
		runtime: verifyOnly ? "not-run" : "pending",
		passed: false,
	};
	try {
		await extractArchive(archive, path.join(directory, "extracted"));
		const apps = await findApplications(path.join(directory, "extracted"));
		const host =
			(
				await run("/usr/sbin/sysctl", ["-n", "hw.optional.arm64"])
			).stdout.trim() === "1"
				? "arm64"
				: "x86_64";
		let team = process.env.LIGHT_MACOS_TEAM_ID;
		report.applications = [];
		for (const application of apps.values()) {
			const verified = await verifyApplication(application, mode, host, team);
			team ??= verified.team;
			report.applications.push(verified);
		}
		const packaged = new Set(
			report.applications.flatMap((app) =>
				app.binaries.map((binary) => binary.file),
			),
		);
		report.standaloneBinaries = [];
		for (const file of await machoFiles(path.join(directory, "extracted")))
			if (!packaged.has(file))
				report.standaloneBinaries.push(
					await verifyMachO(file, mode, host, team),
				);
		if (!verifyOnly) {
			report.runtimeIsolation = await smokeApplications(apps, directory);
			report.runtime = "passed";
		}
		report.passed = true;
		report.gatekeeperAssessmentPassed = mode === "developer-id";
		console.log(
			`Downloaded suite signatures and dependencies${verifyOnly ? " passed; runtime was not run" : " and isolated startup passed"}. Evidence: ${directory}`,
		);
		if (mode === "adhoc")
			console.log(
				"Ad-hoc signature and startup checks do not establish Gatekeeper approval for downloaded applications.",
			);
	} catch (error) {
		report.error = error.message;
		throw error;
	} finally {
		await fs.writeFile(
			path.join(directory, "report.json"),
			`${JSON.stringify(report, null, 2)}\n`,
		);
		console.log(`Download smoke evidence: ${directory}`);
	}
}

if (
	process.argv[1] &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
	main().catch((error) => {
		console.error(error.message);
		process.exitCode = 1;
	});
