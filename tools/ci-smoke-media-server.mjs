#!/usr/bin/env node

import { execFile, spawn } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { promisify } from "node:util";
import { artifactPaths } from "./artifact-paths.mjs";

const target = process.env.LIGHT_MEDIA_SMOKE_TARGET;
const healthPort = Number(process.env.LIGHT_MEDIA_SMOKE_PORT ?? "8080");
const configuration = process.env.LIGHT_MEDIA_SMOKE_CONFIG;
if (process.env.GITHUB_ACTIONS === "true" && !target)
	throw new Error("LIGHT_MEDIA_SMOKE_TARGET is required in CI.");

const suffix = process.platform === "win32" ? ".exe" : "";
const buildDirectory = target
	? path.join(artifactPaths.cargo, target, "release")
	: path.join(artifactPaths.cargo, "release");
const executable = path.join(buildDirectory, `media-server${suffix}`);
await fs.access(executable);
await fs.mkdir(artifactPaths.tmp, { recursive: true });
const runtimeDirectory = await fs.mkdtemp(
	path.join(artifactPaths.tmp, "tosklight-media-ci-launch-"),
);
const output = [];
const { MEDIA_CONFIG: _ignoredConfiguration, ...environment } = process.env;
const child = spawn(executable, [], {
	cwd: runtimeDirectory,
	detached: process.platform !== "win32",
	env: {
		...environment,
		...(configuration ? { MEDIA_CONFIG: configuration } : {}),
		MEDIA_LOG: "info",
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
	await waitForHealth(child);
	console.log(`ToskLight Media reached healthy startup on ${process.platform}.`);
} catch (error) {
	const detail = output.join("").trim();
	throw new Error(
		`${error instanceof Error ? error.message : String(error)}${detail ? `\nRecent Media Server output:\n${detail}` : ""}`,
	);
} finally {
	await terminateProcessTree(child.pid);
	await removeRuntimeDirectory(runtimeDirectory);
}

/**
 * Windows reports a process killed before it has let go of its working directory, so removing it
 * can fail as busy for a moment. The directory is scratch either way: a leftover must not turn a
 * healthy startup into a failed smoke test.
 */
async function removeRuntimeDirectory(directory) {
	for (let attempt = 0; attempt < 10; attempt += 1) {
		try {
			await fs.rm(directory, { recursive: true, force: true });
			return;
		} catch (error) {
			if (attempt === 9) {
				console.warn(`Could not remove ${directory}: ${error.message}`);
				return;
			}
			await new Promise((resolve) => setTimeout(resolve, 200));
		}
	}
}

async function waitForHealth(child) {
	const deadline = Date.now() + 60_000;
	while (Date.now() < deadline) {
		if (child.exitCode !== null || child.signalCode !== null) {
			throw new Error(
				`Media Server exited before health (${JSON.stringify({ code: child.exitCode, signal: child.signalCode })}).`,
			);
		}
		try {
			const healthResponse = await fetch(
				`http://127.0.0.1:${healthPort}/api/v2/health`,
				{ signal: AbortSignal.timeout(2_000) },
			);
			const health = healthResponse.ok ? await healthResponse.json() : null;
			const interfaceResponse = await fetch(`http://127.0.0.1:${healthPort}/`, {
				signal: AbortSignal.timeout(2_000),
			});
			if (
				health?.status === "ok" &&
				health.outputs > 0 &&
				interfaceResponse.ok
			)
				return;
		} catch {}
		await new Promise((resolve) => setTimeout(resolve, 250));
	}
	throw new Error("Media Server did not reach healthy startup within 60 seconds.");
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
