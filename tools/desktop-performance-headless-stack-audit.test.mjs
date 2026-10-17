import assert from "node:assert/strict";
import test from "node:test";
import {
	auditPerfScript,
	validateAuditReport,
} from "./desktop-performance-headless-stack-audit.mjs";

const owned = "/private/control",
	runtime = "/usr/lib/libc.so.6";
const frame = (symbol = "light_engine::render", dso = owned, ip = "1001") =>
	` ${ip} ${symbol} (${dso})`;
const sample = (frames, pid = 42, inline = false) =>
	`${pid}/${pid} 10.123456789: 20408163 cpu-clock:u:${inline ? frames[0] : ""}\n${(inline ? frames.slice(1) : frames).join("\n")}\n\n`;
const expected = (samples, frames, self = 0, ancestors = 0, depth1 = 0) => ({
	samples,
	foreign_samples: 0,
	stackless_samples: 0,
	coverage: {
		frame_count: frames,
		unresolved_self_samples: self,
		unresolved_ancestor_frames: ancestors,
		depths: [
			{ id: 0, samples: 0 },
			{ id: 1, samples: depth1 },
		],
	},
});
const audit = (text, exp, more = {}) =>
	auditPerfScript(text, {
		tgid: 42,
		ownedExecutable: owned,
		verifiedRuntimePaths: [runtime],
		expectedReport: exp,
		...more,
	});
const maps =
	"1000-2000 r-xp 00000000 00:01 9007199254740993 /private/control\n2000-3000 rw-p 00001000 00:01 1 /usr/lib/libc.so.6\n3000-4000 rw-p 00000000 00:00 0 \n";

test("shallow and inline-shaped physical frames retain exact numeric parity and no private strings", () => {
	const text = `# private comment\n${sample([frame()])}${sample([frame("helper (inlined)"), frame("light_dynamics::thing")], 42, true)}`;
	const report = audit(text, expected(2, 3, 0, 0, 1));
	assert.equal(report.parser_valid, 1);
	assert.equal(report.parity_valid, 1);
	assert.equal(report.audit_valid, 1);
	assert.equal(report.comment_lines, 1);
	assert.equal(report.recognized_header_lines, 2);
	assert.equal(report.recognized_frame_lines, 3);
	assert.equal(report.classes[1].self_samples, 2);
	assert.equal(report.classes[1].resolved_unmapped_self_samples, 1);
	assert.equal(report.depth1_samples, 1);
	assert.equal(report.maps_available, 0);
	assert.equal(report.maps_are_sample_time, 0);
	assert(!JSON.stringify(report).includes(owned));
	const numeric = (value) =>
		typeof value === "number" ||
		(Array.isArray(value)
			? value.every(numeric)
			: Object.values(value).every(numeric));
	assert(numeric(report));
	assert.equal(validateAuditReport(report), report);
});

test("unknown self and ancestors remain separate and optional mapping absence is explicit", () => {
	const report = audit(
		sample([frame("[unknown]"), frame("0xfeed", "[unknown]")]),
		expected(1, 2, 1, 1),
	);
	assert.equal(report.parity_valid, 1);
	assert.equal(report.unresolved_self_samples, 1);
	assert.equal(report.unresolved_ancestor_frames, 1);
	assert.equal(report.classes[1].unresolved_self_samples, 1);
	assert.equal(report.classes[0].unresolved_ancestor_frames, 1);
	assert.equal(report.unresolved_ancestor_mapping_unavailable, 1);
	assert.equal(report.unresolved_ancestor_unmapped, 0);
	assert.equal(report.unresolved_ancestor_dso_filename_unavailable, 1);
});

test("fixed DSO classes distinguish exact verified paths from observed names and aliases", () => {
	const dsos = [
		"[unknown]",
		owned,
		runtime,
		"/other/libgcc_s.so.1",
		"libstdc++.so.6",
		"[vdso]",
		"/alias/control",
		"anonymous",
	];
	const report = audit(
		dsos.map((dso) => sample([frame("unmapped", dso)])).join(""),
		expected(8, 8, 0, 0, 8),
	);
	assert.equal(report.parity_valid, 1);
	for (const row of report.classes) {
		assert.equal(row.self_samples, 1);
		assert.equal(row.depth1_self_samples, 1);
		assert.equal(row.resolved_unmapped_self_samples, 1);
	}
	assert.equal(
		audit(
			sample([frame("unmapped", "/alias/libc.so.6")]),
			expected(1, 1, 0, 0, 1),
		).classes[6].self_samples,
		1,
	);
	assert.equal(
		audit(
			sample([frame("unmapped", "/verified/libgcc_s.so.1")]),
			expected(1, 1, 0, 0, 1),
			{ verifiedRuntimePaths: ["/verified/libgcc_s.so.1"] },
		).classes[2].self_samples,
		1,
	);
});

