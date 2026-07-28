import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";

function parseModel(buffer: ArrayBuffer) {
	return new Promise<THREE.Group>((resolve, reject) => {
		new GLTFLoader().parse(buffer, "", (gltf) => resolve(gltf.scene), reject);
	});
}

async function loadModel(source: string) {
	const response = await fetch(source);
	if (!response.ok)
		throw new Error(`Stage model request failed with ${response.status}`);
	return parseModel(await response.arrayBuffer());
}

function markSharedResources(root: THREE.Object3D) {
	root.traverse((object) => {
		const mesh = object as THREE.Mesh;
		if (mesh.geometry) mesh.geometry.userData.stageSharedModelResource = true;
		const materials = Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: [];
		for (const material of materials)
			material.userData.stageSharedModelResource = true;
	});
}

function disposeTemplate(root: THREE.Object3D) {
	const geometries = new Set<THREE.BufferGeometry>();
	const materials = new Set<THREE.Material>();
	const textures = new Set<THREE.Texture>();
	root.traverse((object) => {
		const mesh = object as THREE.Mesh;
		if (mesh.geometry) geometries.add(mesh.geometry);
		for (const material of Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: []) {
			materials.add(material);
			for (const value of Object.values(material)) {
				if (value instanceof THREE.Texture) textures.add(value);
			}
		}
	});
	for (const texture of textures) texture.dispose();
	for (const material of materials) material.dispose();
	for (const geometry of geometries) geometry.dispose();
}

/** Renderer-surface-owned decoded GLB templates shared by every fixture instance. */
export class StageModelCache {
	private readonly templates = new Map<string, Promise<THREE.Group>>();
	private disposed = false;

	constructor(
		private readonly load: (source: string) => Promise<THREE.Group> = loadModel,
	) {}

	async clone(source: string) {
		if (this.disposed) throw new Error("The Stage model cache is closed");
		let template = this.templates.get(source);
		frontendPerformanceDiagnostics.recordStageModelCacheLookup(Boolean(template));
		if (!template) {
			const finishLoad = frontendPerformanceDiagnostics.beginStageModelLoad();
			template = this.load(source).then(
				(model) => {
					markSharedResources(model);
					finishLoad();
					return model;
				},
				(error) => {
					finishLoad(error);
					throw error;
				},
			);
			this.templates.set(source, template);
		}
		const clone = (await template).clone(true);
		frontendPerformanceDiagnostics.recordStageModelClone();
		return clone;
	}

	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		for (const template of this.templates.values())
			void template.then(disposeTemplate).catch(() => undefined);
		this.templates.clear();
		frontendPerformanceDiagnostics.recordStageModelCacheDisposal();
	}
}
