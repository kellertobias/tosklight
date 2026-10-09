import type { FixtureMode, FixtureProfile, GeometryGraph } from "../../wire";
import { uuid } from "./utilities";

export type GeometryTemplateName =
	| "fixed"
	| "moving_head"
	| "bar"
	| "matrix"
	| "shared_pan_multi_head";

const vector = (value = 0) => ({ x: value, y: value, z: value });

function geometryNode(
	id: string,
	name: string,
	parentId: string | null,
	motion: GeometryGraph["nodes"][number]["motion"] = null,
): GeometryGraph["nodes"][number] {
	return {
		id,
		name,
		parent_id: parentId,
		transform: {
			translation: vector(),
			rotation_degrees: vector(),
			scale: vector(1),
		},
		pivot: vector(),
		glb_node: null,
		motion,
	};
}

export function blankGeometry(headIds: string[] = []): GeometryGraph {
	const root = uuid();
	return {
		nodes: [geometryNode(root, "Chassis", null)],
		emitters: (headIds.length ? headIds : [null]).map((headId, index) => ({
			id: uuid(),
			name: headIds.length > 1 ? `Beam ${index + 1}` : "Beam",
			node_id: root,
			head_id: headId,
			origin: vector(),
			orientation_degrees: vector(),
			beam_angle_degrees: 20,
			field_angle_degrees: 24,
			feather: 0,
			focus: 1,
			directional: true,
			layout: { type: "point" as const },
		})),
	};
}

function addMovingHeadNodes(
	graph: GeometryGraph,
	root: string,
	headIds: string[],
) {
	const pan = uuid();
	graph.nodes.push(
		geometryNode(pan, "Pan arm", root, {
			attribute: "pan",
			kind: "rotation",
			axis: { x: 0, y: 1, z: 0 },
			physical_min: -270,
			physical_max: 270,
		}),
	);
	return headIds.map((_, index) => {
		const tilt = uuid();
		graph.nodes.push(
			geometryNode(
				tilt,
				headIds.length === 1 ? "Tilt head" : `Tilt head ${index + 1}`,
				pan,
				{
					attribute: "tilt",
					kind: "rotation",
					axis: { x: 1, y: 0, z: 0 },
					physical_min: -135,
					physical_max: 135,
				},
			),
		);
		return tilt;
	});
}

function emitterLayout(template: GeometryTemplateName) {
	if (template === "bar") {
		return { type: "strip" as const, count: 8, spacing_millimetres: 50 };
	}
	if (template === "matrix") {
		return {
			type: "matrix" as const,
			columns: 4,
			rows: 4,
			spacing: { x: 50, y: 50, z: 0 },
		};
	}
	return { type: "point" as const };
}

export function geometryTemplate(
	template: GeometryTemplateName,
	headIds: string[],
): GeometryGraph {
	const graph = blankGeometry([]);
	const root = graph.nodes[0].id;
	const moving =
		template === "moving_head" || template === "shared_pan_multi_head";
	const emitterParents = moving
		? addMovingHeadNodes(graph, root, headIds)
		: headIds.map(() => root);
	graph.emitters = headIds.map((headId, index) => ({
		id: uuid(),
		name: headIds.length === 1 ? "Beam" : `Beam ${index + 1}`,
		node_id: emitterParents[index],
		head_id: headId,
		origin: vector(),
		orientation_degrees: vector(),
		beam_angle_degrees: 20,
		field_angle_degrees: 24,
		feather: 0,
		focus: 1,
		directional: template !== "bar" && template !== "matrix",
		layout: emitterLayout(template),
	}));
	return graph;
}

/**
 * This mode's geometry: the fixture's own, with its heads bound to the emitters.
 *
 * The counterpart of `FixtureProfile::mode_geometry` in the fixture crate, and the only place that
 * has to know whether a profile has been lifted or still carries a graph on each mode. An emitter
 * no head owns is not lit in that mode, so it is left out rather than drawn dark.
 */
export function modeGeometry(
	profile: Pick<FixtureProfile, "geometry">,
	mode: Partial<
		Pick<FixtureMode, "geometry" | "emitter_heads" | "motion_attributes">
	>,
): GeometryGraph {
	// A mode that still carries its own graph is one the lift left alone, and its graph is the
	// more specific statement. After a lift the mode's graph is empty and this is the fixture's.
	// Both sides are read from stored show data, which may predate either field.
	const own = {
        physical_contract: mode.geometry?.physical_contract,
		nodes: mode.geometry?.nodes ?? [],
		emitters: mode.geometry?.emitters ?? [],
	};
	const fixture = profile.geometry;
	if (own.nodes.length || !fixture?.nodes?.length) return own;
	const heads = new Map(
		(mode.emitter_heads ?? []).map((binding) => [
			binding.emitter_id,
			binding.head_id,
		]),
	);
	// A moving part is driven by what this mode binds to it; one it does not bind has no attribute
	// and rests at its centre, as an emitter no head owns is not lit.
	const drives = new Map(
		(mode.motion_attributes ?? []).map((binding) => [
			binding.node_id,
			binding.attribute,
		]),
	);
	return {
		...fixture,
		nodes: fixture.nodes.map((node) =>
			node.motion
				? { ...node, motion: { ...node.motion, attribute: drives.get(node.id) ?? null } }
				: node,
		),
		emitters: fixture.emitters
			.filter((emitter) => heads.has(emitter.id))
			.map((emitter) => ({ ...emitter, head_id: heads.get(emitter.id) })),
	};
}

