import * as THREE from "three";
import { describe, expect, it, vi } from "vitest";
import { StageModelCache } from "./modelCache";

describe("StageModelCache", () => {
	it("loads one decoded template, clones instances, and owns final disposal", async () => {
		const geometry = new THREE.BoxGeometry(1, 1, 1);
		const material = new THREE.MeshStandardMaterial();
		const geometryDispose = vi.spyOn(geometry, "dispose");
		const materialDispose = vi.spyOn(material, "dispose");
		const template = new THREE.Group();
		template.add(new THREE.Mesh(geometry, material));
		const load = vi.fn(async () => template);
		const cache = new StageModelCache(load);

		const [first, second] = await Promise.all([
			cache.clone("fixture.glb"),
			cache.clone("fixture.glb"),
		]);

		expect(load).toHaveBeenCalledOnce();
		expect(first).not.toBe(second);
		expect((first.children[0] as THREE.Mesh).geometry).toBe(geometry);
		expect((second.children[0] as THREE.Mesh).material).toBe(material);
		expect(geometry.userData.stageSharedModelResource).toBe(true);
		expect(material.userData.stageSharedModelResource).toBe(true);

		cache.dispose();
		await Promise.resolve();
		expect(geometryDispose).toHaveBeenCalledOnce();
		expect(materialDispose).toHaveBeenCalledOnce();
		await expect(cache.clone("fixture.glb")).rejects.toThrow(
			"Stage model cache is closed",
		);
	});
});
