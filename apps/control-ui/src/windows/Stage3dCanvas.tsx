import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import type { VisualizationSnapshot } from "../api/types";
import type { StagePosition3d } from "../api/ServerContext";
import {
	buildStageScene,
	disposeScene,
	mountFixtureModel,
	type Stage3dFixture,
} from "./stage3dScene";
import { useApp } from "../state/AppContext";

export const DEFAULT_STAGE_CAMERA_3D = {
	position: [0, 1.625, 8] as const,
	target: [0, 2.6, -4] as const,
};

interface Props {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	selected: string[];
	virtualHighlight?: string[];
	setup: boolean;
	showSelection: boolean;
	showFloorGrid: boolean;
	showBeamGuides: boolean;
	environmentBrightness: number;
	camera3d?: {
		position: readonly [number, number, number];
		target: readonly [number, number, number];
	};
	onSelect: (fixtureId: string, additive: boolean) => void;
	onMove: (fixtureId: string, position: StagePosition3d) => void;
	onMoveEnd: (fixtureId: string, position: StagePosition3d) => void;
}

export function Stage3dCanvas({
	fixtures,
	visualization,
	selected,
	virtualHighlight = [],
	setup,
	showSelection,
	showFloorGrid,
	showBeamGuides,
	environmentBrightness,
	camera3d,
	onSelect,
	onMove,
	onMoveEnd,
}: Props) {
	const { state, dispatch } = useApp();
	const defaultCamera3d =
		state.stageZoom === 1 && state.stageOrbitX === 0 && state.stageOrbitY === 0
			? DEFAULT_STAGE_CAMERA_3D
			: undefined;
	const resolvedCamera3d = camera3d ?? defaultCamera3d;
	const host = useRef<HTMLDivElement>(null);
	const cameraPosition = useRef(new THREE.Vector3(0, 3.2, 12));
	const cameraTarget = useRef(new THREE.Vector3(0, 1.8, -4));
	const sceneRef = useRef<THREE.Scene | null>(null);
	const fixtureObjectsRef = useRef(new Map<string, THREE.Object3D>());
	const positionByIdRef = useRef(new Map<string, StagePosition3d>());
	const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
	const controlsRef = useRef<OrbitControls | null>(null);
	const setupRef = useRef(setup);
	const latestVisualization = useRef(visualization);
	const interacting = useRef(false);
	const [renderVisualization, setRenderVisualization] = useState(visualization);
	const callbacks = useRef({ onSelect, onMove, onMoveEnd });
	callbacks.current = { onSelect, onMove, onMoveEnd };
	setupRef.current = setup;
	useEffect(() => {
		latestVisualization.current = visualization;
		if (!interacting.current) setRenderVisualization(visualization);
	}, [visualization]);

	useEffect(() => {
		const { scene, fixtureObjects } = buildStageScene(
			fixtures,
			renderVisualization,
			showSelection ? new Set(selected) : new Set(),
			environmentBrightness,
			showFloorGrid,
			showBeamGuides,
			new Set(virtualHighlight),
		);
		const previousScene = sceneRef.current;
		const previousFixtureObjects = fixtureObjectsRef.current;
		const retainedFixtureModels = new Set<string>();
		for (const item of fixtures) {
			const instanceId = item.instanceId ?? item.fixture.fixture_id;
			const previousRoot = previousFixtureObjects.get(instanceId);
			const nextRoot = fixtureObjects.get(instanceId);
			const selectedNow =
				showSelection && selected.includes(item.fixture.fixture_id);
			if (
				!previousRoot ||
				!nextRoot ||
				Boolean(previousRoot.userData.stageSelected) !== selectedNow
			)
				continue;
			const mounted = previousRoot.children.filter(
				(child) =>
					child.name === "fixture-model" ||
					child.name.startsWith("fixture-model-part:"),
			);
			previousRoot.traverse((child) => {
				if (
					child.parent !== previousRoot &&
					(child.name === "fixture-model" ||
						child.name.startsWith("fixture-model-part:"))
				)
					mounted.push(child);
			});
			for (const model of [...new Set(mounted)]) {
				const target = model.parent?.name
					? nextRoot.getObjectByName(model.parent.name)
					: nextRoot;
				if (!target) continue;
				if (model.name === "fixture-model")
					target.getObjectByName("fixture-placeholder")?.removeFromParent();
				else
					target
						.getObjectByName(
							`geometry-part:${model.name.slice("fixture-model-part:".length)}`,
						)
						?.removeFromParent();
				target.add(model);
				retainedFixtureModels.add(instanceId);
			}
		}
		let modelCancelled = false;
		const decode = (value: string) =>
			fetch(value).then((response) => response.arrayBuffer());
		for (const item of fixtures) {
			if (retainedFixtureModels.has(item.instanceId ?? item.fixture.fixture_id))
				continue;
			const source = item.fixture.definition.model_asset;
			if (!source) continue;
			void decode(source)
				.then((buffer) => {
					if (modelCancelled) return;
					new GLTFLoader().parse(buffer, "", (gltf) => {
						if (modelCancelled) return;
						const root = fixtureObjects.get(
							item.instanceId ?? item.fixture.fixture_id,
						);
						if (!root) return;
						const placeholder = root.getObjectByName("fixture-placeholder");
						placeholder?.parent?.remove(placeholder);
						mountFixtureModel(
							root,
							gltf.scene,
							item.fixture,
							showSelection && selected.includes(item.fixture.fixture_id),
						);
					});
				})
				.catch(() => undefined);
		}
		positionByIdRef.current = new Map(
			fixtures.map((item) => [
				item.instanceId ?? item.fixture.fixture_id,
				item.position,
			]),
		);
		sceneRef.current = scene;
		fixtureObjectsRef.current = fixtureObjects;
		if (previousScene) disposeScene(previousScene);
		return () => {
			modelCancelled = true;
		};
	}, [
		fixtures,
		renderVisualization,
		selected,
		virtualHighlight,
		showSelection,
		showFloorGrid,
		showBeamGuides,
		environmentBrightness,
	]);

	useEffect(() => {
		const container = host.current;
		if (!container) return;
		// Retain the completed frame between animation ticks. Chromium's video capture can otherwise
		// sample WebGL after its default buffer clear and record an intermittent blank Stage frame.
		const renderer = new THREE.WebGLRenderer({
			antialias: true,
			preserveDrawingBuffer: true,
		});
		renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
		renderer.outputColorSpace = THREE.SRGBColorSpace;
		container.replaceChildren(renderer.domElement);
		const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
		const controls = new OrbitControls(camera, renderer.domElement);
		controls.enableDamping = true;
		cameraRef.current = camera;
		controlsRef.current = controls;
		const rememberCamera = () => {
			cameraPosition.current.copy(camera.position);
			cameraTarget.current.copy(controls.target);
		};
		const publishCamera = () => {
			const offset = camera.position.clone().sub(controls.target);
			dispatch({
				type: "SET_STAGE_NAVIGATION",
				zoom: 12 / Math.max(2, offset.length()),
				orbitX: THREE.MathUtils.radToDeg(Math.atan2(offset.x, offset.z)),
				orbitY:
					THREE.MathUtils.radToDeg(Math.asin(offset.y / offset.length())) - 18,
			});
		};
		controls.addEventListener("change", rememberCamera);
		controls.addEventListener("end", publishCamera);
		const raycaster = new THREE.Raycaster();
		const pointer = new THREE.Vector2();
		let dragging: {
			fixtureId: string;
			instanceId: string;
			root: THREE.Object3D;
			y: number;
			offset: THREE.Vector3;
			pending: StagePosition3d;
			additive: boolean;
		} | null = null;
		const updatePointer = (event: PointerEvent) => {
			const box = renderer.domElement.getBoundingClientRect();
			pointer.set(
				((event.clientX - box.left) / box.width) * 2 - 1,
				(-(event.clientY - box.top) / box.height) * 2 + 1,
			);
			raycaster.setFromCamera(pointer, camera);
		};
		const down = (event: PointerEvent) => {
			interacting.current = true;
			updatePointer(event);
			const hit = raycaster
				.intersectObjects([...fixtureObjectsRef.current.values()], true)
				.find((entry) => {
					let node: THREE.Object3D | null = entry.object;
					while (node && !node.userData.fixtureId) node = node.parent;
					return Boolean(node?.userData.fixtureId);
				});
			if (!hit) return;
			let root: THREE.Object3D | null = hit.object;
			while (root && !root.userData.fixtureId) root = root.parent;
			const id = root?.userData.fixtureId as string;
			const instanceId = (root?.userData.instanceId as string) || id;
			const additive = event.metaKey || event.ctrlKey;
			if (!setupRef.current) callbacks.current.onSelect(id, additive);
			if (setupRef.current && root) {
				const current = positionByIdRef.current.get(instanceId);
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
			}
		};
		const move = (event: PointerEvent) => {
			if (!dragging) return;
			updatePointer(event);
			const plane = new THREE.Plane(new THREE.Vector3(0, 1, 0), -dragging.y);
			const point = new THREE.Vector3();
			if (!raycaster.ray.intersectPlane(plane, point)) return;
			point.add(dragging.offset);
			const current = positionByIdRef.current.get(dragging.instanceId);
			if (!current) return;
			dragging.root.position.copy(point);
			dragging.pending = { ...current, x: point.x, y: -point.z, z: point.y };
		};
		const up = () => {
			if (dragging) {
				callbacks.current.onSelect(dragging.fixtureId, dragging.additive);
				callbacks.current.onMove(dragging.instanceId, dragging.pending);
				callbacks.current.onMoveEnd(dragging.instanceId, dragging.pending);
			}
			dragging = null;
			controls.enabled = true;
			interacting.current = false;
			setRenderVisualization(latestVisualization.current);
		};
		renderer.domElement.addEventListener("pointerdown", down);
		renderer.domElement.addEventListener("pointermove", move);
		renderer.domElement.addEventListener("pointerup", up);
		renderer.domElement.addEventListener("pointercancel", up);
		const resize = () => {
			const { width, height } = container.getBoundingClientRect();
			renderer.setSize(width, height, false);
			camera.aspect = width / Math.max(height, 1);
			camera.updateProjectionMatrix();
		};
		const observer = new ResizeObserver(resize);
		observer.observe(container);
		resize();
		let frame = 0;
		const animate = () => {
			controls.update();
			if (sceneRef.current) renderer.render(sceneRef.current, camera);
			frame = requestAnimationFrame(animate);
		};
		animate();
		return () => {
			cancelAnimationFrame(frame);
			observer.disconnect();
			controls.dispose();
			controls.removeEventListener("change", rememberCamera);
			controls.removeEventListener("end", publishCamera);
			renderer.domElement.removeEventListener("pointerdown", down);
			renderer.domElement.removeEventListener("pointermove", move);
			renderer.domElement.removeEventListener("pointerup", up);
			renderer.domElement.removeEventListener("pointercancel", up);
			const scene = sceneRef.current;
			sceneRef.current = null;
			fixtureObjectsRef.current = new Map();
			if (scene) disposeScene(scene);
			cameraRef.current = null;
			controlsRef.current = null;
			renderer.forceContextLoss();
			renderer.dispose();
		};
	}, [dispatch]);

	useEffect(() => {
		const camera = cameraRef.current;
		const controls = controlsRef.current;
		if (!camera || !controls) return;
		if (resolvedCamera3d) {
			camera.position.set(...resolvedCamera3d.position);
			controls.target.set(...resolvedCamera3d.target);
		} else {
			const orbitRadius = Math.max(2, 12 / Math.max(0.2, state.stageZoom));
			const azimuth = THREE.MathUtils.degToRad(state.stageOrbitX);
			const elevation = THREE.MathUtils.degToRad(18 + state.stageOrbitY);
			camera.position.set(
				Math.sin(azimuth) * orbitRadius,
				1.8 + Math.sin(elevation) * orbitRadius,
				-4 + Math.cos(azimuth) * Math.cos(elevation) * orbitRadius,
			);
			controls.target.copy(cameraTarget.current);
		}
		controls.update();
	}, [resolvedCamera3d, state.stageZoom, state.stageOrbitX, state.stageOrbitY]);

	return (
		<div
			className="stage-3d-canvas"
			data-camera-position={resolvedCamera3d?.position.join(",")}
			data-camera-target={resolvedCamera3d?.target.join(",")}
			data-environment-brightness={environmentBrightness}
			data-floor-grid={showFloorGrid ? "on" : "off"}
			data-beam-guides={showBeamGuides ? "on" : "off"}
			ref={host}
		/>
	);
}
