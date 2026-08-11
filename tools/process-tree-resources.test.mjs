import assert from "node:assert/strict";
import test from "node:test";
import {
	descendants,
	parseLinuxProcessStat,
	processTreeResourceDelta,
} from "./process-tree-resources.mjs";

function stat(pid, command, parentPid, user, system, start, resident) {
	const fields = Array(22).fill("0");
	fields[0] = "S";
	fields[1] = String(parentPid);
	fields[11] = String(user);
	fields[12] = String(system);
	fields[19] = String(start);
	fields[21] = String(resident);
	return `${pid} (${command}) ${fields.join(" ")}`;
}

test("parses process names containing spaces and selects the complete child tree", () => {
	const root = parseLinuxProcessStat(
		stat(10, "light desktop", 1, 10, 4, 8, 20),
	);
	const child = parseLinuxProcessStat(
		stat(11, "light-headless", 10, 5, 2, 9, 8),
	);
	const grandchild = parseLinuxProcessStat(
		stat(12, "WebKitWebProces", 11, 1, 1, 10, 4),
	);
	const unrelated = parseLinuxProcessStat(stat(99, "node", 1, 20, 2, 11, 30));
	assert.deepEqual(descendants([root, child, grandchild, unrelated], 10), [
		root,
		child,
		grandchild,
	]);
});

test("reports interval CPU and current aggregate resident memory", () => {
	const before = [parseLinuxProcessStat(stat(10, "desk", 1, 10, 5, 8, 20))];
	const after = [
		parseLinuxProcessStat(stat(10, "desk", 1, 40, 15, 8, 22)),
		parseLinuxProcessStat(stat(11, "webview", 10, 20, 5, 9, 8)),
	];
	assert.deepEqual(processTreeResourceDelta(before, after, 1), {
		cpuPercent: 40,
		residentBytes: 30 * 4096,
	});
});
