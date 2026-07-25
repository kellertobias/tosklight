import * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import { buildStageScene, disposeScene } from "./stageScene";
import type { Stage3dFixture } from "./types";

function thumbnailCamera(width: number, height: number) {
	const camera = new THREE.PerspectiveCamera(48, width / height, 0.1, 100);
	camera.position.set(10, 8, 11);
	camera.lookAt(0, 1.8, -4);
	return camera;
}

function thumbnailRenderer(width: number, height: number) {
	const renderer = new THREE.WebGLRenderer({
		antialias: true,
		preserveDrawingBuffer: true,
		alpha: false,
	});
	renderer.setSize(width, height, false);
	renderer.setPixelRatio(1);
	return renderer;
}

export function renderStageThumbnail(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot,
	width = 240,
	height = 135,
) {
	const { scene } = buildStageScene(fixtures, snapshot);
	const renderer = thumbnailRenderer(width, height);
	renderer.render(scene, thumbnailCamera(width, height));
	const result = renderer.domElement.toDataURL("image/webp", 0.8);
	disposeScene(scene);
	renderer.forceContextLoss();
	renderer.dispose();
	return result;
}
