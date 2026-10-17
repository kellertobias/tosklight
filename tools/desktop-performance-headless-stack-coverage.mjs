import path from "node:path";

export const FUNCTION_MAPPING_VERSION = 2;
const FRAME_LIMIT = 128;
const SAMPLE_LIMIT = 10000;
// Fixed depth bins: 0, 1, 2, 3-4, 5-8, 9-16, 17-32, 33-64, 65-128.
const DEPTH_MAXIMUM = [0, 1, 2, 4, 8, 16, 32, 64, FRAME_LIMIT];
const require_ = (condition, message) => {
	if (!condition) throw new Error(`Headless coverage unavailable: ${message}`);
};
const uint = (value) => {
	require_(Number.isSafeInteger(value) && value >= 0, "numeric bound");
	return value;
};
const add = (a, b) => uint(a + b);
const sum = (values) => values.reduce(add, 0);
const increment = (object, key, value = 1) => {
	object[key] = add(object[key], value);
};
const unresolved = (symbol) => /^(?:\[unknown\]|0x[0-9a-f]+)$/iu.test(symbol);
// Fixed function IDs; unknown=0. No symbol, DSO, address or process identifier is public.
// Inclusive rows overlap and are not summable. Recorder wall bounds do not join samples to frames.
const FUNCTIONS = [
	[1, /light_dynamics::programming::preparation::.*prepare_owner_sample/],
	[
		2,
		/light_dynamics::programming::preparation::.*prepare_dynamic_family_samples/,
	],
	[3, /light_dynamics::programming::.*complete_deferred/],
	[4, /light_headless_runtime::.*hybrid::staged::.*prepare_captured/],
	[
		5,
		/light_headless_runtime::.*family_inputs::assemble_captured_family_inputs/,
	],
	[6, /light_headless_runtime::.*family_lanes::.*finish/],
	[7, /light_dynamics::programming::composition::.*compose/],
	[8, /light_dynamics::programming::.*classify/],
	[9, /light_dynamics::programming::.*compile/],
	[10, /light_engine::.*prepare_families/],
	[11, /rayon_core::.*(?:join|execute|steal|wait)/],
	[12, /^(?:__libc_)?malloc(?:$|\+|@)/],
	[13, /^(?:__libc_)?free(?:$|\+|@)/],
	[14, /^(?:__libc_)?realloc(?:$|\+|@)/],
	[15, /^(?:__)?memcpy(?:$|\+|@|_)/],
	[16, /^(?:__)?memmove(?:$|\+|@|_)/],
	[17, /light_headless_runtime::.*output_scheduler::.*tick/],
	// Broad classes are last: IDs0..17 and their matching priority remain unchanged.
	[18, /^<?light_dynamics::/],
	[19, /^<?light_headless_runtime::/],
	[20, /^<?light_engine::/],
	[21, /^<?alloc::(?:collections|raw_vec|vec|alloc)::/],
	[22, /^<?core::(?:clone::|ptr::drop_in_place|ops::drop::)/],
	[23, /^<?hashbrown::/],
	[24, /^<?(?:rayon|rayon_core)::/],
	[25, /^<?tokio::/],
];
export function functionId(symbol) {
	const normalized = symbol.replace(/\[[0-9a-f]+\]/gu, "");
	return FUNCTIONS.find(([, pattern]) => pattern.test(normalized))?.[0] ?? 0;
}