test("bigint snapshot membership and filename equality are separate from ownership and sample-time claims", () => {
	const ancestors = [
		frame("[unknown]", owned, "1001"),
		frame("[unknown]", "/alias/control", "1002"),
		frame("0xbeef", runtime, "2001"),
		frame("[unknown]", "[unknown]", "3001"),
		frame("[unknown]", owned, "4000"),
	];
	const report = audit(sample([frame(), ...ancestors]), expected(1, 6, 0, 5), {
		mapsText: maps,
	});
	assert.equal(report.parity_valid, 1);
	assert.equal(report.maps_available, 1);
	assert.equal(report.maps_snapshot_phase, 1);
	assert.equal(report.maps_are_sample_time, 0);
	assert.equal(report.unresolved_ancestor_exec_mapped, 2);
	assert.equal(report.unresolved_ancestor_nonexec_mapped, 2);
	assert.equal(report.unresolved_ancestor_unmapped, 1);
	assert.equal(report.unresolved_ancestor_dso_filename_match, 2);
	assert.equal(report.unresolved_ancestor_dso_filename_mismatch, 1);
	assert.equal(report.unresolved_ancestor_dso_filename_unavailable, 2);
	const high =
		"ffffffffff600000-ffffffffff601000 r-xp 00000000 00:00 0 [vsyscall]\n";
	const exact = audit(
		sample([frame(), frame("[unknown]", "[vsyscall]", "ffffffffff600001")]),
		expected(1, 2, 0, 1),
		{ mapsText: high },
	);
	assert.equal(exact.unresolved_ancestor_exec_mapped, 1);
	assert.equal(exact.unresolved_ancestor_dso_filename_match, 1);
});

test("invalid maps discard partial membership; anonymous one-blank lines are valid", () => {
	for (const invalid of [
		"",
		"bad\n",
		maps + "1000-2000 r-xp 0 00:00 0\n",
		"2000-1000 r-xp 0 00:00 0\n",
		"1000-2000 r-xp 0 00:00 18446744073709551616\n",
		"#".repeat(1024 * 1024 + 1),
		maps.replace("/private/control", "/private/\0control"),
	]) {
		const report = audit(
			sample([frame(), frame("[unknown]")]),
			expected(1, 2, 0, 1),
			{ mapsText: invalid },
		);
		assert.equal(report.audit_valid, 0);
		assert.equal(report.maps_requested, 1);
		assert.equal(report.maps_valid, 0);
		assert.equal(report.maps_available, 0);
		assert(report.maps_rejected_lines > 0);
		assert.equal(report.unresolved_ancestor_mapping_unavailable, 1);
		assert.equal(report.unresolved_ancestor_exec_mapped, 0);
	}
});

test("malformed, unrecognized, orphan and empty sample inputs retain failure flags", () => {
	for (const bad of [
		"",
		"# only a comment\n",
		"not a frame\n",
		frame() + "\n",
		sample([" corrupted"]),
		"42/42 10.1234567890: 1 cpu-clock:u:\n",
		"42/42 1.0: 1 cpu-clock:u:\n\n",
		sample([frame()]) + "PERF_RECORD_LOST id: 1, lost: 2\n",
	]) {
		const report = audit(bad, expected(0, 0));
		assert.equal(report.parser_valid, 0);
		assert.equal(report.parity_valid, 0);
		assert(
			report.rejected_lines > 0 ||
				report.stackless_samples > 0 ||
				report.samples === 0,
		);
	}
	const foreign = audit(sample([frame()], 99), expected(0, 0));
	assert.equal(foreign.foreign_samples, 1);
	assert.equal(foreign.samples, 0);
	const mismatch = audit(sample([frame()]), expected(1, 2, 0, 0, 1));
	assert.equal(mismatch.parser_valid, 1);
	assert.equal(mismatch.parity_valid, 0);
	assert.equal(audit(sample([frame()]), undefined).parity_valid, 0);
});

test("byte, line, sample and frame hard caps remain numeric failures", () => {
	const cases = [
		"x".repeat(64 * 1024 * 1024 + 1),
		"x".repeat(4097),
		sample(Array.from({ length: 129 }, () => frame())),
		sample([frame()]).repeat(10001),
	];
	for (const text of cases) {
		const report = audit(text, expected(0, 0));
		assert.equal(report.cap_exceeded, 1);
		assert.equal(report.parser_valid, 0);
		assert.equal(report.parity_valid, 0);
	}
	assert.equal(
		audit(sample(Array.from({ length: 128 }, () => frame())), expected(1, 128))
			.cap_exceeded,
		0,
	);
});

