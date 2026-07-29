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
	private readonly templates = new Map<
		string,
		{
			template: Promise<THREE.Group>;
			retainers: number;
			retired: boolean;
		}
	>();
	private disposed = false;

	constructor(
		private readonly load: (source: string) => Promise<THREE.Group> = loadModel,
	) {}

	retain(source: string, revision: string | number) {
		if (this.disposed) throw new Error("The Stage model cache is closed");
		const key = `${source}\u0000${revision}`;
		let entry = this.templates.get(key);
		frontendPerformanceDiagnostics.recordStageModelCacheLookup(Boolean(entry));
		if (!entry) {
			const finishLoad = frontendPerformanceDiagnostics.beginStageModelLoad();
			const template = this.load(source).then(
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
			entry = { template, retainers: 0, retired: false };
			this.templates.set(key, entry);
		}
		entry.retainers += 1;
		const retainedEntry = entry;
		let released = false;
		return {
			model: retainedEntry.template.then((template) => {
				const clone = template.clone(true);
				frontendPerformanceDiagnostics.recordStageModelClone();
				return clone;
			}),
			release: () => {
				if (released) return;
				released = true;
				retainedEntry.retainers -= 1;
				if (retainedEntry.retainers === 0) this.retire(key, retainedEntry);
			},
		};
	}

	private retire(
		key: string,
		entry: {
			template: Promise<THREE.Group>;
			retainers: number;
			retired: boolean;
		},
	) {
		if (entry.retired) return;
		entry.retired = true;
		if (this.templates.get(key) === entry) this.templates.delete(key);
		void entry.template.then(disposeTemplate).catch(() => undefined);
	}

	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		for (const [key, entry] of this.templates) this.retire(key, entry);
		frontendPerformanceDiagnostics.recordStageModelCacheDisposal();
	}
}
