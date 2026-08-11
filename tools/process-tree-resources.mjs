import { readdir, readFile } from "node:fs/promises";

export function parseLinuxProcessStat(text) {
	const close = text.lastIndexOf(")");
	if (close < 0) throw new Error("invalid /proc process stat");
	const pid = Number.parseInt(text.slice(0, text.indexOf(" ")), 10);
	const command = text.slice(text.indexOf("(") + 1, close);
	const fields = text
		.slice(close + 2)
		.trim()
		.split(/\s+/u);
	return {
		pid,
		command,
		parentPid: Number.parseInt(fields[1], 10),
		cpuTicks: Number.parseInt(fields[11], 10) + Number.parseInt(fields[12], 10),
		startTicks: Number.parseInt(fields[19], 10),
		residentPages: Number.parseInt(fields[21], 10),
	};
}

export function descendants(processes, rootPid) {
	const included = new Set([rootPid]);
	let changed = true;
	while (changed) {
		changed = false;
		for (const process of processes) {
			if (!included.has(process.parentPid) || included.has(process.pid))
				continue;
			included.add(process.pid);
			changed = true;
		}
	}
	return processes.filter((process) => included.has(process.pid));
}

export async function readLinuxProcessTree(rootPid) {
	const entries = await readdir("/proc");
	const processes = [];
	await Promise.all(
		entries
			.filter((entry) => /^\d+$/u.test(entry))
			.map(async (entry) => {
				try {
					processes.push(
						parseLinuxProcessStat(
							await readFile(`/proc/${entry}/stat`, "utf8"),
						),
					);
				} catch {}
			}),
	);
	return descendants(processes, rootPid);
}

export function processTreeResourceDelta(
	before,
	after,
	elapsedSeconds,
	options = {},
) {
	const ticksPerSecond = options.ticksPerSecond ?? 100;
	const pageSize = options.pageSize ?? 4096;
	const earlier = new Map(
		before.map((entry) => [`${entry.pid}:${entry.startTicks}`, entry]),
	);
	let cpuTicks = 0;
	let residentBytes = 0;
	for (const entry of after) {
		const previous = earlier.get(`${entry.pid}:${entry.startTicks}`);
		if (previous) cpuTicks += Math.max(0, entry.cpuTicks - previous.cpuTicks);
		residentBytes += Math.max(0, entry.residentPages) * pageSize;
	}
	return {
		cpuPercent:
			elapsedSeconds > 0
				? (cpuTicks / ticksPerSecond / elapsedSeconds) * 100
				: 0,
		residentBytes,
	};
}
