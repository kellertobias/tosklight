import { createHash } from "node:crypto";
import {
	dsoId,
	validateCoverage,
} from "./desktop-performance-headless-stack-coverage.mjs";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import path from "node:path";
import test, { after } from "node:test";
import { artifactPaths } from "./artifact-paths.mjs";
import {
	HeadlessStackCapture,
	PerfScriptParser,
	emptyReport,
	exportReport,
	functionId,
} from "./desktop-performance-headless-stacks.mjs";
const root = await mkdtemp(path.join(artifactPaths.tmp, "coverage-tests-"));
after(() => rm(root, { recursive: true, force: true }));
const owned = "/private/mount/usr/bin/light-headless";
function parse(frames, executable = owned) {
	const parser = new PerfScriptParser(10, undefined, executable);
	parser.feed(
		`10/10 1.000000001: 20 cpu-clock:u:\n${frames.map(([symbol, dso]) => `  123 ${symbol} (${dso})`).join("\n")}\n\n`,
	);
	parser.finish();
	return parser;
}
test("unresolved coverage separates sampled IP, ancestors and resolved unmapped symbols", () => {
	const self = parse([
		["[unknown]", owned],
		["malloc", "/lib/libc.so.6"],
	]).report;
	assert.equal(self.coverage?.unresolved_self_samples, 1);
	assert.equal(self.coverage.unresolved_ancestor_frames, 0);
	assert.equal(self.coverage.dsos[1].unresolved_self_samples, 1);
	const ancestor = parse([
		["private_unmapped_symbol", owned],
		["0xbeef", "/lib/libc.so.6"],
	]).report;
	assert.equal(ancestor.coverage.unresolved_self_samples, 0);
	assert.equal(ancestor.coverage.unresolved_ancestor_frames, 1);
	assert.equal(ancestor.coverage.resolved_unmapped_self_samples, 1);
	assert.equal(ancestor.coverage.dsos[2].unresolved_ancestor_frames, 1);
	assert.equal(ancestor.unknown_self_samples, 1);
	assert.equal(ancestor.unresolved_frames, 1);
	const both = parse([
		["0xdead", owned],
		["[unknown]", "[unknown]"],
	]).report;
	assert.equal(both.coverage.unresolved_both_samples, 1);
	assert.equal(both.coverage.samples_with_unresolved, 1);
	assert.equal(both.unresolved_frames, 2);
});

test("fixed mapping v2 preserves every old ID and priority before broad namespaces", () => {
	const old = [
		"private_unknown",
		"light_dynamics::programming::preparation::prepare_owner_sample",
		"light_dynamics::programming::preparation::prepare_dynamic_family_samples",
		"light_dynamics::programming::complete_deferred",
		"light_headless_runtime::hybrid::staged::prepare_captured",
		"light_headless_runtime::family_inputs::assemble_captured_family_inputs",
		"light_headless_runtime::family_lanes::finish",
		"light_dynamics::programming::composition::compose",
		"light_dynamics::programming::classify",
		"light_dynamics::programming::compile",
		"light_engine::prepare_families",
		"rayon_core::join::execute",
		"malloc",
		"free",
		"realloc",
		"memcpy",
		"memmove",
		"light_headless_runtime::output_scheduler::tick",
	];
	old.forEach((symbol, id) => assert.equal(functionId(symbol), id, symbol));
	const broad = [
		"light_dynamics::other",
		"light_headless_runtime::other",
		"light_engine::other",
		"alloc::collections::btree::search",
		"core::ptr::drop_in_place<light_dynamics::Private>",
		"hashbrown::raw::find",
		"rayon_core::registry::other",
		"tokio::runtime::other",
	];
	broad.forEach((symbol, index) =>
		assert.equal(functionId(symbol), index + 18, symbol),
	);
	assert.equal(functionId("unmapped<light_engine::Private>"), 0);
	assert.equal(functionId("<light_dynamics::Private as Clone>::clone"), 18);
	const report = emptyReport();
	assert.equal(report.function_mapping_version, 2);
	assert.equal(report.functions.length, 26);
});

