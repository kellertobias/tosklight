//! The benchmark may observe only its original owned server, never an app fallback or neighbour.
import { execFile, spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, open, realpath } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { promisify } from "node:util";

const exec = promisify(execFile);
const pause = (millis) => new Promise((resolve) => setTimeout(resolve, millis));

export async function startPackagedIsolatedServer(application, dataDirectory, port, osc, repositoryRoot) {
	const contents = application.bundle
		? path.join(application.bundle, "Contents")
		: null;
	const server = contents
		? path.join(contents, "MacOS", "light-headless")
		: path.join(
				path.dirname(application.executable),
				process.platform === "win32" ? "light-headless.exe" : "light-headless",
			);
	const fixtureLibrary = contents
		? path.join(contents, "Resources", "fixture-library")
		: path.join(repositoryRoot, "assets", "fixture-library");
	const logPath = path.join(dataDirectory, "owned-light-headless.log");
	const owned = await startOwnedBenchmarkServer({
		port,
		executable: server,
		spawnServer: async () => {
			await mkdir(dataDirectory, { recursive: true });
			const log = await open(logPath, "wx");
			let child;
			try {
				// No env override: preserve the caller's exact diagnostic and scheduling flags.
				child = spawn(server, [
					"--data-dir", dataDirectory,
					"--fixture-package-dir", fixtureLibrary,
					"--bind", `127.0.0.1:${port}`,
					"--osc-bind", `127.0.0.1:${osc}`,
				], { cwd: repositoryRoot, stdio: ["ignore", log.fd, log.fd] });
			} catch (error) {
				await log.close();
				throw error;
			}
			child.once("exit", () => void log.close().catch(() => undefined));
			child.once("error", () => void log.close().catch(() => undefined));
			return child;
		},
	});
	owned.identity.logPath = logPath;
	owned.identity.diagnosticEnvironment = {
		LIGHT_OUTPUT_PHASE_DIAGNOSTICS: process.env.LIGHT_OUTPUT_PHASE_DIAGNOSTICS ?? null,
		RUST_LOG: process.env.RUST_LOG ?? null,
	};
	return owned;
}

export async function requireFreeBenchmarkPort(port) {
	await new Promise((resolve, reject) => {
		const socket = net.createServer();
		socket.once("error", (error) => reject(new Error(`Isolated benchmark port ${port} is unavailable: ${error.message}`)));
		socket.listen({ host: "127.0.0.1", port, exclusive: true }, () => socket.close(resolve));
	});
}

export async function benchmarkListenerPids(port, { platform = process.platform, run = exec } = {}) {
	if (!Number.isSafeInteger(port) || port < 1 || port > 65535) throw new Error("Invalid benchmark listener port");
	let command;
	let args;
	if (platform === "darwin") {
		command = "/usr/sbin/lsof";
		args = ["-nP", "-t", `-iTCP:${port}`, "-sTCP:LISTEN"];
	} else if (platform === "linux") {
		command = "ss";
		args = ["-ltnpH", "sport", "=", `:${port}`];
	} else if (platform === "win32") {
		command = "powershell.exe";
		args = ["-NoProfile", "-NonInteractive", "-Command",
			`Get-NetTCPConnection -State Listen -LocalPort ${port} -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess`];
	} else throw new Error(`Listener attribution unsupported on ${platform}`);
	let stdout;
	try {
		({ stdout } = await run(command, args, { timeout: 2_000, maxBuffer: 256 * 1024 }));
	} catch (error) {
		if (platform === "darwin" && error.code === 1 && !error.stdout?.trim() && !error.stderr?.trim()) return [];
		throw new Error(`Cannot verify isolated listener ownership using ${command}: ${error.message}`);
	}
	const values = platform === "linux"
		? [...stdout.matchAll(/pid=(\d+)/g)].map((match) => Number(match[1]))
		: stdout.trim().split(/\s+/).filter(Boolean).map(Number);
	if (values.some((pid) => !Number.isSafeInteger(pid) || pid <= 1)) throw new Error("Invalid listener PID evidence");
	if (platform === "linux" && stdout.trim() && values.length === 0) throw new Error("Listener exists but ss did not expose its PID");
	return [...new Set(values)];
}

async function executableIdentity(executable) {
	const resolved = await realpath(executable);
	const hash = createHash("sha256");
	for await (const chunk of createReadStream(resolved)) hash.update(chunk);
	return { executable: resolved, sha256: hash.digest("hex") };
}

export async function stopOwnedBenchmarkChild(child) {
	if (child.exitCode !== null || child.signalCode !== null) return;
	const exited = new Promise((resolve) => child.once("exit", resolve));
	const wait = async (millis) => {
		let timer;
		const result = await Promise.race([exited.then(() => true), new Promise((resolve) => { timer = setTimeout(() => resolve(false), millis); })]);
		clearTimeout(timer);
		return result;
	};
	child.kill("SIGTERM");
	if (await wait(3_000)) return;
	child.kill("SIGKILL");
	if (!(await wait(2_000))) throw new Error(`Owned benchmark server ${child.pid} did not exit after SIGKILL`);
}

export async function startOwnedBenchmarkServer(options, dependencies = {}) {
	const { port, executable, spawnServer, timeoutMillis = 60_000 } = options;
	const freePort = dependencies.freePort ?? requireFreeBenchmarkPort;
	const identify = dependencies.identify ?? executableIdentity;
	const listeners = dependencies.listeners ?? benchmarkListenerPids;
	const stop = dependencies.stop ?? stopOwnedBenchmarkChild;
	const now = dependencies.now ?? Date.now;
	const sleep = dependencies.sleep ?? pause;
	const readiness = dependencies.readiness ?? (async () => {
		const response = await fetch(`http://127.0.0.1:${port}/api/v2/readiness`, { signal: AbortSignal.timeout(1_000) }).catch(() => null);
		await response?.body?.cancel();
		return response?.ok === true;
	});
	await freePort(port);
	const binary = await identify(executable);
	const startedAtEpochMillis = now();
	const child = await spawnServer();
	let spawnError;
	child.on("error", (error) => { spawnError = error; });
	const alive = () => {
		if (spawnError || !Number.isSafeInteger(child.pid) || child.exitCode !== null || child.signalCode !== null)
			throw new Error(`Original isolated benchmark server exited or failed: ${spawnError?.message ?? child.exitCode ?? child.signalCode}`);
	};
	const verify = async () => {
		alive();
		const pids = await listeners(port);
		alive();
		if (pids.length !== 1 || pids[0] !== child.pid)
			throw new Error(`Isolated listener ownership mismatch: expected ${child.pid}, observed ${pids.join(",") || "none"}`);
		return { pid: child.pid, listenerPid: pids[0], ...binary, verifiedAtEpochMillis: now() };
	};
	try {
		const deadline = now() + timeoutMillis;
		while (now() < deadline) {
			alive();
			if (await readiness()) {
				const begin = await verify();
				return { child, identity: { startedAtEpochMillis, begin }, verify: async () => {
					const end = await verify();
					const after = await identify(executable);
					alive();
					if (after.executable !== binary.executable || after.sha256 !== binary.sha256)
						throw new Error("Isolated server executable changed during benchmark");
					return end;
				} };
			}
			alive();
			await sleep(100);
		}
		throw new Error(`Original isolated benchmark server did not become ready within ${timeoutMillis}ms`);
	} catch (error) {
		await stop(child).catch((cleanup) => { error.message += `; cleanup failed: ${cleanup.message}`; });
		throw error;
	}
}