/** The mode as the Stage reads it: its channels, with the fixture's geometry bound to its heads. */
export function modeWithBoundGeometry<
	T extends Partial<
		Pick<FixtureMode, "geometry" | "emitter_heads" | "motion_attributes">
	>,
>(profile: Pick<FixtureProfile, "geometry">, mode: T): T {
	return { ...mode, geometry: modeGeometry(profile, mode) };
}

/**
 * Moves an attribute still written on one of the fixture's moving parts into every mode.
 *
 * Profiles written before modes bound their moving parts, and parts a template has just built,
 * name the driving attribute on the part itself. Each mode that does not already bind the part is
 * given that attribute, and the part stops carrying it — the counterpart of the read-time migration
 * in the fixture crate, so the editor shows and saves what the desk would read.
 */
export function liftMotionAttributes<T extends Pick<FixtureProfile, "geometry" | "modes">>(
	profile: T,
): T {
	const nodes = profile.geometry?.nodes ?? [];
	const named = nodes.filter((node) => node.motion?.attribute);
	if (!named.length || !profile.geometry) return profile;
	return {
		...profile,
		geometry: {
			...profile.geometry,
			nodes: nodes.map((node) =>
				node.motion?.attribute
					? { ...node, motion: { ...node.motion, attribute: null } }
					: node,
			),
		},
		modes: profile.modes.map((mode) => {
			const bound = new Set((mode.motion_attributes ?? []).map((binding) => binding.node_id));
			const added = named
				.filter((node) => !bound.has(node.id))
				.map((node) => ({ node_id: node.id, attribute: node.motion?.attribute ?? "" }));
			return added.length
				? { ...mode, motion_attributes: [...(mode.motion_attributes ?? []), ...added] }
				: mode;
		}),
	};
}

/** Move template-only geometry owners into the mode-owned binding contract. */
export function liftGeometryBindings<
	T extends Pick<FixtureProfile, "geometry" | "modes">,
>(profile: T): T {
	const geometry = profile.geometry;
	if (!geometry) return profile;
	const named = geometry.emitters.filter((emitter) => emitter.head_id);
	const emitterIds = new Set(geometry.emitters.map((emitter) => emitter.id));
	const nodeIds = new Set(geometry.nodes.map((node) => node.id));
	const source = new Map(
		named.flatMap((emitter) => {
			const sourceMode = profile.modes.find((mode) =>
				mode.heads.some((head) => head.id === emitter.head_id),
			);
			const head = sourceMode?.heads.find(
				(head) => head.id === emitter.head_id,
			);
			return head
				? [
						[
							emitter.id,
							{
								head,
								unique:
									sourceMode!.heads.filter(
										(candidate) => candidate.name === head.name,
									).length === 1,
							},
						] as const,
					]
				: [];
		}),
	);
	let changed = source.size > 0;
	const modes = profile.modes.map((mode) => {
		// A legacy mode's own graph is retained, including bindings into that graph.
		const ownEmitters = new Set(
			mode.geometry.emitters.map((emitter) => emitter.id),
		);
		const ownNodes = new Set(mode.geometry.nodes.map((node) => node.id));
		const kept = (mode.emitter_heads ?? []).filter(
			(binding) =>
				emitterIds.has(binding.emitter_id) ||
				ownEmitters.has(binding.emitter_id),
		);
		const motions = (mode.motion_attributes ?? []).filter(
			(binding) =>
				nodeIds.has(binding.node_id) || ownNodes.has(binding.node_id),
		);
		const bound = new Set(kept.map((binding) => binding.emitter_id));
		const added = named.flatMap((emitter) => {
			if (bound.has(emitter.id)) return [];
			const owner = source.get(emitter.id);
			if (!owner) return [];
			const exact = mode.heads.find((head) => head.id === owner.head.id);
			const matches = mode.heads.filter(
				(head) => head.name === owner.head.name,
			);
			const head =
				exact ??
				(owner.unique && matches.length === 1 ? matches[0] : undefined);
			// No index fallback: an unmatched personality is explicitly not driven.
			return head ? [{ emitter_id: emitter.id, head_id: head.id }] : [];
		});
		const modeChanged =
			added.length > 0 ||
			kept.length !== (mode.emitter_heads?.length ?? 0) ||
			motions.length !== (mode.motion_attributes?.length ?? 0);
		changed ||= modeChanged;
		return modeChanged
			? {
					...mode,
					emitter_heads: [...kept, ...added],
					motion_attributes: motions,
				}
			: mode;
	});
	return liftMotionAttributes(
		changed
			? {
					...profile,
					geometry: {
						...geometry,
						emitters: geometry.emitters.map((emitter) =>
							source.has(emitter.id) ? { ...emitter, head_id: null } : emitter,
						),
					},
					modes,
				}
			: profile,
	);
}
