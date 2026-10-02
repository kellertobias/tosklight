import type {
	ChannelFunction,
	PhysicalMappingCalibration,
	PhysicalMappingSample,
} from "../../fixtureProfile";

export function emptyPhysicalMapping(): PhysicalMappingCalibration {
	return { quality: "unknown", source: null, revision: 0, samples: [] };
}

export function hasZoomDegreeMapping(fn: ChannelFunction): boolean {
	return (
		fn.attribute === "zoom" &&
		fn.behavior.type === "continuous" &&
		["deg", "degree", "degrees", "°"].includes(
			fn.behavior.unit?.trim().toLowerCase() ?? "",
		)
	);
}

/** Authoring validation mirrors the portable Rust mapping contract, not fixture guesses. */
export function physicalMappingErrors(
	fn: ChannelFunction,
	maximumRaw = 0xffff_ffff,
): string[] {
	const mapping = fn.physical_mapping;
	if (!mapping) return [];
	if (fn.behavior.type !== "continuous")
		return ["Physical mapping requires a continuous function."];
	const errors: string[] = [];
	const quality = mapping.quality ?? "unknown";
	if (!["unknown", "estimated", "manufacturer", "measured"].includes(quality))
		errors.push("Physical mapping quality is invalid.");
	if (
		(quality === "manufacturer" || quality === "measured") &&
		!mapping.source?.trim()
	)
		errors.push("Manufacturer and measured mappings need a source.");
	const revision = mapping.revision ?? 0;
	if (!Number.isInteger(revision) || revision < 0 || revision > 0xffff_ffff)
		errors.push("Physical mapping revision must be a whole number from 0 to 4294967295.");
	if (mapping.opening_convention != null) {
		if (!["beam", "field"].includes(mapping.opening_convention))
			errors.push("Zoom opening convention must be beam or field.");
		if (!hasZoomDegreeMapping(fn))
			errors.push("Opening convention requires a Zoom function with an explicit degree unit.");
	}
	const { physical_min: from, physical_max: to } = fn.behavior;
	if (!Number.isFinite(Math.fround(from)) || !Number.isFinite(Math.fround(to)) || Math.fround(from) === Math.fround(to))
		errors.push("Physical endpoints must be finite and different.");
	if (
		!Number.isInteger(fn.dmx_from) || !Number.isInteger(fn.dmx_to) ||
		fn.dmx_from < 0 || fn.dmx_to > maximumRaw || fn.dmx_from >= fn.dmx_to
	)
		errors.push("A physical mapping needs an increasing raw interval within the channel resolution.");
	const samples = mapping.samples ?? [];
	if (!samples.length) return errors;
	if (samples.length < 2)
		errors.push("A sampled mapping needs at least two samples.");
	const first = samples[0];
	const last = samples[samples.length - 1];
	if (
		first.raw !== fn.dmx_from || Math.fround(first.physical) !== Math.fround(from) ||
		last.raw !== fn.dmx_to || Math.fround(last.physical) !== Math.fround(to)
	)
		errors.push("First and last samples must match the function's raw and physical endpoints.");
	const direction = Math.sign(Math.fround(to) - Math.fround(from));
	for (let index = 0; index < samples.length; index += 1) {
		const sample = samples[index];
		if (
			!Number.isInteger(sample.raw) || sample.raw < fn.dmx_from ||
			sample.raw > fn.dmx_to || !Number.isFinite(Math.fround(sample.physical))
		) {
			errors.push("Samples need whole raw values inside the function range and finite physical values.");
			break;
		}
		if (index > 0) {
			const previous = samples[index - 1];
			if (sample.raw <= previous.raw)
				errors.push("Sample raw values must strictly increase.");
			if ((Math.fround(sample.physical) - Math.fround(previous.physical)) * direction <= 0)
				errors.push("Sample physical values must be strictly monotonic in the endpoint direction.");
		}
	}
	return [...new Set(errors)];
}

export function physicalMappingSamples(fn: ChannelFunction): PhysicalMappingSample[] {
	if (fn.behavior.type !== "continuous") return [];
	return fn.physical_mapping?.samples?.length
		? fn.physical_mapping.samples
		: [
				{ raw: fn.dmx_from, physical: fn.behavior.physical_min },
				{ raw: fn.dmx_to, physical: fn.behavior.physical_max },
			];
}

/** Configuration-only forward preview sharing the portable Rust mapping contract. */
export function previewPhysicalValue(fn: ChannelFunction, raw: number): number | null {
	if (fn.behavior.type !== "continuous" || !Number.isFinite(raw)) return null;
	// Validate the linear fallback as well, without attaching metadata to the profile.
	if (physicalMappingErrors({ ...fn, physical_mapping: fn.physical_mapping ?? emptyPhysicalMapping() }).length)
		return null;
	const samples = physicalMappingSamples(fn);
	const clamped = Math.max(fn.dmx_from, Math.min(fn.dmx_to, raw));
	for (let index = 1; index < samples.length; index += 1) {
		const left = samples[index - 1];
		const right = samples[index];
		if (clamped <= right.raw) {
			const fraction = (clamped - left.raw) / (right.raw - left.raw);
			const leftPhysical = Math.fround(left.physical);
			return leftPhysical + (Math.fround(right.physical) - leftPhysical) * fraction;
		}
	}
	return samples[samples.length - 1]?.physical ?? null;
}

/** Find the widest gap that can store another integer raw / f32 physical sample. */
export function nextPhysicalMappingSample(
	fn: ChannelFunction,
): { index: number; sample: PhysicalMappingSample } | null {
	if (physicalMappingErrors(fn).length) return null;
	const samples = fn.physical_mapping?.samples ?? [];
	let candidate: { index: number; sample: PhysicalMappingSample } | null = null;
	let widestGap = 1;
	for (let index = 1; index < samples.length; index += 1) {
		const left = samples[index - 1];
		const right = samples[index];
		const gap = right.raw - left.raw;
		if (gap <= widestGap) continue;
		const raw = Math.floor((left.raw + right.raw) / 2);
		const fraction = (raw - left.raw) / gap;
		const leftPhysical = Math.fround(left.physical);
		const rightPhysical = Math.fround(right.physical);
		const physical = Math.fround(
			leftPhysical + (rightPhysical - leftPhysical) * fraction,
		);
		// JSON physical values are stored as f32 by Rust. Adjacent f32 endpoints
		// have no representable interior value, even when their raw gap is large.
		if (
			physical <= Math.min(leftPhysical, rightPhysical) ||
			physical >= Math.max(leftPhysical, rightPhysical)
		)
			continue;
		candidate = { index, sample: { raw, physical } };
		widestGap = gap;
	}
	return candidate;
}
