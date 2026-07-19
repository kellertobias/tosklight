import * as THREE from "three";
import type {
	FixtureMode,
	GeometryEmitter,
	PatchedFixture,
	VisualizationSnapshot,
} from "../../api/types";
import {
	attributesForHead,
	channelDefault,
	headOwnerId,
	normalized,
	resolvedColor,
} from "./attributeValues";
import { buildGeometryBeam } from "./emitterGeometry";
import { addSelectionOutline, millimetres } from "./sceneObjects";
import type { FixtureAttributeValues, FixtureValuesById } from "./types";

type GeometryGraph = FixtureMode["geometry"];
type GeometryNode = GeometryGraph["nodes"][number];
type GeometryNodeParts = { group: THREE.Group; anchor: THREE.Group };

export type ProfileGeometryOptions = {
	fixture: PatchedFixture;
	mode: FixtureMode;
	byFixture: FixtureValuesById;
	selected: boolean;
	snapshot: VisualizationSnapshot | null;
	projectedOwners: Set<string>;
	showBeamGuides: boolean;
	virtualHighlight?: boolean;
};

function relatedHeadsByNode(graph: GeometryGraph) {
	const parentByNode = new Map(
		graph.nodes.map((node) => [node.id, node.parent_id]),
	);
	const result = new Map<string, Set<string>>();
	for (const emitter of graph.emitters) {
		let nodeId: string | null = emitter.node_id;
		while (nodeId) {
			const heads = result.get(nodeId) ?? new Set<string>();
			heads.add(emitter.head_id);
			result.set(nodeId, heads);
			nodeId = parentByNode.get(nodeId) ?? null;
		}
	}
	return result;
}

function attributesForNode(
	options: ProfileGeometryOptions,
	headIds: Set<string>,
) {
	const { fixture, mode, byFixture } = options;
	if (headIds.size === 1) {
		return attributesForHead(fixture, mode, [...headIds][0], byFixture);
	}
	const attributes = new Map(byFixture.get(fixture.fixture_id) ?? []);
	for (const headId of headIds) {
		for (const [attribute, value] of attributesForHead(
			fixture,
			mode,
			headId,
			byFixture,
		)) {
			if (!attributes.has(attribute)) attributes.set(attribute, value);
		}
	}
	return attributes;
}

function applyNodeMotion(
	node: GeometryNode,
	attributes: FixtureAttributeValues,
	translation: THREE.Vector3,
	rotation: { x: number; y: number; z: number },
) {
	if (!node.motion) return;
	const level = normalized(attributes.get(node.motion.attribute), 0.5);
	const physical =
		node.motion.physical_min +
		(node.motion.physical_max - node.motion.physical_min) * level;
	if (node.motion.kind === "rotation") {
		rotation.x += node.motion.axis.x * physical;
		rotation.y += node.motion.axis.y * physical;
		rotation.z += node.motion.axis.z * physical;
		return;
	}
	translation.add(
		millimetres({
			x: node.motion.axis.x * physical,
			y: node.motion.axis.y * physical,
			z: node.motion.axis.z * physical,
		}),
	);
}

function markerDimensions(fixture: PatchedFixture, nodeIndex: number) {
	if (nodeIndex !== 0) return [0.12, 0.12, 0.12] as const;
	return [
		fixture.definition.physical.width_millimetres,
		fixture.definition.physical.height_millimetres,
		fixture.definition.physical.depth_millimetres,
	].map((value) => Math.max(0.08, (value ?? 160) / 1_000));
}

function createGeometryMarker(
	fixture: PatchedFixture,
	node: GeometryNode,
	nodeIndex: number,
	selected: boolean,
) {
	const dimensions = markerDimensions(fixture, nodeIndex);
	const marker = new THREE.Mesh(
		new THREE.BoxGeometry(dimensions[0], dimensions[1], dimensions[2]),
		new THREE.MeshStandardMaterial({
			color: selected ? 0x136f80 : 0x252c33,
			roughness: 0.55,
			metalness: 0.35,
		}),
	);
	marker.name = `geometry-part:${node.id}`;
	return marker;
}

