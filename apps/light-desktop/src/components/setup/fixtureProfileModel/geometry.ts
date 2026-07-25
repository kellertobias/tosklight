import type { GeometryGraph } from "../../../api/types";
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
		emitters: headIds.map((headId, index) => ({
			id: uuid(),
			name: headIds.length === 1 ? "Beam" : `Beam ${index + 1}`,
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