export function functionRows() {
	return Array.from({ length: FUNCTIONS.length + 1 }, (_, id) => ({
		id,
		self_samples: 0,
		self_period: 0,
		inclusive_samples: 0,
		inclusive_period: 0,
	}));
}
export function emptyCoverage() {
	return {
		frame_count: 0,
		unresolved_self_samples: 0,
		unresolved_ancestor_frames: 0,
		resolved_unmapped_self_samples: 0,
		samples_with_unresolved: 0,
		unresolved_self_only_samples: 0,
		unresolved_ancestor_only_samples: 0,
		unresolved_both_samples: 0,
		dsos: Array.from({ length: 5 }, (_, id) => ({
			id,
			unresolved_self_samples: 0,
			unresolved_ancestor_frames: 0,
			resolved_unmapped_self_samples: 0,
		})),
		depths: DEPTH_MAXIMUM.map((_, id) => ({ id, samples: 0 })),
	};
}
// DSO IDs: 0 missing/unknown; 1 exact verified executable path; 2 observed standard
// runtime-library label; 3 other absolute DSO; 4 anonymous/nonabsolute label.
// IDs2..4 imply no ownership. Never infer executable ownership from its basename.
export function dsoId(dso, ownedExecutable = null) {
	if (!dso || dso === "[unknown]") return 0;
	if (
		ownedExecutable !== null &&
		path.isAbsolute(ownedExecutable) &&
		dso === ownedExecutable
	)
		return 1;
	const basename = path.basename(dso);
	if (
		/^(?:(?:libc|libm|libpthread|libdl|librt)\.so(?:\.\d+)*|ld-linux(?:-[a-z0-9_-]+)?\.so(?:\.\d+)*)$/u.test(
			basename,
		)
	)
		return 2;
	return path.isAbsolute(dso) ? 3 : 4;
}
// Symbols and DSO paths exist only in this bounded transient input; all output is numeric.
export function observeCoverage(
	report,
	frames,
	period,
	ownedExecutable = null,
) {
	require_(
		Array.isArray(frames) && frames.length <= FRAME_LIMIT,
		"frame bound",
	);
	uint(period);
	const coverage = report.coverage;
	increment(coverage, "frame_count", frames.length);
	increment(
		coverage.depths[DEPTH_MAXIMUM.findIndex((max) => frames.length <= max)],
		"samples",
	);
	const ids = [];
	let resolvedFrames = 0,
		unresolvedSelf = false,
		unresolvedAncestors = 0;
	for (const [index, frame] of frames.entries()) {
		require_(
			typeof frame.symbol === "string" &&
				typeof frame.dso === "string" &&
				frame.symbol.length <= 4096 &&
				frame.dso.length <= 4096,
			"private frame shape",
		);
		const id = functionId(frame.symbol),
			missing = unresolved(frame.symbol),
			dso = coverage.dsos[dsoId(frame.dso, ownedExecutable)];
		ids.push(id);
		if (missing) {
			increment(report, "unresolved_frames");
			if (index === 0) {
				unresolvedSelf = true;
				increment(coverage, "unresolved_self_samples");
				increment(dso, "unresolved_self_samples");
			} else {
				unresolvedAncestors++;
				increment(coverage, "unresolved_ancestor_frames");
				increment(dso, "unresolved_ancestor_frames");
			}
		} else {
			resolvedFrames++;
			if (index === 0 && id === 0) {
				increment(coverage, "resolved_unmapped_self_samples");
				increment(dso, "resolved_unmapped_self_samples");
			}
		}
	}
	if (unresolvedSelf || unresolvedAncestors) {
		increment(coverage, "samples_with_unresolved");
		increment(
			coverage,
			unresolvedSelf
				? unresolvedAncestors
					? "unresolved_both_samples"
					: "unresolved_self_only_samples"
				: "unresolved_ancestor_only_samples",
		);
	}
	const self = ids[0] ?? 0;
	if (self === 0) increment(report, "unknown_self_samples");
	increment(report.functions[self], "self_samples");
	increment(report.functions[self], "self_period", period);
	for (const id of new Set(ids)) {
		increment(report.functions[id], "inclusive_samples");
		increment(report.functions[id], "inclusive_period", period);
	}
	return resolvedFrames;
}
function exact(object, keys) {
	require_(
		object &&
			typeof object === "object" &&
			!Array.isArray(object) &&
			Object.keys(object).length === keys.length &&
			keys.every((key) => Object.hasOwn(object, key)),
		"unknown coverage field",
	);
}
function rows(actual, reference) {
	require_(
		Array.isArray(actual) && actual.length === reference.length,
		"coverage row count",
	);
	for (const [index, row] of actual.entries()) {
		exact(row, Object.keys(reference[index]));
		require_(row.id === index, "coverage row identity");
		for (const value of Object.values(row)) uint(value);
	}
}
export function validateCoverage(report) {
	require_(
		!report.capture_complete || report.unresolved_frames === 0,
		"complete capture contains unresolved frames",
	);
	const c = report.coverage,
		baseline = emptyCoverage();
	exact(c, Object.keys(baseline));
	for (const [key, value] of Object.entries(c))
		if (!["dsos", "depths"].includes(key)) uint(value);
	rows(c.dsos, baseline.dsos);
	rows(c.depths, baseline.depths);
	const samples = uint(report.samples),
		frames = uint(c.frame_count);
	require_(
		samples <= SAMPLE_LIMIT && frames <= samples * FRAME_LIMIT,
		"coverage count cap",
	);
	require_(
		sum(c.depths.map((row) => row.samples)) === samples &&
			c.depths[0].samples === report.stackless_samples &&
			samples - c.depths[0].samples - c.depths[1].samples ===
				report.multi_frame_samples,
		"depth sample identity",
	);
	const minimum = sum(
		c.depths.map(
			(row, index) =>
				row.samples * (index === 0 ? 0 : DEPTH_MAXIMUM[index - 1] + 1),
		),
	);
	const maximum = sum(
		c.depths.map((row, index) => row.samples * DEPTH_MAXIMUM[index]),
	);
	require_(frames >= minimum && frames <= maximum, "depth frame bound");
	require_(
		sum([c.unresolved_self_samples, c.unresolved_ancestor_frames]) ===
			report.unresolved_frames && report.unresolved_frames <= frames,
		"unresolved frame identity",
	);
	require_(
		c.unresolved_self_samples <= samples - report.stackless_samples &&
			c.unresolved_ancestor_frames <=
				frames - (samples - report.stackless_samples),
		"self ancestor bound",
	);
	require_(
		sum([
			c.unresolved_self_samples,
			c.resolved_unmapped_self_samples,
			report.stackless_samples,
		]) === report.unknown_self_samples &&
			report.functions[0].self_samples === report.unknown_self_samples,
		"unknown self identity",
	);
	require_(
		sum([
			c.unresolved_self_only_samples,
			c.unresolved_ancestor_only_samples,
			c.unresolved_both_samples,
		]) === c.samples_with_unresolved &&
			c.samples_with_unresolved <= samples - report.stackless_samples,
		"unresolved sample partition",
	);
	require_(
		sum([c.unresolved_self_only_samples, c.unresolved_both_samples]) ===
			c.unresolved_self_samples &&
			sum([c.unresolved_ancestor_only_samples, c.unresolved_both_samples]) <=
				c.unresolved_ancestor_frames,
		"unresolved position partition",
	);

	const ancestorSamples = sum([
		c.unresolved_ancestor_only_samples,
		c.unresolved_both_samples,
	]);
	require_(
		ancestorSamples <= report.multi_frame_samples &&
			c.unresolved_ancestor_frames <= ancestorSamples * (FRAME_LIMIT - 1),
		"ancestor sample bound",
	);
	for (const key of [
		"unresolved_self_samples",
		"unresolved_ancestor_frames",
		"resolved_unmapped_self_samples",
	])
		require_(
			sum(c.dsos.map((row) => row[key])) === c[key],
			"DSO count identity",
		);
}
