import { readFile, readlink } from "node:fs/promises";
import path from "node:path";
import { parseLinuxProcessStat } from "./process-tree-resources.mjs";

// Reason0 none;1 server count;2 ancestry;3 executable;4 stat;5 stable identity;6 observation I/O.
export class CpuProcessObserver {
  constructor(io = {}) {
	  this.requests = [];
		this.receipts = [];
		this.bindings = new Map();
		this.cleanupBirths = new Map();
		this.observerWallNs = 0;
		this.observerFailureReason = 0;
		this.readStat = io.readStat ?? (filename => readFile(filename, "utf8"));
		this.readlink = io.readlink ?? readlink;
	}

	require(condition, reason, message) {
	  if (!condition) {
		  this.observerFailureReason ||= reason;
			throw new Error(`Typed CPU diagnostic STOP: ${message}`);
		}
	}

	uint(value) {
	  this.require(Number.isSafeInteger(value) && value >= 0, 4, "invalid owned process numeric field");
		return value;
	}

	requestStart() {
	  return {
		  wall_start_ms: Date.now(),
		  mono_start_ns: Math.floor(performance.now() * 1e6)
		};
	}

	requestEnd(start) {
	  this.requests.push({
		  ...start,
		  wall_end_ms: Date.now(),
		  mono_end_ns: Math.floor(performance.now() * 1e6)
		});
	}

	start(rootPid) {
	  this.rootPid = this.uint(rootPid);
		this.require(rootPid > 0, 2, "owned root PID is unavailable");
	}

	ancestry(entries, entry) {
	  const seen = new Set(), chain = [];
		let current = entry;

		for (; ; ) {
		  this.require(current && !seen.has(current.pid), 2, "owned server ancestry is unavailable");
			seen.add(current.pid);

			chain.push({
			  pid: this.uint(current.pid),
			  parent_pid: this.uint(current.parentPid),
			  start_ticks: this.uint(current.startTicks)
			});

			if (current.pid === this.rootPid)
			  return chain;

			current = entries.find(candidate => candidate.pid === current.parentPid);
		}
	}

	// Cleanup only: existing snapshots verify observed births, never native roles or CPU ownership.
	// Unobserved transient descendants remain outside this registry's coverage.
	rememberCleanupBirths(processes) {
	  for (const entry of processes) {
		  const seen = new Set();
			let current = entry;

			while (current && !seen.has(current.pid)) {
			  seen.add(current.pid);

				if (current.pid === this.rootPid) {
				  if (Number.isSafeInteger(entry.pid) && entry.pid > 0 && Number.isSafeInteger(entry.startTicks) && entry.startTicks >= 0) this.cleanupBirths.set(`${entry.pid}:${entry.startTicks}`, {
					  pid: entry.pid,
					  start_ticks: entry.startTicks
					});

					break;
				}

				current = processes.find(candidate => candidate.pid === current.parentPid);
			}
		}
	}

	async observe(processes) {
	  const started = performance.now(), wallStart = Date.now();

		try {
		  this.rememberCleanupBirths(processes);
			const candidates = processes.filter(entry => entry.command.startsWith("light-headless"));
			this.require(candidates.length === 1, 1, "expected exactly one owned headless server");
			const entry = candidates[0];
			const ancestry = this.ancestry(processes, entry);
			const executable = await this.readlink(`/proc/${entry.pid}/exe`);

			this.require(
			  path.isAbsolute(executable) && !executable.endsWith(" (deleted)") && path.basename(executable).startsWith("light-headless"),
				3,
				"actual server executable identity mismatch"
			);

			const confirmed = parseLinuxProcessStat(await this.readStat(`/proc/${entry.pid}/stat`));

			this.require(
			  confirmed.pid === entry.pid && confirmed.parentPid === entry.parentPid && confirmed.startTicks === entry.startTicks && confirmed.command === entry.command,
				4,
				"server process changed during executable verification"
			);

			this.require(
			  (await this.readlink(`/proc/${entry.pid}/exe`)) === executable,
				3,
				"server executable changed during verification"
			);

			const record = {
			  pid: this.uint(entry.pid),
			  parent_pid: this.uint(entry.parentPid),
			  start_ticks: this.uint(entry.startTicks),
			  executable,
			  ancestry
			};

			const previous = this.bindings.get(entry.pid);

			this.require(
			  (this.bindings.size === 0 || previous) && (!previous || JSON.stringify(previous) === JSON.stringify(record)),
				5,
				"owned server identity changed"
			);

			this.bindings.set(entry.pid, record);

			this.receipts.push({
			  wall_start_ms: wallStart,
			  wall_end_ms: Date.now(),
			  owned: [record],

			  server: {
				  child_pid: record.pid,
				  parent_pid: record.parent_pid,
				  executable
				}
			});
		} catch (error) {
		  this.observerFailureReason ||= 6;
			throw error;
		} finally {
		  this.observerWallNs = this.uint(this.observerWallNs + Math.ceil((performance.now() - started) * 1e6));
		}
	}

	async requireStopped() {
	  for (const identity of this.cleanupBirths.values()) {
		  try {
			  const current = parseLinuxProcessStat(await this.readStat(`/proc/${identity.pid}/stat`));

				this.require(
				  current.startTicks !== identity.start_ticks,
					5,
					"observed owned process still alive after termination"
				);
			} catch (error) {
			  if (error.code !== "ENOENT" && error.code !== "ESRCH")
				  throw error;
			}
		}
	}
}
