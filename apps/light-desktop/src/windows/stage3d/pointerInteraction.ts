import * as THREE from "three";
import type { StageSceneController } from "./useStageScene";

function fixtureRoot(object: THREE.Object3D) {
	let root: THREE.Object3D | null = object;
	while (root && !root.userData.fixtureId) root = root.parent;
	return root;
}

function updateRaycaster(
	event: PointerEvent,
	renderer: THREE.WebGLRenderer,
	camera: THREE.PerspectiveCamera,
	raycaster: THREE.Raycaster,
	pointer: THREE.Vector2,
) {
	const box = renderer.domElement.getBoundingClientRect();
	pointer.set(
		((event.clientX - box.left) / box.width) * 2 - 1,
		(-(event.clientY - box.top) / box.height) * 2 + 1,
	);
	raycaster.setFromCamera(pointer, camera);
}

export function bindStagePointerInteraction(
	renderer: THREE.WebGLRenderer,
	camera: THREE.PerspectiveCamera,
	controller: StageSceneController,
) {
	const raycaster = new THREE.Raycaster();
	const pointer = new THREE.Vector2();
	const down = (event: PointerEvent) => {
		controller.interactingRef.current = true;
		updateRaycaster(event, renderer, camera, raycaster, pointer);
		const hit = raycaster
			.intersectObjects(
				[...controller.fixtureObjectsRef.current.values()],
				true,
			)
			.find((entry) => Boolean(fixtureRoot(entry.object)?.userData.fixtureId));
		if (!hit) return;
		const id = fixtureRoot(hit.object)?.userData.fixtureId as string;
		controller.callbacksRef.current.onSelect(
			id,
			event.metaKey || event.ctrlKey,
		);
	};
	const up = () => {
		controller.interactingRef.current = false;
		controller.installVisualization(controller.latestVisualizationRef.current);
	};
	renderer.domElement.addEventListener("pointerdown", down);
	renderer.domElement.addEventListener("pointerup", up);
	renderer.domElement.addEventListener("pointercancel", up);
	return () => {
		renderer.domElement.removeEventListener("pointerdown", down);
		renderer.domElement.removeEventListener("pointerup", up);
		renderer.domElement.removeEventListener("pointercancel", up);
	};
}