function createGeometryNode(
	options: ProfileGeometryOptions,
	node: GeometryNode,
	nodeIndex: number,
	headIds: Set<string>,
): GeometryNodeParts {
	const attributes = attributesForNode(options, headIds);
	const translation = millimetres(node.transform.translation);
	const rotation = { ...node.transform.rotation_degrees };
	applyNodeMotion(node, attributes, translation, rotation);
	const pivot = millimetres(node.pivot);
	const group = new THREE.Group();
	group.name = `geometry-node:${node.id}`;
	group.position.copy(translation).add(pivot);
	group.rotation.set(
		THREE.MathUtils.degToRad(rotation.x),
		THREE.MathUtils.degToRad(rotation.y),
		THREE.MathUtils.degToRad(rotation.z),
	);
	group.scale.set(
		node.transform.scale.x || 1,
		node.transform.scale.y || 1,
		node.transform.scale.z || 1,
	);
	const anchor = new THREE.Group();
	anchor.name = `geometry-node-anchor:${node.id}`;
	anchor.position.copy(pivot).multiplyScalar(-1);
	anchor.add(
		createGeometryMarker(options.fixture, node, nodeIndex, options.selected),
	);
	group.add(anchor);
	return { group, anchor };
}

function createGeometryNodes(options: ProfileGeometryOptions) {
	const relatedHeads = relatedHeadsByNode(options.mode.geometry);
	return new Map(
		options.mode.geometry.nodes.map((node, index) => [
			node.id,
			createGeometryNode(
				options,
				node,
				index,
				relatedHeads.get(node.id) ?? new Set(),
			),
		]),
	);
}

function mountNodeHierarchy(
	graph: GeometryGraph,
	nodes: Map<string, GeometryNodeParts>,
	root: THREE.Group,
) {
	for (const node of graph.nodes) {
		const current = nodes.get(node.id);
		if (!current) continue;
		const parent = node.parent_id ? nodes.get(node.parent_id)?.anchor : null;
		(parent ?? root).add(current.group);
	}
}

function emitterIntensity(
	options: ProfileGeometryOptions,
	emitter: GeometryEmitter,
	attributes: FixtureAttributeValues,
) {
	if (options.virtualHighlight) return 1;
	const resolved = normalized(
		attributes.get("intensity"),
		channelDefault(options.mode, emitter.head_id, "intensity", 1),
	);
	const owner = headOwnerId(options.fixture, options.mode, emitter.head_id);
	if (options.projectedOwners.has(owner)) return resolved;
	return (options.snapshot?.blackout ? 0 : resolved) *
		(options.snapshot?.grand_master ?? 1);
}

function mountEmitter(
	options: ProfileGeometryOptions,
	emitter: GeometryEmitter,
	nodes: Map<string, GeometryNodeParts>,
	root: THREE.Group,
) {
	const attributes = attributesForHead(
		options.fixture,
		options.mode,
		emitter.head_id,
		options.byFixture,
	);
	const beam = buildGeometryBeam(
		emitter,
		attributes,
		emitterIntensity(options, emitter, attributes),
		resolvedColor(attributes.get("color"), attributes),
		options.showBeamGuides,
	);
	(nodes.get(emitter.node_id)?.anchor ?? root).add(beam);
}

function mountEmitters(
	options: ProfileGeometryOptions,
	nodes: Map<string, GeometryNodeParts>,
	root: THREE.Group,
) {
	for (const emitter of options.mode.geometry.emitters) {
		mountEmitter(options, emitter, nodes, root);
	}
}

export function buildFixtureProfileGeometry(options: ProfileGeometryOptions) {
	if (!options.mode.geometry.nodes.length) return null;
	const root = new THREE.Group();
	root.name = "fixture-profile-geometry";
	const nodes = createGeometryNodes(options);
	mountNodeHierarchy(options.mode.geometry, nodes, root);
	mountEmitters(options, nodes, root);
	if (options.selected) addSelectionOutline(root);
	return root;
}

function previewFixture() {
	return {
		fixture_id: "fixture-profile-preview",
		logical_heads: [],
		definition: { physical: {}, heads: [] },
	} as unknown as PatchedFixture;
}

/** Build the same hierarchy and beam objects used on Stage for the profile editor's live preview. */
export function buildFixtureProfileGeometryPreview(mode: FixtureMode) {
	return (
		buildFixtureProfileGeometry({
			fixture: previewFixture(),
			mode,
			byFixture: new Map(),
			selected: false,
			snapshot: null,
			projectedOwners: new Set(),
			showBeamGuides: true,
		}) ?? new THREE.Group()
	);
}
