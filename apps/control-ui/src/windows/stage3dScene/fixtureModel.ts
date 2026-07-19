import * as THREE from "three";
import type { FixtureMode, PatchedFixture } from "../../api/types";
import { profileMode } from "./attributeValues";
import { addSelectionOutline } from "./sceneObjects";

type GeometryBinding = FixtureMode["geometry"]["nodes"][number];

type BoundPartContext = {
	root: THREE.Object3D;
	model: THREE.Object3D;
	boundNames: Set<string>;
	fixtureId: string;
	instanceId: string;
	scale: number;
	selected: boolean;
};

function removeGeometryMarker(root: THREE.Object3D, nodeId: string) {
	root.getObjectByName(`geometry-part:${nodeId}`)?.removeFromParent();
}

function normalizedModelScale(model: THREE.Object3D, fixture: PatchedFixture) {
	const size = new THREE.Box3()
		.setFromObject(model)
		.getSize(new THREE.Vector3());
	const desiredHeight =
		(fixture.definition.physical.height_millimetres ?? 600) / 1_000;
	return desiredHeight / Math.max(size.y, size.x, size.z, 0.001);
}

function modelScale(model: THREE.Object3D, fixture: PatchedFixture) {
	// Venue packages author visual-only GLBs in metres to retain their real size.
	return fixture.definition.profile_snapshot?.model_units === "metres"
		? 1
		: normalizedModelScale(model, fixture);
}

function removeNestedBoundParts(
	model: THREE.Object3D,
	boundNames: Set<string>,
	ownName: string,
) {
	const nested: THREE.Object3D[] = [];
	model.traverse((object) => {
		if (object !== model && object.name !== ownName && boundNames.has(object.name)) {
			nested.push(object);
		}
	});
	for (const object of nested) object.removeFromParent();
}

function tagModel(
	model: THREE.Object3D,
	fixtureId: string,
	instanceId: string,
) {
	model.traverse((object) => {
		object.userData.fixtureId = fixtureId;
		object.userData.instanceId = instanceId;
	});
}

function mountBoundPart(
	binding: GeometryBinding,
	context: BoundPartContext,
) {
	const source = binding.glb_node
		? context.model.getObjectByName(binding.glb_node)
		: null;
	const anchor = context.root.getObjectByName(
		`geometry-node-anchor:${binding.id}`,
	);
	if (!source || !anchor) return false;
	const part = source.clone(true);
	removeNestedBoundParts(part, context.boundNames, source.name);
	// The profile graph owns this transform; retain only GLB-local scale/children.
	part.position.set(0, 0, 0);
	part.quaternion.identity();
	const wrapper = new THREE.Group();
	wrapper.name = `fixture-model-part:${binding.id}`;
	wrapper.scale.setScalar(context.scale);
	wrapper.add(part);
	tagModel(wrapper, context.fixtureId, context.instanceId);
	if (context.selected) addSelectionOutline(wrapper);
	removeGeometryMarker(context.root, binding.id);
	anchor.add(wrapper);
	return true;
}

function mountBoundParts(
	root: THREE.Object3D,
	model: THREE.Object3D,
	bindings: GeometryBinding[],
	fixture: PatchedFixture,
	instanceId: string,
	scale: number,
	selected: boolean,
) {
	const boundNames = new Set(
		bindings.flatMap((binding) =>
			binding.glb_node ? [binding.glb_node] : [],
		),
	);
	const context: BoundPartContext = {
		root,
		model,
		boundNames,
		fixtureId: fixture.fixture_id,
		instanceId,
		scale,
		selected,
	};
	let mounted = 0;
	for (const binding of bindings) {
		if (mountBoundPart(binding, context)) mounted += 1;
	}
	return mounted;
}

function centerModel(model: THREE.Object3D) {
	const box = new THREE.Box3().setFromObject(model);
	const center = box.getCenter(new THREE.Vector3());
	model.position.sub(center);
	model.position.y -= box.min.y - center.y;
}

function mountWholeModel(
	root: THREE.Object3D,
	model: THREE.Object3D,
	fixture: PatchedFixture,
	instanceId: string,
	scale: number,
	selected: boolean,
) {
	model.name = "fixture-model";
	tagModel(model, fixture.fixture_id, instanceId);
	model.scale.setScalar(scale);
	centerModel(model);
	if (selected) addSelectionOutline(model);
	const profileRoot = profileMode(fixture)?.geometry.nodes.find(
		(node) => node.parent_id == null,
	);
	const target = profileRoot
		? (root.getObjectByName(`geometry-node-anchor:${profileRoot.id}`) ?? root)
		: root;
	if (profileRoot) removeGeometryMarker(root, profileRoot.id);
	target.add(model);
}

/**
 * Attach a loaded profile GLB while keeping the profile hierarchy authoritative
 * for pivots, transforms, and motion.
 */
export function mountFixtureModel(
	root: THREE.Object3D,
	model: THREE.Object3D,
	fixture: PatchedFixture,
	selected = false,
) {
	const instanceId = root.userData.instanceId ?? fixture.fixture_id;
	model.updateMatrixWorld(true);
	const scale = modelScale(model, fixture);
	const bindings =
		profileMode(fixture)?.geometry.nodes.filter((node) => node.glb_node) ?? [];
	const mounted = mountBoundParts(
		root,
		model,
		bindings,
		fixture,
		instanceId,
		scale,
		selected,
	);
	if (mounted) return mounted;
	mountWholeModel(root, model, fixture, instanceId, scale, selected);
	return 1;
}
