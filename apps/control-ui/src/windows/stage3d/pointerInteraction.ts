import * as THREE from "three";
import type { StagePosition3d } from "../../api/ServerContext";
import type { StageSceneController } from "./useStageScene";

type DragState = {
	fixtureId: string;
	instanceId: string;
	root: THREE.Object3D;
	y: number;
	offset: THREE.Vector3;
	pending: StagePosition3d;
	additive: boolean;
};

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
	controls: import("three/examples/jsm/controls/OrbitControls.js").OrbitControls,
	controller: StageSceneController,
) {
	const raycaster = new THREE.Raycaster();
	const pointer = new THREE.Vector2();
	let dragging: DragState | null = null;
	const update = (event: PointerEvent) =>
		updateRaycaster(event, renderer, camera, raycaster, pointer);
	const down = (event: PointerEvent) => {
		controller.interactingRef.current = true;
		update(event);
		const hit = raycaster
			.intersectObjects(
				[...controller.fixtureObjectsRef.current.values()],
				true,
			)
			.find((entry) => Boolean(fixtureRoot(entry.object)?.userData.fixtureId));
		if (!hit) return;
		const root = fixtureRoot(hit.object);
		const id = root?.userData.fixtureId as string;
		const instanceId = (root?.userData.instanceId as string) || id;
		const additive = event.metaKey || event.ctrlKey;
		if (!controller.setupRef.current)
			controller.callbacksRef.current.onSelect(id, additive);
		if (!controller.setupRef.current || !root) return;
		const current = controller.positionByIdRef.current.get(instanceId);
		if (!current) return;
		dragging = {
			fixtureId: id,
			instanceId,
			root,
			y: root.position.y,
			offset: root.position.clone().sub(hit.point),
			pending: current,
			additive,
		};
		controls.enabled = false;
		renderer.domElement.setPointerCapture(event.pointerId);
	};
	const move = (event: PointerEvent) => {
		if (!dragging) return;
		update(event);
		const plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), -dragging.y);
		const point = new THREE.Vector3();
		if (!raycaster.ray.intersectPlane(plane, point)) return;
		point.add(dragging.offset);
		const current = controller.positionByIdRef.current.get(dragging.instanceId);
		if (!current) return;
		dragging.root.position.copy(point);
		dragging.pending = { ...current, x: point.x, y: -point.z, z: point.y };
	};
	const up = () => {
		if (dragging) {
			controller.callbacksRef.current.onSelect(
				dragging.fixtureId,
				dragging.additive,
			);
			controller.callbacksRef.current.onMove(
				dragging.instanceId,
				dragging.pending,
			);
			controller.callbacksRef.current.onMoveEnd(
				dragging.instanceId,
				dragging.pending,
			);
		}
		dragging = null;
		controls.enabled = true;
		controller.interactingRef.current = false;
		controller.setRenderVisualization(
			controller.latestVisualizationRef.current,
		);
	};
	renderer.domElement.addEventListener("pointerdown", down);
	renderer.domElement.addEventListener("pointermove", move);
	renderer.domElement.addEventListener("pointerup", up);
	renderer.domElement.addEventListener("pointercancel", up);
	return () => {
		renderer.domElement.removeEventListener("pointerdown", down);
		renderer.domElement.removeEventListener("pointermove", move);
		renderer.domElement.removeEventListener("pointerup", up);
		renderer.domElement.removeEventListener("pointercancel", up);
	};
}