test("closed exact-key validator rejects strings, unsafe integers, unknown fields and forged identities", () => {
	const base = audit(sample([frame()]), expected(1, 1, 0, 0, 1));
	for (const mutate of [
		(r) => {
			r.audit_valid = 0;
		},
		(r) => {
			r.path = owned;
		},
		(r) => {
			r.samples = "1";
		},
		(r) => {
			r.samples = Number.MAX_SAFE_INTEGER + 1;
		},
		(r) => {
			r.classes[0].symbol = "private";
		},
		(r) => {
			r.classes[0].id = 1;
		},
		(r) => {
			r.maps_are_sample_time = 1;
		},
		(r) => {
			r.maps_snapshot_phase = 1;
		},
		(r) => {
			r.unresolved_ancestor_unmapped = 1;
		},
		(r) => {
			r.parser_valid = 2;
		},
		(r) => {
			r.rejected_lines = 1;
		},
	]) {
		const report = structuredClone(base);
		mutate(report);
		assert.throws(() => validateAuditReport(report));
	}
	assert.throws(() =>
		auditPerfScript("", { tgid: 42, ownedExecutable: "relative" }),
	);
});

test("foreign and stackless parity checks distinguish matching counts from mismatches", () => {
	const text = sample([frame()]) + sample([frame()], 99);
	const matching = { ...expected(1, 1, 0, 0, 1), foreign_samples: 1 };
	assert.equal(audit(text, matching).parity_valid, 1);
	assert.equal(
		audit(text, { ...matching, foreign_samples: 0 }).parity_valid,
		0,
	);
	const stackless = sample([frame()]) + "42/42 10.1: 1 cpu-clock:u:\n\n";
	const exp = { ...expected(2, 1, 0, 0, 1), stackless_samples: 1 };
	const actual = audit(stackless, exp);
	assert.equal(actual.parity_valid, 1);
	assert.equal(actual.parser_valid, 0);
	assert.equal(actual.audit_valid, 0);
	assert.equal(
		audit(stackless, { ...exp, stackless_samples: 0 }).parity_valid,
		0,
	);
});

test("zero-IP and terminal unknown ancestors overlap without being dropped or resolved", () => {
	const text =
		sample([
			frame(),
			frame("[unknown]", "[unknown]", "0"),
			frame("[unknown]", owned, "1002"),
		]) + sample([frame(), frame("0x0", "[unknown]", "0")]);
	const report = audit(text, expected(2, 5, 0, 3), { mapsText: maps });
	assert.equal(report.audit_valid, 1);
	assert.equal(report.unresolved_ancestor_frames, 3);
	assert.equal(report.unresolved_ancestor_zero_ip_frames, 2);
	assert.equal(report.unresolved_ancestor_terminal_frames, 2);
	assert.equal(report.unresolved_ancestor_unmapped, 2);
	assert.equal(report.unresolved_ancestor_exec_mapped, 1);
	for (const key of [
		"unresolved_ancestor_zero_ip_frames",
		"unresolved_ancestor_terminal_frames",
	]) {
		assert.throws(() => validateAuditReport({ ...report, [key]: 4 }));
	}
});

test("up to64 exact verified runtime aliases are bounded", () => {
	const paths = Array.from({ length: 64 }, (_, index) => `/verified/${index}`);
	const report = audit(
		sample([frame("unmapped", paths[63])]),
		expected(1, 1, 0, 0, 1),
		{ verifiedRuntimePaths: paths },
	);
	assert.equal(report.classes[2].self_samples, 1);
	assert.throws(() =>
		audit(sample([frame()]), expected(1, 1, 0, 0, 1), {
			verifiedRuntimePaths: [...paths, "/verified/extra"],
		}),
	);
});

test("schema2 fixed spelling families classify only resolved unmapped self symbols", () => {
	const cases = [
		["plain_other", owned, 0],
		["light_control::update", owned, 1],
		["<std::thread::Thread as Trait>::run", owned, 2],
		["core::slice::sort", owned, 3],
		["alloc::string::String::new", owned, 4],
		["__rust_alloc+0x10", owned, 5],
		["other_crate::work", owned, 9],
		["_int_malloc+0x1a", runtime, 10],
		["__GI___libc_calloc@@GLIBC_2.2.5+0x2", runtime, 10],
		["sin@GLIBC_2.2.5+0x10", runtime, 11],
		["memset", runtime, 12],
		["unclassified_libc_leaf", "/unverified/libc.so.6", 13],
		["unclassified_libm_leaf", "libm.so.6", 13],
	];
	const text = cases
		.map(([symbol, dso]) => sample([frame(symbol, dso)]))
		.join("");
	const report = audit(
		text,
		expected(cases.length, cases.length, 0, 0, cases.length),
	);
	assert.equal(report.schema_version, 2);
	assert.equal(report.function_mapping_version, 2);
	assert.equal(report.audit_valid, 1);
	assert.equal(report.resolved_unmapped_self_families.length, 14);
	for (let id = 0; id < 14; id++)
		assert.equal(
			report.resolved_unmapped_self_families[id].self_samples,
			cases.filter((entry) => entry[2] === id).length,
		);
	assert.equal(report.resolved_unmapped_self_period, cases.length * 20408163);
	// These broad namespace classes are already mapped by function map2 and stay excluded.
	const excluded = [
		"light_engine::render",
		"malloc",
		"__memcpy_avx2",
		"hashbrown::map",
		"rayon::work",
		"tokio::task",
		"[unknown]",
		"0x0",
	];
	const result = audit(
		excluded.map((symbol) => sample([frame(symbol)])).join(""),
		expected(8, 8, 2, 0, 8),
	);
	assert.equal(result.resolved_unmapped_self_period, 0);
	assert(
		result.resolved_unmapped_self_families.every(
			(entry) => entry.self_samples === 0 && entry.self_period === 0,
		),
	);
});

