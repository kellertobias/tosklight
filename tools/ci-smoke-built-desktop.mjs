#!/usr/bin/env node

import { execFile, spawn } from "node:child_process";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { artifactPaths } from "./artifact-paths.mjs";

if (process.env.GITHUB_ACTIONS !== "true") {
	throw new Error("The packaged desktop launch probe is CI-only.");
}

const target = process.env.LIGHT_DESKTOP_SMOKE_TARGET;
if (!target) throw new Error("LIGHT_DESKTOP_SMOKE_TARGET is required.");

const executable =
	process.platform === "darwin"
		? path.join(
				artifactPaths.cargo,
				target,
				"release/bundle/macos/ToskLight.app/Contents/MacOS/light-desktop",
			)
		: path.join(
				artifactPaths.cargo,
				target,
				"release",
				process.platform === "win32" ? "light-desktop.exe" : "light-desktop",
			);

await fs.access(executable);
const dataDirectory = await fs.mkdtemp(
	path.join(os.tmpdir(), "tosklight-ci-launch-"),
);
const port = await freePort();
const output = [];
const child = spawn(executable, [], {
	cwd: path.resolve(import.meta.dirname, ".."),
	detached: process.platform !== "win32",
	env: {
		...process.env,
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
	const earlyExit = await Promise.race([
		new Promise((resolve) =>
			child.once("exit", (code, signal) => resolve({ code, signal })),
		),
		new Promise((resolve) =>
			child.once("error", (error) => resolve({ error: error.message })),
		),
		new Promise((resolve) => setTimeout(() => resolve(undefined), 5_000)),
	]);
	if (earlyExit) {
		throw new Error(
			`ToskLight exited during its five-second launch probe (${JSON.stringify(earlyExit)}).`,
		);
	}
	console.log(
		`ToskLight stayed alive for five seconds on ${process.platform}.`,
	);
} catch (error) {
	const detail = output.join("").trim();
	throw new Error(
		`${error instanceof Error ? error.message : String(error)}${detail ? `\nRecent application output:\n${detail}` : ""}`,
	);
} finally {
	await terminateProcessTree(child.pid);
	await fs.rm(dataDirectory, { recursive: true, force: true });
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
