import path from "node:path";
import {
	functionId,
	FUNCTION_MAPPING_VERSION,
} from "./desktop-performance-headless-stack-coverage.mjs";

const CAPS = {
	bytes: 64 * 1024 * 1024,
	line: 4096,
	samples: 10000,
	frames: 128,
	maps: 1024 * 1024,
	mappings: 16384,
};
const require_ = (value) => {
	if (!value) throw new Error("Invalid private stack audit input/report");
};
const integer = (value) => Number.isSafeInteger(value) && value >= 0;
const missing = (symbol) => /^(?:\[unknown\]|0x[0-9a-f]+)$/iu.test(symbol);
const cleanAbsolute = (value) =>
	typeof value === "string" &&
	path.posix.isAbsolute(value) &&
	!/[\0\r\n]/u.test(value);
// Fixed classes: 0 unknown/missing; 1 exact owned executable; 2 exact verified
// runtime path; 3 observed UNVERIFIED libgcc; 4 observed UNVERIFIED libstdc++;
// 5 observed vdso; 6 other absolute; 7 other nonabsolute. Labels never prove ownership.
function category(dso, owned, runtimes) {
	if (!dso || dso === "[unknown]") return 0;
	if (dso === owned) return 1;
	if (runtimes.has(dso)) return 2;
	const basename = path.posix.basename(dso);
	if (/^libgcc_s\.so(?:\.\d+)*$/u.test(basename)) return 3;
	if (/^libstdc\+\+\.so(?:\.\d+)*$/u.test(basename)) return 4;
	if (dso === "[vdso]" || /^linux-vdso\.so(?:\.\d+)*$/u.test(basename))
		return 5;
	return path.posix.isAbsolute(dso) ? 6 : 7;
}
// Fixed resolved-unmapped SELF spelling families, not callers, CPU stages or ownership:
// 0 remainder; 1 SELECTED known project namespace; 2 std; 3 core; 4 alloc;
// 5 Rust global allocator wrapper; 6 hashbrown; 7 rayon; 8 tokio;
// 9 other namespace syntax (not necessarily Rust); 10 C allocator wrapper/internal;
// 11 C math wrapper; 12 C memory/string wrapper; 13 remaining observed libc/libm label.
// Selected names pinned from repository Cargo manifests; this is not all light_*.
const PROJECT_NAMES = new Set([
	"light_application",
	"light_control",
	"light_programmer",
	"light_playback",
	"light_output",
	"light_wire",
	"light_core",
	"light_fixture",
	"light_show",
	"light_patch_wire",
	"light_media",
	"light_dynamics",
	"light_engine",
	"light_headless_runtime",
]);
const RUST_ALLOC = new Set([
	"__rust_alloc",
	"__rust_dealloc",
	"__rust_realloc",
	"__rust_alloc_zeroed",
	"__rdl_alloc",
	"__rdl_dealloc",
	"__rdl_realloc",
	"__rdl_alloc_zeroed",
	"__rg_alloc",
	"__rg_dealloc",
	"__rg_realloc",
	"__rg_alloc_zeroed",
]);
const C_ALLOC = new Set([
	"malloc",
	"free",
	"realloc",
	"calloc",
	"__libc_malloc",
	"__libc_free",
	"__libc_realloc",
	"__libc_calloc",
	"__libc_memalign",
	"_int_malloc",
	"_int_free",
	"_int_realloc",
	"_int_memalign",
	"malloc_consolidate",
	"sysmalloc",
	"systrim",
	"malloc_trim",
	"memalign",
	"aligned_alloc",
	"posix_memalign",
	"tcache_init",
	"tcache_thread_shutdown",
	"unlink_chunk",
]);
const C_MATH = new Set([
	"sin",
	"sinf",
	"cos",
	"cosf",
	"tan",
	"tanf",
	"asin",
	"asinf",
	"acos",
	"acosf",
	"atan",
	"atanf",
	"atan2",
	"atan2f",
	"pow",
	"powf",
	"exp",
	"expf",
	"log",
	"logf",
	"sqrt",
	"sqrtf",
	"floor",
	"floorf",
	"ceil",
	"ceilf",
	"fmod",
	"fmodf",
	"sincos",
	"sincosf",
]);
const C_MEMORY = new Set([
	"memcpy",
	"memmove",
	"memset",
	"memcmp",
	"memchr",
	"strlen",
	"strcmp",
	"strncmp",
	"strcpy",
	"strncpy",
	"bcmp",
	"bzero",
]);
function spellingFamily(symbol, dso) {
	const namespace = /^<?([a-zA-Z_][a-zA-Z0-9_]*)::/u.exec(symbol)?.[1];
	if (PROJECT_NAMES.has(namespace)) return 1;
	if (namespace === "std") return 2;
	if (namespace === "core") return 3;
	if (namespace === "alloc") return 4;
	// Normalize ONLY a terminal hexadecimal perf offset and optional GLIBC version.
	const base = symbol
		.replace(/\+0x[0-9a-f]+$/iu, "")
		.replace(/@@?GLIBC_\d+(?:\.\d+)*$/u, "");
	if (RUST_ALLOC.has(base)) return 5;
	if (namespace === "hashbrown") return 6;
	if (namespace === "rayon" || namespace === "rayon_core") return 7;
	if (namespace === "tokio") return 8;
	if (symbol.includes("::")) return 9;
	// __GI_ is a narrowly recognized glibc alias prefix, not proof of library ownership.
	const cbase = base.replace(/^__GI_/u, "");
	if (C_ALLOC.has(cbase)) return 10;
	if (C_MATH.has(cbase)) return 11;
	if (C_MEMORY.has(cbase)) return 12;
	if (/^lib(?:c|m)\.so(?:\.\d+)*$/u.test(path.posix.basename(dso))) return 13;
	return 0;
}
const familyRow = (id) => ({ id, self_samples: 0, self_period: 0 });
const row = (id) => ({
	id,
	self_samples: 0,
	unresolved_self_samples: 0,
	unresolved_ancestor_frames: 0,
	depth1_self_samples: 0,
	resolved_unmapped_self_samples: 0,
});
const KEYS = [
	"schema_version",
	"function_mapping_version",
	"decoded_bytes",
	"line_count",
	"blank_lines",
	"comment_lines",
	"recognized_header_lines",
	"recognized_frame_lines",
	"rejected_lines",
	"foreign_samples",
	"samples",
	"frame_count",
	"unresolved_self_samples",
	"unresolved_ancestor_frames",
	"unresolved_ancestor_zero_ip_frames",
	"unresolved_ancestor_terminal_frames",
	"depth1_samples",
	"stackless_samples",
	"arithmetic_overflow",
	"cap_exceeded",
	"parser_valid",
	"parity_valid",
	"audit_valid",
	"maps_requested",
	"maps_available",
	"maps_valid",
	"maps_are_sample_time",
	"maps_snapshot_phase",
	"maps_rejected_lines",
	"unresolved_ancestor_exec_mapped",
	"unresolved_ancestor_nonexec_mapped",
	"unresolved_ancestor_unmapped",
	"unresolved_ancestor_mapping_unavailable",
	"unresolved_ancestor_dso_filename_match",
	"unresolved_ancestor_dso_filename_mismatch",
	"unresolved_ancestor_dso_filename_unavailable",
	"classes",
	"resolved_unmapped_self_period",
	"resolved_unmapped_self_families",
];
const ROW_KEYS = Object.keys(row(0));
const FLAGS = [
	"arithmetic_overflow",
	"cap_exceeded",
	"parser_valid",
	"parity_valid",
	"audit_valid",
	"maps_requested",
	"maps_available",
	"maps_valid",
];
function exactKeys(value, keys) {
	return (
		value &&
		typeof value === "object" &&
		!Array.isArray(value) &&
		Object.keys(value).length === keys.length &&
		keys.every((key) => Object.hasOwn(value, key))
	);
}

