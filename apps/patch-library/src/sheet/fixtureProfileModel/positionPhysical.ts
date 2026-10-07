import type {
	FixtureProfile,
	GeometryGraph,
	Vector3Value,
} from "../../fixtureProfile";
import { modeGeometry } from "./geometry";
const finite = (v: Vector3Value) =>
	[v.x, v.y, v.z].every((n) => Number.isFinite(Math.fround(n)));
const axis = (v: Vector3Value) => finite(v) && Math.hypot(v.x, v.y, v.z) > 1e-9;
function geometryErrors(graph: GeometryGraph): string[] {
	const c = graph.physical_contract;
	if (!c) return [];
	const errors: string[] = [];
	if (
		c.version !== 1 ||
		!graph.nodes.length ||
		graph.nodes.length > 4096 ||
		graph.emitters.length > 4096
	)
		errors.push(
			"Physical geometry requires version 1 and a bounded nonempty graph.",
		);
	if (
		!["unknown", "estimated", "manufacturer", "measured"].includes(
			c.provenance.quality,
		) ||
		!Number.isInteger(c.provenance.revision) ||
		c.provenance.revision < 0 ||
		c.provenance.revision > 0xffff_ffff
	)
		errors.push("Geometry evidence quality or revision is invalid.");
	if (
		new TextEncoder().encode(c.provenance.source ?? "").length > 1024 ||
		(["manufacturer", "measured"].includes(c.provenance.quality) &&
			!c.provenance.source?.trim())
	)
		errors.push(
			"Manufacturer and measured geometry require evidence of at most 1024 bytes.",
		);
	for (const n of graph.nodes) {
		if (
			![
				n.transform.translation,
				n.transform.rotation_degrees,
				n.transform.scale,
				n.pivot,
			].every(finite) ||
			[n.transform.scale.x, n.transform.scale.y, n.transform.scale.z].some(
				(v) => v < 0,
			)
		)
			errors.push(
				`${n.name}: physical transforms must be finite with nonnegative scales.`,
			);
		if (
			n.motion &&
			(!axis(n.motion.axis) ||
				![n.motion.physical_min, n.motion.physical_max].every(Number.isFinite))
		)
			errors.push(`${n.name}: motion needs a finite nonzero axis and limits.`);
	}
	for (const e of graph.emitters)
		if (
			!finite(e.origin) ||
			!finite(e.orientation_degrees) ||
			![e.beam_angle_degrees, e.field_angle_degrees].every(Number.isFinite)
		)
			errors.push(`${e.name}: lens geometry must be finite.`);
	const bracket = c.bracket;
	if (
		bracket.kind === "hinge" &&
		(!graph.nodes.some((n) => n.id === bracket.node_id) ||
			!finite(bracket.pivot) ||
			!axis(bracket.axis))
	)
		errors.push(
			"Bracket requires an existing body node, finite pivot and nonzero axis.",
		);
	const starts = [
		...graph.nodes.filter((n) => n.motion).map((n) => n.id),
		...graph.emitters.map((e) => e.node_id),
	];
	for (const start of starts) {
		let id: string | null = start;
		const seen = new Set<string>();
		while (id) {
			if (seen.has(id)) {
				errors.push("Physical hierarchy contains a cycle.");
				break;
			}
			seen.add(id);
			const n = graph.nodes.find((n) => n.id === id);
			if (!n) {
				errors.push("Physical ancestor is missing.");
				break;
			}
			if (
				[n.transform.scale.x, n.transform.scale.y, n.transform.scale.z].some(
					(s) => s !== 0 && s !== 1,
				)
			)
				errors.push(
					`${n.name}: physical paths need identity scale; bake visual scale into geometry.`,
				);
			id = n.parent_id;
		}
	}
	return errors;
}
export function positionPhysicalErrors(profile: FixtureProfile): string[] {
	const errors = profile.geometry ? geometryErrors(profile.geometry) : [];
	for (const mode of profile.modes) {
		if (mode.geometry.nodes.length)
			errors.push(...geometryErrors(mode.geometry));
		const m = mode.position_physical;
		if (!m) continue;
		const graph = modeGeometry(profile, mode);
		if (
			m.version !== 1 ||
			!Number.isInteger(m.revision) ||
			m.revision < 0 ||
			m.revision > 0xffff_ffff ||
			!m.bindings.length ||
			m.bindings.length > 4096 ||
			!graph.physical_contract
		)
			errors.push(
				`${mode.name}: physical Position requires version 1, a revision, exact bindings and a geometry declaration.`,
			);
		const seen = new Set<string>(),
			roles = new Map<string, string>(),
			drivers = new Map<string, string>();
		for (const b of m.bindings) {
			const node = graph.nodes.find((n) => n.id === b.node_id),
				channel = mode.channels.find((c) => c.id === b.channel_id),
				fn = channel?.functions.find((f) => f.id === b.function_id);
			if (
				node?.motion?.kind !== "rotation" ||
				!channel ||
				channel.behavior === "static" ||
				!fn?.angular_motion ||
				fn.behavior.type !== "continuous"
			) {
				errors.push(
					`${mode.name}: Position binding needs a rotational node and controllable continuous angular function.`,
				);
				continue;
			}
			const key = `${b.node_id}:${b.channel_id}:${b.function_id}`;
			if (seen.has(key))
				errors.push(`${mode.name}: duplicate Position binding.`);
			seen.add(key);
			if (roles.has(b.node_id) && roles.get(b.node_id) !== b.role)
				errors.push(`${node.name}: one axis cannot be both Pan and Tilt.`);
			roles.set(b.node_id, b.role);
			const velocity = fn.angular_motion.kind === "angular_velocity",
				unit = fn.behavior.unit?.trim().toLowerCase();
			if (
				!(
					velocity
						? ["deg/s", "degree/s", "degrees/s", "degrees per second", "°/s"]
						: ["deg", "degree", "degrees", "°"]
				).includes(unit ?? "")
			)
				errors.push(`${fn.name}: unit disagrees with angular motion kind.`);
			const driver = `${b.node_id}:${velocity}`;
			if (drivers.has(driver) && drivers.get(driver) !== channel.id)
				errors.push(
					`${node.name}: only one channel per angular motion kind is supported.`,
				);
			drivers.set(driver, channel.id);
			if (!mode.heads.some((h) => h.id === channel.head_id && h.master_shared))
				for (const e of graph.emitters) {
					let id: string | null = e.node_id;
					const path = new Set<string>();
					while (id && !path.has(id)) {
						path.add(id);
						if (id === node.id && e.head_id && e.head_id !== channel.head_id) {
							errors.push(
								`${node.name}: cross-head motion requires a shared channel head.`,
							);
							break;
						}
						id = graph.nodes.find((n) => n.id === id)?.parent_id ?? null;
					}
				}
		}
	}
	return [...new Set(errors)];
}
