import * as THREE from "three";
import { describe, expect, it, vi } from "vitest";
import { StageModelCache } from "./modelCache";

describe("StageModelCache", () => {
	it("shares one revision while retained and disposes it after the final release", async () => {
		const { frontendPerformanceDiagnostics } = await import(
			"../../features/frontendWarmup/diagnostics"
		);
		const baseline = frontendPerformanceDiagnostics.snapshot().stage;
		const geometry = new THREE.BoxGeometry(1, 1, 1);
		const material = new THREE.MeshStandardMaterial();
		const geometryDispose = vi.spyOn(geometry, "dispose");
		const materialDispose = vi.spyOn(material, "dispose");
		const template = new THREE.Group();
		template.add(new THREE.Mesh(geometry, material));
		const load = vi.fn(async () => template);
		const cache = new StageModelCache(load);

		const firstLease = cache.retain("fixture.glb", 7);
		const secondLease = cache.retain("fixture.glb", 7);
		const [first, second] = await Promise.all([
			firstLease.model,
			secondLease.model,
		]);

		expect(load).toHaveBeenCalledOnce();
		expect(first).not.toBe(second);
		expect((first.children[0] as THREE.Mesh).geometry).toBe(geometry);
		expect((second.children[0] as THREE.Mesh).material).toBe(material);
		expect(geometry.userData.stageSharedModelResource).toBe(true);
		expect(material.userData.stageSharedModelResource).toBe(true);

		firstLease.release();
		expect(geometryDispose).not.toHaveBeenCalled();
		secondLease.release();
		await Promise.resolve();
		expect(geometryDispose).toHaveBeenCalledOnce();
		expect(materialDispose).toHaveBeenCalledOnce();
		cache.dispose();
		expect(() => cache.retain("fixture.glb", 7)).toThrow(
			"The Stage model cache is closed",
		);
		const measured = frontendPerformanceDiagnostics.snapshot().stage;
		expect(measured.modelCacheMisses - baseline.modelCacheMisses).toBe(1);
		expect(measured.modelCacheHits - baseline.modelCacheHits).toBe(1);
		expect(measured.modelClones - baseline.modelClones).toBe(2);
		expect(measured.modelCacheDisposals - baseline.modelCacheDisposals).toBe(1);
		expect(measured.modelLoads.slice(baseline.modelLoads.length)).toEqual([
			expect.objectContaining({
				status: "ready",
				durationMs: expect.any(Number),
			}),
		]);
	});

	it("owns revisions independently and releases a stale pending load", async () => {
		let resolveDeferred: (value: THREE.Group) => void = () => undefined;
		const deferred = new Promise<THREE.Group>((resolve) => {
			resolveDeferred = resolve;
		});
		const firstGeometry = new THREE.BoxGeometry();
		const secondGeometry = new THREE.SphereGeometry();
		const firstDispose = vi.spyOn(firstGeometry, "dispose");
		const secondDispose = vi.spyOn(secondGeometry, "dispose");
		const first = new THREE.Group();
		first.add(new THREE.Mesh(firstGeometry, new THREE.MeshBasicMaterial()));
		const second = new THREE.Group();
		second.add(new THREE.Mesh(secondGeometry, new THREE.MeshBasicMaterial()));
		const load = vi
			.fn<(source: string) => Promise<THREE.Group>>()
			.mockReturnValueOnce(deferred)
			.mockResolvedValueOnce(second);
		const cache = new StageModelCache(load);

		const stale = cache.retain("fixture.glb", 1);
		stale.release();
		const current = cache.retain("fixture.glb", 2);
		resolveDeferred(first);
		await Promise.all([stale.model, current.model]);
		await Promise.resolve();

		expect(load).toHaveBeenCalledTimes(2);
		expect(firstDispose).toHaveBeenCalledOnce();
		expect(secondDispose).not.toHaveBeenCalled();
		current.release();
		await Promise.resolve();
		expect(secondDispose).toHaveBeenCalledOnce();
	});
});