// Membership describes the BEFORE-CAPTURE snapshot only, never sample-time mappings.
// audit_valid means parsing/parity mechanics only, never performance/completeness.
// Pure private maps parsing. No file identity, build-ID or unwind conclusion follows.
function mappings(text) {
	const result = { valid: 1, rejected: 0, entries: [] };
	if (
		typeof text !== "string" ||
		Buffer.byteLength(text) > CAPS.maps ||
		/[^\x09\x0a\x20-\x7e]/u.test(text)
	)
		return { valid: 0, rejected: 1, entries: [] };
	const lines = text.split("\n");
	if (lines.at(-1) === "") lines.pop();
	if (!lines.length || lines.length > CAPS.mappings)
		return { valid: 0, rejected: 1, entries: [] };
	let previousEnd = 0n;
	for (const line of lines) {
		const match =
			/^([0-9a-f]{1,16})-([0-9a-f]{1,16}) ([r-][w-][x-][ps]) ([0-9a-f]{1,16}) ([0-9a-f]+):([0-9a-f]+) (\d+)(?:[ \t]+(\S.*))?[ \t]*$/u.exec(
				line,
			);
		if (!match || line.length > CAPS.line) {
			result.rejected++;
			continue;
		}
		const start = BigInt(`0x${match[1]}`),
			end = BigInt(`0x${match[2]}`);
		if (
			end <= start ||
			start < previousEnd ||
			BigInt(match[7]) > 0xffffffffffffffffn
		) {
			result.rejected++;
			continue;
		}
		previousEnd = end;
		result.entries.push({
			start,
			end,
			executable: match[3][2] === "x",
			filename: match[8] ?? "",
		});
	}
	result.valid = Number(result.rejected === 0);
	if (!result.valid) result.entries = [];
	return result;
}

