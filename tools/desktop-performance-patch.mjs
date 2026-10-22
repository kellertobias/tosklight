/** Count the real v2 Patch output addresses, rather than a requested scene or semantic lanes. */
export function countPerformancePatch(patch) {
	if (
		!Array.isArray(patch?.fixtures) ||
		!Array.isArray(patch?.profile_revisions)
	)
		throw new Error(
			"Performance Patch is missing fixtures or immutable profile revisions",
		);
	const profiles = new Map(
		patch.profile_revisions.map((profile) => [
			`${profile.profile_id}:${profile.profile_revision}`,
			profile,
		]),
	);
	const slots = new Set();
	const universes = new Set();
	let physicalInstances = 0;
	for (const fixture of patch.fixtures) {
		const profile = profiles.get(
			`${fixture.profile_id}:${fixture.profile_revision}`,
		);
		const mode =
			profile?.referenced_modes?.find(
				(mode) => mode.mode_id === fixture.mode_id,
			) ??
			profile?.profile_snapshot?.modes?.find(
				(mode) => mode.id === fixture.mode_id,
			);
		if (!mode || !Array.isArray(mode.splits))
			throw new Error(
				`Performance cannot resolve ${fixture.name ?? fixture.fixture_id} profile mode ${fixture.mode_id}`,
			);
		const footprints = new Map(
			mode.splits.map((split) => {
				if (
					!Number.isInteger(split.footprint) ||
					split.footprint < 0 ||
					split.footprint > 512
				)
					throw new Error(
						`Invalid performance footprint for ${fixture.fixture_id}`,
					);
				return [split.split ?? split.number, split.footprint];
			}),
		);
		for (const instance of [fixture, ...(fixture.multipatch ?? [])]) {
			physicalInstances++;
			const assignedSplits = new Set();
			for (const assignment of instance.split_patches ?? []) {
				if (
					assignedSplits.has(assignment.split) ||
					!footprints.has(assignment.split)
				)
					throw new Error(
						`Invalid performance split assignment for ${fixture.fixture_id}`,
					);
				assignedSplits.add(assignment.split);
				const footprint = footprints.get(assignment.split);
				if (
					footprint === 0 ||
					(assignment.universe == null && assignment.address == null)
				)
					continue;
				if (
					!Number.isInteger(assignment.universe) ||
					assignment.universe < 0 ||
					!Number.isInteger(assignment.address) ||
					assignment.address < 1 ||
					assignment.address + footprint > 513
				)
					throw new Error(
						`Invalid performance DMX address for ${fixture.fixture_id}`,
					);
				universes.add(assignment.universe);
				for (let offset = 0; offset < footprint; offset++)
					slots.add(`${assignment.universe}:${assignment.address + offset}`);
			}
		}
	}
	return {
		fixtureRecords: patch.fixtures.length,
		physicalInstances,
		parameterCount: slots.size,
		universes: [...universes].sort((left, right) => left - right),
	};
}

export function summarizePerformancePatch(patch, plan) {
	const counts = countPerformancePatch(patch);
	return {
		...counts,
		universes: counts.universes.length,
		animatedAttributes: plan.dynamicTargetCount,
		masterLanes: plan.activations.length,
	};
}

const OUTPUT_COUNTERS = [
	"frames_sent",
	"packets_sent",
	"deadline_misses",
	"send_errors",
];
const unsigned = (value) => Number.isSafeInteger(value) && value >= 0;

/** Port of the 576 clock contract, without its fixed 61-observation requirement. */
export function readPerformanceCounterSample(
	diagnostics,
	bounds,
	expectedRateHz,
) {
	if (
		expectedRateHz !== undefined &&
		diagnostics?.output?.frame_hz !== expectedRateHz
	)
		throw new Error(
			`Performance scheduler reports ${diagnostics?.output?.frame_hz} Hz instead of verified ${expectedRateHz} Hz`,
		);
	const sample = diagnostics?.output_sample;
	if (
		!sample ||
		!unsigned(sample.monotonic_micros) ||
		!unsigned(sample.sampled_at_epoch_micros)
	)
		throw new Error(
			"Performance diagnostics lack a same-lock server counter timestamp",
		);
	if (
		sample.sampled_at_epoch_micros < bounds.utcStartMillis * 1000 ||
		sample.sampled_at_epoch_micros >= (bounds.utcEndMillis + 1) * 1000
	)
		throw new Error(
			"Performance server counter timestamp falls outside its HTTP bounds",
		);
	const counters = Object.fromEntries(
		OUTPUT_COUNTERS.map((name) => [name, diagnostics.output?.[name]]),
	);
	if (!Object.values(counters).every(unsigned))
		throw new Error("Performance diagnostics contain invalid output counters");
	return { ...sample, counters, bounds };
}

export function performanceCounterInterval(before, after) {
	const micros = after.monotonic_micros - before.monotonic_micros;
	const lower =
		(after.bounds.monotonicStartMillis - before.bounds.monotonicEndMillis) *
			1000 -
		2;
	const upper =
		(after.bounds.monotonicEndMillis - before.bounds.monotonicStartMillis) *
			1000 +
		2;
	if (micros <= 0 || micros < lower || micros > upper)
		throw new Error(
			"Performance server counter time is nonmonotonic or outside the HTTP envelope",
		);
	if (
		Math.abs(
			after.sampled_at_epoch_micros - before.sampled_at_epoch_micros - micros,
		) > 1000
	)
		throw new Error("Performance server UTC and monotonic clocks diverged");
	const deltas = Object.fromEntries(
		OUTPUT_COUNTERS.map((name) => [
			name,
			after.counters[name] - before.counters[name],
		]),
	);
	if (!Object.values(deltas).every(unsigned))
		throw new Error("Performance output counter regressed during measurement");
	return { seconds: micros / 1e6, ...deltas };
}

export function requireNoPerformanceRenderer(processes) {
	if (processes.some((process) => process.command.includes("viz-renderer")))
		throw new Error(
			"Control performance baseline unexpectedly started viz-renderer",
		);
}
