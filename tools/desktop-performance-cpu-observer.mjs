import { lstat, readdir, readFile, readlink, realpath } from "node:fs/promises";
import path from "node:path";
import { parseLinuxProcessStat } from "./process-tree-resources.mjs";

// Reason0 none;1 server count;2 ancestry;3 executable;4 stat;5 stable identity;6 observation I/O.
export class CpuProcessObserver {
  constructor(io = {}) {
    this.requests = [];
    this.receipts = [];
    this.bindings = new Map();
    this.cleanupBirths = new Map();
    this.nativeGenerations = new Map();
    this.nativeInvalid = new Set();
    this.nativeUnverified = 0;
    this.observerWallNs = 0;
    this.observerFailureReason = 0;
    this.readStat = io.readStat ?? ((filename) => readFile(filename, "utf8"));
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
      mono_start_ns: Math.floor(performance.now() * 1e6),
    };
  }

  requestEnd(start) {
    this.requests.push({
      ...start,
      wall_end_ms: Date.now(),
      mono_end_ns: Math.floor(performance.now() * 1e6),
    });
  }

  start(rootPid) {
    this.rootPid = this.uint(rootPid);
    this.require(rootPid > 0, 2, "owned root PID is unavailable");
  }

  ancestry(entries, entry) {
    const seen = new Set(),
      chain = [];
    let current = entry;

    for (;;) {
      this.require(current && !seen.has(current.pid), 2, "owned server ancestry is unavailable");
      seen.add(current.pid);

      chain.push({
        pid: this.uint(current.pid),
        parent_pid: this.uint(current.parentPid),
        start_ticks: this.uint(current.startTicks),
      });

      if (current.pid === this.rootPid) return chain;

      current = entries.find((candidate) => candidate.pid === current.parentPid);
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
          if (
            Number.isSafeInteger(entry.pid) &&
            entry.pid > 0 &&
            Number.isSafeInteger(entry.startTicks) &&
            entry.startTicks >= 0
          )
            this.cleanupBirths.set(`${entry.pid}:${entry.startTicks}`, {
              pid: entry.pid,
              start_ticks: entry.startTicks,
            });

          break;
        }

        current = processes.find((candidate) => candidate.pid === current.parentPid);
      }
    }
  }

  async nativeOwned(processes) {
    const owned = [];
    if (this.mode !== "on") return owned;
    for (const entry of processes.filter((value) => value.command.startsWith("viz-renderer"))) {
      const key = `${entry.pid}:${entry.startTicks}`;
      try {
        const chain = [],
          seen = new Set();
        let current = entry;
        for (;;) {
          if (!current || seen.has(current.pid)) throw new Error("native ancestry");
          seen.add(current.pid);
          if (
            ![current.pid, current.parentPid, current.startTicks].every(
              (value) => Number.isSafeInteger(value) && value >= 0,
            )
          )
            throw new Error("native identity");
          chain.push({ pid: current.pid, parent_pid: current.parentPid, start_ticks: current.startTicks });
          if (current.pid === this.rootPid) break;
          current = processes.find((value) => value.pid === current.parentPid);
        }
        const executable = await this.readlink(`/proc/${entry.pid}/exe`);
        if (
          !path.isAbsolute(executable) ||
          executable.endsWith(" (deleted)") ||
          !path.basename(executable).startsWith("viz-renderer")
        )
          throw new Error("native executable");
        const confirmed = parseLinuxProcessStat(await this.readStat(`/proc/${entry.pid}/stat`));
        if (
          confirmed.pid !== entry.pid ||
          confirmed.parentPid !== entry.parentPid ||
          confirmed.startTicks !== entry.startTicks ||
          confirmed.command !== entry.command ||
          (await this.readlink(`/proc/${entry.pid}/exe`)) !== executable
        )
          throw new Error("native changed");
        const record = {
          pid: entry.pid,
          parent_pid: entry.parentPid,
          start_ticks: entry.startTicks,
          executable,
          ancestry: chain,
        };
        const previous = this.nativeGenerations.get(key);
        if (previous && JSON.stringify(previous) !== JSON.stringify(record)) {
          this.nativeInvalid.add(key);
          throw new Error("native generation changed");
        }
        if (this.nativeInvalid.has(key)) throw new Error("native generation invalid");
        if (this.nativeGenerations.size >= 64 && !previous) throw new Error("native generation cap");
        this.nativeGenerations.set(key, record);
        owned.push(record);
      } catch {
        this.nativeUnverified++;
      }
    }
    return owned;
  }

  async nativeCaptures(directory, readPrivate, parseCapture) {
    const captures = [],
      seen = new Set();
    let invalid = 0,
      bytes = 0;
    try {
      const before = await lstat(directory);
      if (
        !before.isDirectory() ||
        before.isSymbolicLink() ||
        before.uid !== process.getuid() ||
        (before.mode & 0o7777) !== 0o700 ||
        (await realpath(directory)) !== directory
      )
        throw new Error("native directory");
      const names = (await readdir(directory)).sort();
      if (names.length > 128 || names.some((name) => !/^viz-cpu-[1-9][0-9]*-[1-9][0-9]*\.(json|stderr)$/.test(name)))
        throw new Error("native file cap or name");
      for (const name of names) {
        const metadata = await lstat(path.join(directory, name));
        bytes += metadata.size;
        if (!Number.isSafeInteger(bytes) || bytes > 64 * 1024 * 1024) throw new Error("native byte cap");
        const pair = name.endsWith(".json") ? name.replace(/\.json$/, ".stderr") : name.replace(/\.stderr$/, ".json");
        if (!names.includes(pair)) throw new Error("native orphan");
      }
      for (const name of names.filter((value) => value.endsWith(".json"))) {
        try {
          const text = await readPrivate(path.join(directory, name), 65536);
          const manifest = JSON.parse(text);
          if (text !== JSON.stringify(manifest)) throw new Error("noncanonical producer manifest");
          const keys = [
            "private_capture_schema",
            "child_pid",
            "parent_pid",
            "spawn_ordinal",
            "program",
            "arguments",
            "stderr_path",
          ];
          if (
            !manifest ||
            Array.isArray(manifest) ||
            Object.keys(manifest).length !== keys.length ||
            Object.keys(manifest).some((key) => !keys.includes(key))
          )
            throw new Error("native manifest fields");
          if (
            manifest.private_capture_schema !== 1 ||
            ![manifest.child_pid, manifest.parent_pid, manifest.spawn_ordinal].every(
              (value) => Number.isSafeInteger(value) && value > 0,
            )
          )
            throw new Error("native manifest numeric");
          const stem = `viz-cpu-${manifest.parent_pid}-${manifest.spawn_ordinal}`;
          if (
            name !== `${stem}.json` ||
            manifest.stderr_path !== path.join(directory, `${stem}.stderr`) ||
            JSON.stringify(manifest.arguments) !== '["--embed"]'
          )
            throw new Error("native manifest path or role");
          const matches = [...this.nativeGenerations.entries()].filter(
            ([key, value]) =>
              !this.nativeInvalid.has(key) &&
              value.pid === manifest.child_pid &&
              value.parent_pid === manifest.parent_pid &&
              value.executable === manifest.program,
          );
          if (matches.length !== 1) throw new Error("native birth ambiguous");
          const [key, record] = matches[0];
          if (seen.has(key)) {
            for (let index = captures.length - 1; index >= 0; index--) {
              if (
                captures[index].binding.child_pid === record.pid &&
                captures[index].binding.start_ticks === record.start_ticks
              )
                captures.splice(index, 1);
            }
            throw new Error("duplicate native birth");
          }
          seen.add(key);
          const groups = parseCapture(await readPrivate(manifest.stderr_path));
          captures.push({
            binding: {
              child_pid: record.pid,
              parent_pid: record.parent_pid,
              start_ticks: record.start_ticks,
              executable: record.executable,
              ancestry: record.ancestry,
            },
            groups,
          });
        } catch {
          invalid++;
        }
      }
      const after = await lstat(directory);
      if (
        before.ino !== after.ino ||
        before.dev !== after.dev ||
        before.mtimeMs !== after.mtimeMs ||
        JSON.stringify(names) !== JSON.stringify((await readdir(directory)).sort())
      )
        throw new Error("native directory changed");
    } catch {
      return { captures: [], invalid: invalid + 1 };
    }
    return { captures, invalid };
  }

  async observe(processes) {
    const started = performance.now(),
      wallStart = Date.now();

    try {
      this.rememberCleanupBirths(processes);
      const candidates = processes.filter((entry) => entry.command.startsWith("light-headless"));
      this.require(candidates.length === 1, 1, "expected exactly one owned headless server");
      const entry = candidates[0];
      const ancestry = this.ancestry(processes, entry);
      const executable = await this.readlink(`/proc/${entry.pid}/exe`);

      this.require(
        path.isAbsolute(executable) &&
          !executable.endsWith(" (deleted)") &&
          path.basename(executable).startsWith("light-headless"),
        3,
        "actual server executable identity mismatch",
      );

      const confirmed = parseLinuxProcessStat(await this.readStat(`/proc/${entry.pid}/stat`));

      this.require(
        confirmed.pid === entry.pid &&
          confirmed.parentPid === entry.parentPid &&
          confirmed.startTicks === entry.startTicks &&
          confirmed.command === entry.command,
        4,
        "server process changed during executable verification",
      );

      this.require(
        (await this.readlink(`/proc/${entry.pid}/exe`)) === executable,
        3,
        "server executable changed during verification",
      );

      const record = {
        pid: this.uint(entry.pid),
        parent_pid: this.uint(entry.parentPid),
        start_ticks: this.uint(entry.startTicks),
        executable,
        ancestry,
      };

      const previous = this.bindings.get(entry.pid);

      this.require(
        (this.bindings.size === 0 || previous) && (!previous || JSON.stringify(previous) === JSON.stringify(record)),
        5,
        "owned server identity changed",
      );

      this.bindings.set(entry.pid, record);
      const native = await this.nativeOwned(processes);

      this.receipts.push({
        wall_start_ms: wallStart,
        wall_end_ms: Date.now(),
        owned: [record, ...native],

        server: {
          child_pid: record.pid,
          parent_pid: record.parent_pid,
          executable,
        },
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
          "observed owned process still alive after termination",
        );
      } catch (error) {
        if (error.code !== "ENOENT" && error.code !== "ESRCH") throw error;
      }
    }
  }
}