export function auditPerfScript(
	text,
	{
		tgid,
		ownedExecutable,
		verifiedRuntimePaths = [],
		mapsText = null,
		expectedReport,
	} = {},
) {
	require_(
		typeof text === "string" &&
			Number.isSafeInteger(tgid) &&
			tgid > 0 &&
			cleanAbsolute(ownedExecutable),
	);
	require_(
		Array.isArray(verifiedRuntimePaths) &&
			verifiedRuntimePaths.length <= 64 &&
			verifiedRuntimePaths.every(cleanAbsolute),
	);
	const runtimes = new Set(verifiedRuntimePaths);
	const report = Object.fromEntries(KEYS.map((key) => [key, 0]));
	report.schema_version = 2;
	report.function_mapping_version = FUNCTION_MAPPING_VERSION;
	report.resolved_unmapped_self_families = Array.from({ length: 14 }, (_, id) =>
		familyRow(id),
	);
	report.classes = Array.from({ length: 8 }, (_, id) => row(id));
	report.decoded_bytes = Buffer.byteLength(text);
	report.maps_requested = Number(mapsText !== null);
	const maps =
		mapsText === null
			? { valid: 0, rejected: 0, entries: [] }
			: mappings(mapsText);
	report.maps_valid = maps.valid;
	report.maps_available = maps.valid;
	report.maps_snapshot_phase = maps.valid;
	report.maps_rejected_lines = maps.rejected;
	let current = null,
		headers = 0;
	function flush() {
		if (!current) return;
		const { pid, frames, period } = current;
		current = null;
		if (pid !== tgid) {
			report.foreign_samples++;
			return;
		}
		report.samples++;
		report.frame_count += frames.length;
		if (!frames.length) report.stackless_samples++;
		if (frames.length === 1) report.depth1_samples++;
		for (const [index, frame] of frames.entries()) {
			const entry =
				report.classes[category(frame.dso, ownedExecutable, runtimes)];
			if (index === 0) {
				entry.self_samples++;
				if (frames.length === 1) entry.depth1_self_samples++;
				if (missing(frame.symbol)) {
					entry.unresolved_self_samples++;
					report.unresolved_self_samples++;
				} else if (functionId(frame.symbol) === 0) {
					entry.resolved_unmapped_self_samples++;
					const family =
						report.resolved_unmapped_self_families[
							spellingFamily(frame.symbol, frame.dso)
						];
					family.self_samples++;
					if (
						!integer(report.resolved_unmapped_self_period + period) ||
						!integer(family.self_period + period)
					)
						report.arithmetic_overflow = 1;
					else {
						family.self_period += period;
						report.resolved_unmapped_self_period += period;
					}
				}
			} else if (missing(frame.symbol)) {
				entry.unresolved_ancestor_frames++;
				report.unresolved_ancestor_frames++;
				if (frame.ip === 0n) report.unresolved_ancestor_zero_ip_frames++;
				if (index === frames.length - 1)
					report.unresolved_ancestor_terminal_frames++;
				if (!maps.valid) {
					report.unresolved_ancestor_mapping_unavailable++;
					report.unresolved_ancestor_dso_filename_unavailable++;
				} else {
					const mapping = maps.entries.find(
						(candidate) =>
							frame.ip >= candidate.start && frame.ip < candidate.end,
					);
					if (!mapping) {
						report.unresolved_ancestor_unmapped++;
						report.unresolved_ancestor_dso_filename_unavailable++;
					} else {
						report[
							mapping.executable
								? "unresolved_ancestor_exec_mapped"
								: "unresolved_ancestor_nonexec_mapped"
						]++;
						if (!mapping.filename || !frame.dso || frame.dso === "[unknown]")
							report.unresolved_ancestor_dso_filename_unavailable++;
						else
							report[
								mapping.filename === frame.dso
									? "unresolved_ancestor_dso_filename_match"
									: "unresolved_ancestor_dso_filename_mismatch"
							]++;
					}
				}
			}
		}
	}
	if (report.decoded_bytes > CAPS.bytes) report.cap_exceeded = 1;
	else {
		let offset = 0;
		while (offset < text.length) {
			const newline = text.indexOf("\n", offset),
				end = newline < 0 ? text.length : newline;
			const line = text.slice(offset, end);
			offset = end + 1;
			report.line_count++;
			if (line.length > CAPS.line) {
				report.rejected_lines++;
				report.cap_exceeded = 1;
				break;
			}
			if (!line.trim()) {
				report.blank_lines++;
				flush();
				continue;
			}
			if (line.startsWith("#")) {
				report.comment_lines++;
				continue;
			}
			const header =
				/^\s*(\d+)\/(\d+)\s+(\d+\.\d{1,9}):\s+(\d+)\s+cpu-clock(?::u)?:\s*(.*)$/u.exec(
					line,
				);
			if (header) {
				flush();
				if (++headers > CAPS.samples) {
					report.rejected_lines++;
					report.cap_exceeded = 1;
					break;
				}
				if (
					![header[1], header[2], header[4]].every((value) =>
						integer(Number(value)),
					) ||
					Number(header[1]) === 0 ||
					Number(header[2]) === 0
				) {
					report.rejected_lines++;
					continue;
				}
				report.recognized_header_lines++;
				current = {
					pid: Number(header[1]),
					period: Number(header[4]),
					frames: [],
				};
				if (!header[5]) continue;
			}
			const frameText = header ? header[5] : line;
			const frame = /^\s*(?:0x)?([0-9a-f]{1,16})\s+(.+)\s+\((.*)\)\s*$/iu.exec(
				frameText,
			);
			if (!current || !frame) {
				report.rejected_lines++;
				continue;
			}
			if (current.frames.length >= CAPS.frames) {
				report.rejected_lines++;
				report.cap_exceeded = 1;
				break;
			}
			report.recognized_frame_lines++;
			current.frames.push({
				ip: BigInt(`0x${frame[1]}`),
				symbol: frame[2].trim(),
				dso: frame[3].trim(),
			});
		}
		flush();
	}
	report.parser_valid = Number(
		report.samples > 0 &&
			!report.arithmetic_overflow &&
			!report.cap_exceeded &&
			!report.rejected_lines &&
			!report.stackless_samples,
	);
	const expected = [
		expectedReport?.samples,
		expectedReport?.foreign_samples,
		expectedReport?.stackless_samples,
		expectedReport?.coverage?.frame_count,
		expectedReport?.coverage?.unresolved_self_samples,
		expectedReport?.coverage?.unresolved_ancestor_frames,
		expectedReport?.coverage?.depths?.[1]?.samples,
	];
	const actual = [
		report.samples,
		report.foreign_samples,
		report.stackless_samples,
		report.frame_count,
		report.unresolved_self_samples,
		report.unresolved_ancestor_frames,
		report.depth1_samples,
	];
	report.parity_valid = Number(
		report.samples > 0 &&
			!report.arithmetic_overflow &&
			!report.cap_exceeded &&
			!report.rejected_lines &&
			expected.every(
				(value, index) => integer(value) && value === actual[index],
			),
	);
	report.audit_valid = Number(
		report.parser_valid === 1 &&
			report.parity_valid === 1 &&
			(!report.maps_requested || report.maps_valid === 1),
	);
	validateAuditReport(report);
	return report;
}

