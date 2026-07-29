import type * as THREE from "three";

/**
 * Procedural resources owned by one Stage renderer surface. Geometry and static
 * materials can be reused across fixture roots without leaking across a closed
 * renderer context.
 */
export class StageProceduralResourceCache {
	private readonly geometries = new Map<string, THREE.BufferGeometry>();
	private readonly materials = new Map<string, THREE.Material>();
	private disposed = false;

	geometry<T extends THREE.BufferGeometry>(key: string, create: () => T): T {
		if (this.disposed) throw new Error("The Stage procedural cache is closed");
		const retained = this.geometries.get(key);
		if (retained) return retained as T;
		const geometry = create();
		geometry.userData.stageSharedProceduralResource = true;
		this.geometries.set(key, geometry);
		return geometry;
	}

	material<T extends THREE.Material>(key: string, create: () => T): T {
		if (this.disposed) throw new Error("The Stage procedural cache is closed");
		const retained = this.materials.get(key);
		if (retained) return retained as T;
		const material = create();
		material.userData.stageSharedProceduralResource = true;
		this.materials.set(key, material);
		return material;
	}

	dispose() {
		if (this.disposed) return;
		this.disposed = true;
		for (const material of this.materials.values()) material.dispose();
		for (const geometry of this.geometries.values()) geometry.dispose();
		this.materials.clear();
		this.geometries.clear();
	}
}
