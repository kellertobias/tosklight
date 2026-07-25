import * as THREE from "three";
import type { Vector3Value } from "../../api/types";

function fixtureMaterial(selected: boolean) {
	return new THREE.MeshStandardMaterial({
		color: selected ? 0x136f80 : 0x252c33,
		roughness: 0.55,
		metalness: 0.35,
	});
}

function addPlaceholderOutline(group: THREE.Group, mesh: THREE.Mesh) {
	const outline = new THREE.LineSegments(
		new THREE.EdgesGeometry(mesh.geometry),
		new THREE.LineBasicMaterial({ color: 0x378eff }),
	);
	outline.position.copy(mesh.position);
	outline.rotation.copy(mesh.rotation);
	outline.scale.setScalar(1.035);
	outline.name = "selection-outline";
	group.add(outline);
}

export function fixtureBody(selected: boolean) {
	const group = new THREE.Group();
	group.name = "fixture-placeholder";
	const material = fixtureMaterial(selected);
	const base = new THREE.Mesh(
		new THREE.CylinderGeometry(0.22, 0.27, 0.18, 16),
		material,
	);
	const yoke = new THREE.Mesh(
		new THREE.BoxGeometry(0.46, 0.42, 0.12),
		material,
	);
	yoke.position.y = -0.25;
	const head = new THREE.Mesh(
		new THREE.CylinderGeometry(0.2, 0.24, 0.42, 16),
		material,
	);
	head.rotation.z = Math.PI / 2;
	head.position.y = -0.52;
	group.add(base, yoke, head);
	if (selected) {
		for (const mesh of [base, yoke, head]) addPlaceholderOutline(group, mesh);
	}
	return group;
}

export function addSelectionOutline(object: THREE.Object3D) {
	object.traverse((child) => {
		if (!(child instanceof THREE.Mesh)) return;
		// Imported and procedural marker meshes may have no vertices.
		if (!child.geometry.getAttribute("position")?.count) return;
		const outline = new THREE.LineSegments(
			new THREE.EdgesGeometry(child.geometry),
			new THREE.LineBasicMaterial({ color: 0x378eff }),
		);
		outline.name = "selection-outline";
		outline.scale.setScalar(1.025);
		child.add(outline);
	});
}

export function millimetres(value: Vector3Value) {
	return new THREE.Vector3(value.x / 1_000, value.y / 1_000, value.z / 1_000);
}

export function emitterSurfaceMaterial(
	color: THREE.Color,
	intensity: number,
) {
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
