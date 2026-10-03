import { spawn, type ChildProcess } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import readline from "node:readline";
import artifactResolver from "../../../tools/artifact-paths.cjs";

const { artifactPaths } = artifactResolver;

/**
 * The headless Architect (`viz-sync-harness`): one planning document running the Architect's own
 * `SyncEngine`, driven over a loopback JSON API. It stands in for the Architect window in the
 * Control ↔ Architect show-sync scenarios, so they need no display.
 */
const HARNESS =
	process.env.LIGHT_E2E_ARCHITECT_HARNESS ??
	path.join(
		artifactPaths.cargo,
		"debug",
		process.platform === "win32" ? "viz-sync-harness.exe" : "viz-sync-harness",
	);

/** Every harness still running, so a worker that ends mid-test leaves none behind. */
const running = new Set<ChildProcess>();
process.once("exit", () => {
	for (const child of running) child.kill("SIGKILL");
});

export type SyncPhase = "synced" | "pending" | "offline" | "conflict" | "error";

export interface SyncStatus {
	state: SyncPhase;
	label: string;
	detail: string;
	pending: number;
	conflicts: number;
	savedToControl: boolean;
	savedOnThisComputer: boolean;
}

export interface HarnessStatus {
	bound: boolean;
	status?: SyncStatus;
	notice?: string | null;
	binding?: { association_id: string; show_id: string; desk_identity: string | null };
	remote_changes?: number;
	snapshot_reads?: number;
}

export interface ConflictView {
	entry: number;
	kind: string;
	id: string;
	path: string;
	base: unknown;
	mine: unknown;
	theirs: unknown;
	reason: string;
	label: string;
}

export type ArchitectEdit =
	| { put: { kind: string; id: string; body: unknown } }
	| { delete: { kind: string; id: string } }
	| { move_fixture: { fixture_id: string; x: number; y: number; z: number } }
	| { metadata: { key: string; value: string } };

export class ArchitectHarness {
	private constructor(
		private readonly process: ChildProcess,
		readonly baseUrl: string,
		readonly dataDir: string,
		private readonly log: string[],
	) {}

	/** Starts a harness over `dataDir`, the Architect's app-data directory. */
	static async start(dataDir?: string): Promise<ArchitectHarness> {
		await fs.mkdir(artifactPaths.tmp, { recursive: true });
		const directory =
			dataDir ?? (await fs.mkdtemp(path.join(artifactPaths.tmp, "architect-sync-")));
		const child = spawn(HARNESS, ["--data-dir", directory], {
			stdio: ["ignore", "pipe", "pipe"],
			env: { ...process.env, VIZ_SYNC_TRACE: "1" },
		});
		running.add(child);
		child.once("exit", () => running.delete(child));
		const log: string[] = [];
		child.stderr?.on("data", (chunk) => log.push(String(chunk)));
		const lines = readline.createInterface({ input: child.stdout! });
		const baseUrl = await new Promise<string>((resolve, reject) => {
			const timer = setTimeout(
				() => reject(new Error(`viz-sync-harness did not start: ${log.join("")}`)),
				15_000,
			);
			child.once("exit", (code) => {
				clearTimeout(timer);
				reject(new Error(`viz-sync-harness exited with ${code}: ${log.join("")}`));
			});
			lines.on("line", (line) => {
				log.push(line);
				const match = /^LISTENING (\S+)$/.exec(line.trim());
				if (match) {
					clearTimeout(timer);
					resolve(match[1]);
				}
			});
		});
		return new ArchitectHarness(child, baseUrl, directory, log);
	}

	async request<T>(method: string, route: string, body?: unknown): Promise<T> {
		const response = await fetch(`${this.baseUrl}${route}`, {
			method,
			headers: body === undefined ? {} : { "content-type": "application/json" },
			body: body === undefined ? undefined : JSON.stringify(body),
			signal: AbortSignal.timeout(30_000),
		});
		const text = await response.text();
		if (!response.ok) throw new Error(`${method} ${route} failed ${response.status}: ${text}`);
		return (text ? JSON.parse(text) : undefined) as T;
	}

	openFromDesk(deskUrl: string, showId: string, documentPath: string) {
		return this.request<{ bound: boolean }>("POST", "/open-from-desk", {
			base_url: deskUrl,
			show_id: showId,
			path: documentPath,
			desk_name: "Test Control",
		});
	}

	open(documentPath: string) {
		return this.request<{ bound: boolean; notice: string | null }>("POST", "/open", {
			path: documentPath,
		});
	}

	create(documentPath: string, name: string) {
		return this.request<{ bound: boolean }>("POST", "/create", { path: documentPath, name });
	}

	gesture(...edits: ArchitectEdit[]) {
		return this.request<{ ok: true }>("POST", "/gesture", { edits });
	}

	status() {
		return this.request<HarnessStatus>("GET", "/status");
	}

	async phase(): Promise<SyncPhase | "unbound"> {
		const status = await this.status();
		return status.status?.state ?? "unbound";
	}

	conflicts() {
		return this.request<ConflictView[]>("GET", "/conflicts");
	}

	resolve(entry: number, resolution: "keep_control" | "use_mine") {
		return this.request("POST", "/resolve", { entry, resolution });
	}

	objects<T = Record<string, unknown>>(kind: string) {
		return this.request<Array<{ id: string; body: T }>>("GET", `/objects/${kind}`);
	}

	fixtures() {
		return this.request<Array<Record<string, any>>>("GET", "/fixtures");
	}

	document() {
		return this.request<{ show_id: string; name: string; path: string; metadata: Record<string, string> }>(
			"GET",
			"/document",
		);
	}

	setOnline(online: boolean) {
		return this.request("POST", "/online", { online });
	}

	loseNextReply() {
		return this.request("POST", "/lose-next-reply");
	}

	dismissError() {
		return this.request("POST", "/dismiss-error");
	}

	saveAs(documentPath: string) {
		return this.request<{ name: string; opened: { bound: boolean } }>("POST", "/save-as", {
			path: documentPath,
		});
	}

	/** Ends the process at once, as a crash or a quit would: nothing is flushed on the way out. */
	async kill(): Promise<void> {
		if (this.process.exitCode !== null || this.process.signalCode !== null) return;
		const exited = new Promise<void>((resolve) => this.process.once("exit", () => resolve()));
		this.process.kill("SIGKILL");
		await exited;
	}

	output(): string {
		return this.log.join("\n");
	}
}
