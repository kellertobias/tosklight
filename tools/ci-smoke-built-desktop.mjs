#!/usr/bin/env node

import { execFile, spawn } from "node:child_process";
import fs from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { promisify } from "node:util";
import { artifactPaths } from "./artifact-paths.mjs";

const target = process.env.LIGHT_DESKTOP_SMOKE_TARGET;
if (process.env.GITHUB_ACTIONS === "true" && !target)
	throw new Error("LIGHT_DESKTOP_SMOKE_TARGET is required in CI.");
const buildDirectory = target
	? path.join(artifactPaths.cargo, target, "release")
	: path.join(artifactPaths.cargo, "debug");

const executable = await packagedExecutable(buildDirectory);

await fs.access(executable);
const executableDirectory = path.dirname(executable);
const executableSuffix = process.platform === "win32" ? ".exe" : "";
if (process.platform !== "linux") {
	for (const helper of ["light-headless", "viz-renderer"]) {
		await fs.access(
			path.join(executableDirectory, `${helper}${executableSuffix}`),
		);
	}
}
await fs.mkdir(artifactPaths.tmp, { recursive: true });
const dataDirectory = await fs.mkdtemp(
	path.join(artifactPaths.tmp, "tosklight-ci-launch-"),
);
const port = await freePort();
const output = [];
const child = spawn(executable, [], {
	cwd: path.resolve(import.meta.dirname, ".."),
	detached: process.platform !== "win32",
	env: {
		...process.env,
		...(process.platform === "linux" ? { APPIMAGE_EXTRACT_AND_RUN: "1" } : {}),
		LIGHT_DESKTOP_TEST_BIND: `127.0.0.1:${port}`,
		LIGHT_DESKTOP_TEST_DATA_DIR: dataDirectory,
	},
	stdio: ["ignore", "pipe", "pipe"],
});

for (const stream of [child.stdout, child.stderr]) {
	stream.on("data", (chunk) => {
		output.push(chunk.toString());
		if (output.length > 200) output.shift();
	});
}

try {
	await waitForReadiness(child, port);
	console.log(
		`Packaged ToskLight reached healthy readiness on ${process.platform}.`,
	);
} catch (error) {
	const detail = output.join("").trim();
	const serverLog = await fs
		.readFile(path.join(dataDirectory, "light-headless.log"), "utf8")
		.catch(() => "");
	throw new Error(
		`${error instanceof Error ? error.message : String(error)}${detail ? `\nRecent application output:\n${detail}` : ""}${serverLog.trim() ? `\nBundled Light server log:\n${serverLog.trim()}` : ""}`,
	);
} finally {
	await terminateProcessTree(child.pid);
	await fs.rm(dataDirectory, { recursive: true, force: true });
}

async function packagedExecutable(buildDirectory) {
	if (process.platform === "darwin") {
		return path.join(
			buildDirectory,
			"bundle/macos/ToskLight.app/Contents/MacOS/light-desktop",
		);
	}
	if (process.platform === "win32") {
		return path.join(buildDirectory, "light-desktop.exe");
	}
	const appImages = (
		await fs.readdir(path.join(buildDirectory, "bundle/appimage"))
	).filter((entry) => entry.endsWith(".AppImage"));
	if (appImages.length !== 1) {
		throw new Error(
			`Expected exactly one packaged AppImage, found ${appImages.length}.`,
		);
	}
	return path.join(buildDirectory, "bundle/appimage", appImages[0]);
}

async function waitForReadiness(child, port) {
	const deadline = Date.now() + 60_000;
	while (Date.now() < deadline) {
		if (child.exitCode !== null || child.signalCode !== null) {
			throw new Error(
				`ToskLight exited before readiness (${JSON.stringify({ code: child.exitCode, signal: child.signalCode })}).`,
			);
		}
		try {
			const response = await fetch(
				`http://127.0.0.1:${port}/api/v2/readiness`,
				{ signal: AbortSignal.timeout(2_000) },
			);
			if (response.ok) {
				const readiness = await response.json();
				if (readiness.status === "ready" && readiness.recovery_mode === false)
					return;
			}
		} catch {}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error(
		"ToskLight did not reach healthy readiness within 60 seconds.",
	);
}

async function freePort() {
	const listener = net.createServer();
	await new Promise((resolve, reject) => {
		listener.once("error", reject);
		listener.listen(0, "127.0.0.1", resolve);
	});
	const address = listener.address();
	if (!address || typeof address === "string") {
		throw new Error("Unable to allocate a loopback port.");
	}
	await new Promise((resolve) => listener.close(resolve));
	return address.port;
}

async function terminateProcessTree(pid) {
	if (!pid) return;
	if (process.platform === "win32") {
		await promisify(execFile)("taskkill", [
			"/PID",
			String(pid),
			"/T",
			"/F",
		]).catch(() => undefined);
		return;
	}
	try {
		process.kill(-pid, "SIGTERM");
	} catch {}
	await new Promise((resolve) => setTimeout(resolve, 500));
	try {
		process.kill(-pid, "SIGKILL");
	} catch {}
}