export function validateAuditReport(report) {
	require_(exactKeys(report, KEYS));
	for (const key of KEYS.filter(
		(key) => key !== "classes" && key !== "resolved_unmapped_self_families",
	))
		require_(integer(report[key]));
	require_(
		report.schema_version === 2 &&
			report.function_mapping_version === FUNCTION_MAPPING_VERSION,
	);
	for (const key of FLAGS) require_(report[key] <= 1);
	require_(Array.isArray(report.classes) && report.classes.length === 8);
	for (const [id, entry] of report.classes.entries()) {
		require_(
			exactKeys(entry, ROW_KEYS) &&
				ROW_KEYS.every((key) => integer(entry[key])) &&
				entry.id === id,
		);
		require_(
			entry.unresolved_self_samples + entry.resolved_unmapped_self_samples <=
				entry.self_samples && entry.depth1_self_samples <= entry.self_samples,
		);
	}
	require_(
		Array.isArray(report.resolved_unmapped_self_families) &&
			report.resolved_unmapped_self_families.length === 14,
	);
	for (const [id, entry] of report.resolved_unmapped_self_families.entries())
		require_(
			exactKeys(entry, ["id", "self_samples", "self_period"]) &&
				entry.id === id &&
				Object.values(entry).every(integer) &&
				entry.self_samples <= CAPS.samples &&
				(entry.self_samples > 0 || entry.self_period === 0),
		);
	const familySamples = report.resolved_unmapped_self_families.reduce(
		(total, entry) => total + entry.self_samples,
		0,
	);
	const familyPeriod = report.resolved_unmapped_self_families.reduce(
		(total, entry) => total + entry.self_period,
		0,
	);
	require_(
		integer(familyPeriod) &&
			familyPeriod === report.resolved_unmapped_self_period &&
			familySamples ===
				report.classes.reduce(
					(total, entry) => total + entry.resolved_unmapped_self_samples,
					0,
				),
	);
	const sum = (key) =>
		report.classes.reduce((total, entry) => total + entry[key], 0);
	require_(
		sum("self_samples") === report.samples - report.stackless_samples &&
			sum("unresolved_self_samples") === report.unresolved_self_samples &&
			sum("unresolved_ancestor_frames") === report.unresolved_ancestor_frames &&
			sum("depth1_self_samples") === report.depth1_samples,
	);
	require_(
		report.unresolved_ancestor_exec_mapped +
			report.unresolved_ancestor_nonexec_mapped +
			report.unresolved_ancestor_unmapped +
			report.unresolved_ancestor_mapping_unavailable ===
			report.unresolved_ancestor_frames,
	);
	require_(
		report.unresolved_ancestor_dso_filename_match +
			report.unresolved_ancestor_dso_filename_mismatch +
			report.unresolved_ancestor_dso_filename_unavailable ===
			report.unresolved_ancestor_frames,
	);
	require_(
		report.maps_available === report.maps_valid &&
			report.maps_available <= report.maps_requested &&
			report.maps_are_sample_time === 0 &&
			report.maps_snapshot_phase === report.maps_available,
	);
	require_(
		!report.maps_available ||
			(report.maps_rejected_lines === 0 &&
				report.unresolved_ancestor_mapping_unavailable === 0),
	);
	require_(report.maps_requested || report.maps_rejected_lines === 0);
	require_(
		report.maps_available ||
			report.unresolved_ancestor_dso_filename_unavailable ===
				report.unresolved_ancestor_frames,
	);
	require_(
		report.maps_available ||
			report.unresolved_ancestor_mapping_unavailable ===
				report.unresolved_ancestor_frames,
	);
	require_(
		!report.parser_valid ||
			(report.samples > 0 &&
				!report.arithmetic_overflow &&
				!report.cap_exceeded &&
				!report.rejected_lines &&
				!report.stackless_samples),
	);
	// Parity may match a stackless result; parser_valid/audit_valid still fail it.
	require_(
		!report.parity_valid ||
			(report.samples > 0 &&
				!report.arithmetic_overflow &&
				!report.cap_exceeded &&
				!report.rejected_lines),
	);
	require_(
		report.unresolved_ancestor_zero_ip_frames <=
			report.unresolved_ancestor_frames &&
			report.unresolved_ancestor_terminal_frames <=
				report.unresolved_ancestor_frames &&
			report.unresolved_ancestor_terminal_frames <= report.samples,
	);
	require_(
		report.audit_valid ===
			Number(
				report.parser_valid === 1 &&
					report.parity_valid === 1 &&
					(!report.maps_requested || report.maps_valid === 1),
			),
	);
	require_(
		report.samples + report.foreign_samples <= CAPS.samples &&
			report.frame_count <= CAPS.samples * CAPS.frames &&
			report.depth1_samples <= report.samples &&
			report.stackless_samples <= report.samples,
	);
	require_(
		report.frame_count >=
			report.samples -
				report.stackless_samples +
				report.unresolved_ancestor_frames,
	);
	require_(
		report.recognized_header_lines <= CAPS.samples &&
			report.recognized_frame_lines <= CAPS.samples * CAPS.frames,
	);
	require_(report.line_count <= report.decoded_bytes + 1);
	require_(report.decoded_bytes <= CAPS.bytes || report.cap_exceeded === 1);
	// Header and frame recognition can overlap on a header containing its first frame.
	for (const key of [
		"blank_lines",
		"comment_lines",
		"recognized_header_lines",
		"recognized_frame_lines",
		"rejected_lines",
	])
		require_(report[key] <= report.line_count);
	return report;
}