test("spelling boundaries, closed allowlists and observed DSO labels do not imply ownership", () => {
	const cases = [
		["light_corex::thing", owned, 9],
		["light_unlisted::thing", owned, 9],
		["stdish::thing", owned, 9],
		["project::malloc_helper", runtime, 9],
		["_int_malloc_helper", owned, 0],
		["sin@NOT_GLIBC", owned, 0],
		["sin+123", owned, 0],
		["unclassified", "/not-libc.so.6", 0],
		["unclassified", "/unverified/libc.so.6", 13],
	];
	for (const [symbol, dso, id] of cases) {
		const report = audit(sample([frame(symbol, dso)]), expected(1, 1, 0, 0, 1));
		assert.equal(report.resolved_unmapped_self_families[id].self_samples, 1);
		if (id === 13)
			assert.equal(report.classes[6].resolved_unmapped_self_samples, 1);
	}
});

test("weighted periods use each private header once and reject overflow numerically", () => {
	const weighted = (symbol, period) =>
		sample([frame(symbol)]).replace("20408163", String(period));
	const text =
		weighted("std::one", 7) +
		weighted("std::two", 11) +
		weighted("sin", 23) +
		weighted("light_engine::mapped", 99);
	const report = audit(text, expected(4, 4, 0, 0, 4));
	assert.equal(report.resolved_unmapped_self_period, 41);
	assert.equal(report.resolved_unmapped_self_families[2].self_period, 18);
	assert.equal(report.resolved_unmapped_self_families[11].self_period, 23);
	assert.equal(report.arithmetic_overflow, 0);
	assert.equal(report.audit_valid, 1);
	const overflow = audit(
		weighted("std::one", Number.MAX_SAFE_INTEGER) + weighted("sin", 1),
		expected(2, 2, 0, 0, 2),
	);
	assert.equal(overflow.arithmetic_overflow, 1);
	assert.equal(overflow.audit_valid, 0);
	assert.equal(overflow.parser_valid, 0);
	assert.equal(overflow.resolved_unmapped_self_period, Number.MAX_SAFE_INTEGER);
	assert.equal(overflow.resolved_unmapped_self_families[11].self_samples, 1);
	const unsafe = audit(
		weighted("std::one", "9007199254740992"),
		expected(0, 0),
	);
	assert.equal(unsafe.audit_valid, 0);
	assert.equal(unsafe.rejected_lines > 0, true);
});

test("closed schema2 rejects forged family keys, totals, periods and overflow flags", () => {
	const base = audit(sample([frame("std::one")]), expected(1, 1, 0, 0, 1));
	for (const mutate of [
		(r) => {
			r.schema_version = 1;
		},
		(r) => {
			r.resolved_unmapped_self_families[0].name = "private";
		},
		(r) => {
			r.resolved_unmapped_self_families[2].self_samples++;
		},
		(r) => {
			r.resolved_unmapped_self_families[2].self_period++;
		},
		(r) => {
			r.resolved_unmapped_self_period++;
		},
		(r) => {
			r.resolved_unmapped_self_families[0].self_period =
				Number.MAX_SAFE_INTEGER;
		},
		(r) => {
			r.arithmetic_overflow = 1;
		},
		(r) => {
			r.resolved_unmapped_self_families[0].id = 2;
		},
	]) {
		const report = structuredClone(base);
		mutate(report);
		assert.throws(() => validateAuditReport(report));
	}
});

test("overflow cannot retain valid parity when other validity flags are cleared", () => {
	const report = audit(sample([frame("std::one")]), expected(1, 1, 0, 0, 1));
	report.arithmetic_overflow = 1;
	report.parser_valid = 0;
	report.audit_valid = 0;
	assert.equal(report.parity_valid, 1);
	assert.throws(() => validateAuditReport(report));
	report.parity_valid = 0;
	assert.equal(validateAuditReport(report), report);
});