test("DSO classes require exact verified ownership, never a matching basename", () => {
	const cases = [
		["", 0],
		["[unknown]", 0],
		[owned, 1],
		["/lib/x86_64-linux-gnu/libc.so.6", 2],
		["/lib/ld-linux-x86-64.so.2", 2],
		["/foreign/light-headless", 3],
		["light-headless", 4],
		["[vdso]", 4],
	];
	for (const [dso, id] of cases) assert.equal(dsoId(dso, owned), id);
	assert.equal(dsoId(owned, null), 3);
	assert.equal(dsoId(`${owned} (deleted)`, owned), 3);
	assert.equal(dsoId("/foreign/light-headless", owned), 3);
	assert.throws(() => new PerfScriptParser(10, undefined, "relative"));
	for (const [dso, id] of cases) {
		const report = parse([["[unknown]", dso]]).report;
		assert.equal(report.coverage.dsos[id].unresolved_self_samples, 1);
		validateCoverage(report);
	}
});

test("depth histogram is bounded and distinct from unresolved or stackless coverage", () => {
	for (const [depth, id] of [
		[0, 0],
		[1, 1],
		[2, 2],
		[3, 3],
		[4, 3],
		[5, 4],
		[8, 4],
		[9, 5],
		[16, 5],
		[17, 6],
		[32, 6],
		[33, 7],
		[64, 7],
		[65, 8],
		[128, 8],
	]) {
		const report = parse(
			Array.from({ length: depth }, () => ["malloc", owned]),
		).report;
		assert.equal(report.coverage.depths[id].samples, 1);
		assert.equal(report.coverage.frame_count, depth);
		assert.equal(report.stackless_samples, Number(depth === 0));
		assert.equal(report.depth_capped_samples, Number(depth === 128));
		validateCoverage(report);
	}
	assert.throws(
		() => parse(Array.from({ length: 129 }, () => ["malloc", owned])),
		/frame cap/u,
	);
	const foreign = new PerfScriptParser(10);
	foreign.feed(
		"900/901 1.000000001: 20 cpu-clock:u:\n 123 [unknown] (/private/foreign)\n\n",
	);
	foreign.finish();
	assert.equal(foreign.report.foreign_samples, 1);
	assert.equal(foreign.report.coverage.frame_count, 0);
	validateCoverage(foreign.report);
});

test("numeric schema5 coverage roundtrip rejects raw data, unknown rows and inconsistent identities", async () => {
	const valid = parse([
		["secret_operator_symbol", owned],
		["[unknown]", "/private/other.so"],
	]).report;
	const directory = await mkdtemp(path.join(root, "roundtrip-"));
	await exportReport(directory, valid);
	assert.deepEqual(
		JSON.parse(
			await readFile(
				path.join(directory, "headless-stacks-numeric.json"),
				"utf8",
			),
		),
		valid,
	);
	const text = JSON.stringify(valid);
	assert.doesNotMatch(text, /private|secret_operator_symbol|0xbeef|\.so/u);
	assert.equal(valid.schema_version, 5);
	const mutations = [
		(r) => {
			r.capture_complete = 1;
		},
		(r) => {
			r.schema_version = 4;
		},
		(r) => {
			r.function_mapping_version = 1;
		},
		(r) => {
			r.coverage.raw = "secret";
		},
		(r) => {
			r.coverage.dsos[0].path = owned;
		},
		(r) => {
			r.coverage.dsos[0].id = 1;
		},
		(r) => {
			r.coverage.dsos.pop();
		},
		(r) => {
			r.coverage.depths[0].id = 1;
		},
		(r) => {
			r.coverage.depths[0].samples = 1;
		},
		(r) => {
			r.coverage.frame_count = 129;
		},
		(r) => {
			r.coverage.resolved_unmapped_self_samples = 0;
		},
		(r) => {
			r.coverage.unresolved_ancestor_frames = 0;
		},
		(r) => {
			r.coverage.samples_with_unresolved = 0;
		},
		(r) => {
			r.coverage.unresolved_ancestor_only_samples = 0;
			r.coverage.samples_with_unresolved = 0;
		},
		(r) => {
			r.coverage.dsos[3].unresolved_ancestor_frames = 0;
		},
		(r) => {
			r.coverage.frame_count = -1;
		},
		(r) => {
			r.coverage.frame_count = Number.MAX_SAFE_INTEGER + 1;
		},
		(r) => {
			r.coverage.depths[0].samples = null;
		},
		(r) => {
			r.coverage.dsos[0].unresolved_self_samples = "/private/123";
		},
		(r) => {
			r.coverage.dsos[0].unresolved_self_samples = 1;
		},
	];
	for (const mutate of mutations) {
		const report = structuredClone(valid);
		mutate(report);
		const invalidDirectory = await mkdtemp(path.join(root, "invalid-report-"));
		await assert.rejects(exportReport(invalidDirectory, report));
	}
});

