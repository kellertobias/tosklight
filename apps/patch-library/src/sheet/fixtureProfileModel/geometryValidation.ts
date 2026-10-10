import type { FixtureProfile, GeometryGraph } from "../../wire";

export function geometryErrors(profile: FixtureProfile): string[] {
	const errors: string[] = [];
	const check = (
		geometry: GeometryGraph,
		label: string,
		heads: Set<string> | null,
	) => {
		const nodes = new Set(geometry.nodes.map((node) => node.id));
		const ids = new Set<string>();
		for (const emitter of geometry.emitters) {
			const name = `${label}: emitter "${emitter.name || emitter.id}"`;
			if (ids.has(emitter.id))
				errors.push(
					`${name}: emitter identity is duplicated; remove the duplicate emitter.`,
				);
			ids.add(emitter.id);
			if (!nodes.has(emitter.node_id))
				errors.push(
					`${name}: Geometry part is missing; choose an existing part.`,
				);
			if (emitter.head_id && (!heads || !heads.has(emitter.head_id)))
				errors.push(
					`${name}: Logical head must be assigned to an existing head per mode under Modes → Emitters & Motion.`,
				);
			if (
				!Number.isFinite(emitter.beam_angle_degrees) ||
				emitter.beam_angle_degrees < 0
			)
				errors.push(`${name}: Beam angle must be a finite nonnegative angle.`);
			if (
				!Number.isFinite(emitter.field_angle_degrees) ||
				emitter.field_angle_degrees < emitter.beam_angle_degrees
			)
				errors.push(
					`${name}: Field angle must be finite and at least the Beam angle.`,
				);
		}
	};
	const shared = profile.geometry ?? { nodes: [], emitters: [] };
	check(shared, "Geometry", null);
	const sharedIds = new Set(shared.emitters.map((emitter) => emitter.id));
	for (const mode of profile.modes) {
		const heads = new Set(mode.heads.map((head) => head.id));
		check(mode.geometry, mode.name, heads);
		const seen = new Set<string>();
		for (const binding of mode.emitter_heads ?? []) {
			const emitter = shared.emitters.find(
				(emitter) => emitter.id === binding.emitter_id,
			);
			const name = `${mode.name}: emitter "${emitter?.name || binding.emitter_id}"`;
			if (!sharedIds.has(binding.emitter_id))
				errors.push(
					`${name}: emitter no longer exists in Geometry; remove its mode binding.`,
				);
			if (seen.has(binding.emitter_id))
				errors.push(
					`${name}: Logical head is assigned more than once; keep one owner.`,
				);
			seen.add(binding.emitter_id);
			if (!heads.has(binding.head_id))
				errors.push(
					`${name}: Logical head is missing; select an existing head under Emitters & Motion.`,
				);
		}
	}
	return errors;
}
