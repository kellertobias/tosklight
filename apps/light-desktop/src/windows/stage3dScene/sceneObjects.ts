import * as THREE from "three";
import type { Vector3Value } from "../../api/types";
import type { StageProceduralResourceCache } from "./resources";

function fixtureMaterial(resources?: StageProceduralResourceCache) {
	const create = () =>
		new THREE.MeshStandardMaterial({
			color: 0x252c33,
			roughness: 0.55,
			metalness: 0.35,
		});
	return resources?.material("fixture-marker-material", create) ?? create();
}

export function fixtureBody(
	selected: boolean,
	resources?: StageProceduralResourceCache,
) {
	const group = new THREE.Group();
	group.name = "fixture-placeholder";
	const material = fixtureMaterial(resources);
	const geometry = <T extends THREE.BufferGeometry>(
		key: string,
		create: () => T,
	) => resources?.geometry(key, create) ?? create();
	const base = new THREE.Mesh(
		geometry(
			"fixture-placeholder-base",
			() => new THREE.CylinderGeometry(0.22, 0.27, 0.18, 16),
		),
		material,
	);
	const yoke = new THREE.Mesh(
		geometry(
			"fixture-placeholder-yoke",
			() => new THREE.BoxGeometry(0.46, 0.42, 0.12),
		),
		material,
	);
	yoke.position.y = -0.25;
	const head = new THREE.Mesh(
		geometry(
			"fixture-placeholder-head",
			() => new THREE.CylinderGeometry(0.2, 0.24, 0.42, 16),
		),
		material,
	);
	head.rotation.z = Math.PI / 2;
	head.position.y = -0.52;
	group.add(base, yoke, head);
	addSelectionOutline(group, selected);
	return group;
}

export function addSelectionOutline(object: THREE.Object3D, visible = true) {
	if (!visible) return;
	object.traverse((child) => {
		if (!(child instanceof THREE.Mesh)) return;
		if (child.children.some((nested) => nested.name === "selection-outline"))
			return;
		// Imported and procedural marker meshes may have no vertices.
		if (!child.geometry.getAttribute("position")?.count) return;
		const outline = new THREE.LineSegments(
			new THREE.EdgesGeometry(child.geometry),
			new THREE.LineBasicMaterial({ color: 0x378eff }),
		);
		outline.name = "selection-outline";
		outline.userData.stageSelectionScale = 1.025;
		outline.scale.setScalar(1.025);
		child.add(outline);
	});
}

export function setSelectionOutlineVisibility(
	object: THREE.Object3D,
	visible: boolean,
) {
	object.userData.stageSelected = visible;
	if (visible) {
		addSelectionOutline(object);
		return;
	}
	const outlines: THREE.Object3D[] = [];
	object.traverse((child) => {
		if (child.name === "selection-outline") outlines.push(child);
	});
	for (const outline of outlines) {
		outline.removeFromParent();
		if (outline instanceof THREE.LineSegments) {
			outline.geometry.dispose();
			for (const material of Array.isArray(outline.material)
				? outline.material
				: [outline.material])
				material.dispose();
		}
	}
}

export function millimetres(value: Vector3Value) {
	return new THREE.Vector3(value.x / 1_000, value.y / 1_000, value.z / 1_000);
}

export function emitterSurfaceMaterial(color: THREE.Color, intensity: number) {
	if (intensity <= 0.001) {
		return new THREE.MeshStandardMaterial({
			color: 0x56616a,
			roughness: 0.34,
			metalness: 0.18,
			side: THREE.DoubleSide,
		});
	}
	return new THREE.MeshBasicMaterial({
		color: color
			.clone()
			.lerp(new THREE.Color(0xffffff), 0.75)
			.multiplyScalar(2.3),
		toneMapped: false,
		side: THREE.DoubleSide,
	});
}