test("actual capture keeps unresolved strict failure and passes exact owned path into decoding", async () => {
	for (const originalFailure of [false, true]) {
		const binding = { pid: 10, executable: owned, ancestry: [{ pid: 9 }] };
		const cpu = {
			mode: "off",
			rootPid: 9,
			bindings: new Map([[10, binding]]),
			receipts: Array.from({ length: 61 }, () => ({ owned: [binding] })),
		};
		const original = new Error("original deadline failure");
		let exported,
			measured = 0,
			stops = 0;
		const report = emptyReport(),
			capture = new HeadlessStackCapture(
				{
					root: "/private",
					outputDirectory: root,
					preflight: { perf: "/perf", privileged: false },
					binaryHash: createHash("sha256").update("binary").digest("hex"),
					report,
				},
				{
					verifyIdentity: async () => {},
					readFile: async () => Buffer.from("binary"),
					threads: async () => [10],
					writePrivate: async () => {},
					verifyRuntimeSymbols: async () => [],
					openRecorder: async () => ({
						raw: { filename: "/private/raw", bytes: 128 },
						capped: false,
						finish: async () => {
							stops++;
							return {
								endedEarly: false,
								disableAckNs: 2000000000n,
								closedNs: 2200000000n,
								diagnostics: { record_disable_acknowledged: 1 },
							};
						},
					}),
					decode: async (...args) => {
						assert.equal(args[4], 10);
						assert.equal(args[5], owned);
						return parse(
							[
								["[unknown]", args[5]],
								["malloc", "/lib/libc.so.6"],
							],
							args[5],
						);
					},
					exportReport: async (_directory, r) => {
						validateCoverage(r);
						exported = structuredClone(r);
					},
				},
			);
		await assert.rejects(
			capture.run(cpu, async () => {
				measured++;
				if (originalFailure) throw original;
				return 1;
			}),
			(error) =>
				originalFailure
					? error === original
					: /partial sampling coverage/u.test(error.message),
		);
		assert.equal(measured, 1);
		assert.ok(stops >= 1);
		assert.equal(exported.capture_complete, 0);
		assert.equal(exported.unresolved_frames, 1);
		assert.equal(exported.coverage.dsos[1].unresolved_self_samples, 1);
		assert.equal(exported.functions[12].inclusive_samples, 1);
		assert.equal(exported.workload_failed, Number(originalFailure));
	}
});

test("coverage rejects impossible affected-stack and ancestor-stack populations", async (t) => {
	const header = "10/10 1.000000001: 20 cpu-clock:u:\n";
	const frame = (symbol) => ` 123 ${symbol} (${owned})\n`;
	const fixtures = [
		{
			name: "affected samples exclude stackless",
			text:
				header +
				"\n" +
				header +
				frame("[unknown]") +
				frame("[unknown]") +
				frame("malloc") +
				"\n",
			mutate: (c) => {
				c.samples_with_unresolved = 2;
				c.unresolved_self_only_samples = 1;
				c.unresolved_ancestor_only_samples = 1;
				c.unresolved_both_samples = 0;
			},
		},
		{
			name: "ancestor samples require multi-frame stacks",
			text:
				header +
				frame("[unknown]") +
				"\n" +
				header +
				frame("malloc") +
				frame("[unknown]") +
				frame("[unknown]") +
				"\n",
			mutate: (c) => {
				c.unresolved_self_only_samples = 0;
				c.unresolved_ancestor_only_samples = 1;
				c.unresolved_both_samples = 1;
			},
		},
	];
	for (const fixture of fixtures)
		await t.test(fixture.name, async () => {
			const parser = new PerfScriptParser(10, undefined, owned);
			parser.feed(fixture.text);
			parser.finish();
			const report = parser.report;
			validateCoverage(report);
			assert.equal(report.samples, 2);
			assert.equal(report.multi_frame_samples, 1);
			fixture.mutate(report.coverage);
			const directory = await mkdtemp(path.join(root, "invalid-population-"));
			await assert.rejects(exportReport(directory, report));
		});
});
