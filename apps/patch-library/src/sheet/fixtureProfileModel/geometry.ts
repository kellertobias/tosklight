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
	mode: Partial<Pick<FixtureMode, "geometry" | "emitter_heads">>,
): GeometryGraph {
	// A mode that still carries its own graph is one the lift left alone, and its graph is the
	// more specific statement. After a lift the mode's graph is empty and this is the fixture's.
	// Both sides are read from stored show data, which may predate either field.
	const own = {
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
	return {
		...fixture,
		emitters: fixture.emitters
			.filter((emitter) => heads.has(emitter.id))
			.map((emitter) => ({ ...emitter, head_id: heads.get(emitter.id) })),
	};
}

/** The mode as the Stage reads it: its channels, with the fixture's geometry bound to its heads. */
export function modeWithBoundGeometry<
	T extends Partial<Pick<FixtureMode, "geometry" | "emitter_heads">>,
>(profile: Pick<FixtureProfile, "geometry">, mode: T): T {
	return { ...mode, geometry: modeGeometry(profile, mode) };
}
