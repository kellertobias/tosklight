import { act, renderHook } from "@testing-library/react";
import * as THREE from "three";
import { describe, expect, it, vi } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import type { StageRenderQuality } from "../../types";
import { retainFixtureModel, useStageScene } from "./useStageScene";

describe("useStageScene context recovery", () => {
	it("rebuilds retained structural state from the newest inputs after context restoration", () => {
		const baseline = frontendPerformanceDiagnostics.snapshot().stage;
		const { result } = renderHook(() =>
			useStageScene({
				fixtures: [],
				visualization: null,
				selected: [],
				virtualHighlight: [],
				showSelection: true,
				showFloorGrid: true,
				showBeamGuides: true,
				renderQuality: "lines_and_beams",
				environmentBrightness: 1,
				callbacks: { onSelect: vi.fn() },
			}),
		);
		const firstScene = result.current.sceneRef.current;
		expect(firstScene).toBeTruthy();

		act(() => result.current.recoverContext());

		const recovered = frontendPerformanceDiagnostics.snapshot().stage;
		expect(result.current.sceneRef.current).not.toBe(firstScene);
		expect(recovered.sceneBuilds.length - baseline.sceneBuilds.length).toBe(2);
		expect(recovered.sceneDisposals - baseline.sceneDisposals).toBe(1);
	});

	it("applies a quality change even when the visualization snapshot is unchanged", () => {
		const fixture = {
			fixture_id: "fixture",
			universe: 1,
			address: 1,
			logical_heads: [],
			definition: {
				device_type: "moving wash",
				name: "Wash",
				manufacturer: "",
				model: "Wash",
			},
		} as unknown as PatchedFixture;
		const visualization = {
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: "fixture",
					attribute: "intensity",
					value: { kind: "normalized" as const, value: 1 },
				},
			],
		};
		const fixtures = [
			{
				fixture,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const { result, rerender } = renderHook(
			({ quality }: { quality: StageRenderQuality }) =>
				useStageScene({
					fixtures,
					visualization,
					selected: [],
					virtualHighlight: [],
					showSelection: true,
					showFloorGrid: true,
					showBeamGuides: true,
					renderQuality: quality,
					environmentBrightness: 1,
					callbacks: { onSelect: vi.fn() },
				}),
			{ initialProps: { quality: "lines_only" as StageRenderQuality } },
		);
		expect(
			result.current.sceneRef.current?.getObjectByName("beam-volume")?.visible,
		).toBe(false);

		rerender({ quality: "beams" });

		expect(
			result.current.sceneRef.current?.getObjectByName("beam-volume")?.visible,
		).toBe(true);
		expect(
			result.current.sceneRef.current?.getObjectByName("beam-centerline")
				?.visible,
		).toBe(false);
	});

	it("keeps repeated source frames render-idle when visible values are unchanged", () => {
		const visualization = {
			revision: 1,
			generated_at: "2026-07-28T10:00:00Z",
			grand_master: 1,
			blackout: false,
			preload: false,
			values: [],
			profile_output_values: [],
		};
		const { result } = renderHook(() =>
			useStageScene({
				fixtures: [],
				visualization,
				selected: [],
				virtualHighlight: [],
				showSelection: true,
				showFloorGrid: true,
				showBeamGuides: true,
				renderQuality: "lines_and_beams",
				environmentBrightness: 1,
				callbacks: { onSelect: vi.fn() },
			}),
		);
		const invalidate = vi.fn();
		result.current.invalidateRef.current = invalidate;

		act(() =>
			result.current.installVisualization({
				...visualization,
				generated_at: "2026-07-28T10:00:00.100Z",
			}),
		);

		expect(invalidate).not.toHaveBeenCalled();
		expect(result.current.displayedVisualizationRef.current?.generated_at).toBe(
			"2026-07-28T10:00:00.100Z",
		);
		expect(result.current.visualizationSettledRef.current).toBe(true);
	});

	it("does not attach a model after its fixture lease becomes stale", async () => {
		let resolveModel: (model: THREE.Group) => void = () => undefined;
		const model = new Promise<THREE.Group>((resolve) => {
			resolveModel = resolve;
		});
		const release = vi.fn();
		const cache = {
			retain: vi.fn(() => ({ model, release })),
		};
		const fixture = {
			fixture_id: "fixture",
			logical_heads: [],
			definition: {
				id: "profile",
				revision: 1,
				model_asset: "fixture.glb",
			},
		} as unknown as PatchedFixture;
		const root = new THREE.Group();
		root.userData.instanceId = fixture.fixture_id;
		const fixtureObjects = new Map<string, THREE.Object3D>([
			[fixture.fixture_id, root],
		]);
		const mounted = new Map<string, { token: symbol; release: () => void }>();
		retainFixtureModel(
			{
				fixture,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 0,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
			fixtureObjects,
			mounted,
			() => false,
			cache as never,
			vi.fn(),
		);
		mounted.set(fixture.fixture_id, {
			token: Symbol("newer fixture revision"),
			release: vi.fn(),
		});

		resolveModel(new THREE.Group());
		await model;
		await Promise.resolve();

		expect(root.getObjectByName("fixture-model")).toBeUndefined();
		expect(release).toHaveBeenCalledOnce();
	});
});
