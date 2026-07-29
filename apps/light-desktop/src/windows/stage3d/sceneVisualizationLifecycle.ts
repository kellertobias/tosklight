import { type MutableRefObject, useEffect } from "react";
import type * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import type { StageRenderQuality } from "../../types";
import { applyStageVisualization, type Stage3dFixture } from "../stage3dScene";
import {
	interpolateVisualizationSnapshot,
	remainingStageInterpolationMillis,
	stageVisualizationChanged,
} from "./interpolation";

type VisualizationOptions = {
	fixtures: Stage3dFixture[];
	showBeamGuides: boolean;
	renderQuality: StageRenderQuality;
	selected: readonly string[];
	selectedKey: string;
	showSelection: boolean;
	virtualHighlight: readonly string[];
	virtualHighlightKey: string;
	fixtureObjectsRef: MutableRefObject<Map<string, THREE.Object3D>>;
	latestVisualizationRef: MutableRefObject<VisualizationSnapshot | null>;
	displayedVisualizationRef: MutableRefObject<VisualizationSnapshot | null>;
	visualizationSettledRef: MutableRefObject<boolean>;
	appliedRenderQualityRef: MutableRefObject<StageRenderQuality>;
	interactingRef: MutableRefObject<boolean>;
	sceneRef: MutableRefObject<THREE.Scene | null>;
	invalidateRef: MutableRefObject<(() => void) | null>;
	installVisualizationRef: MutableRefObject<
		(
			snapshot: VisualizationSnapshot | null,
			forceVisibleApply?: boolean,
		) => void
	>;
};

export function useStageVisualizationLifecycle(options: VisualizationOptions) {
	useEffect(() => {
		let frame: number | null = null;
		const apply = (
			snapshot: VisualizationSnapshot | null,
			settled: boolean,
			visibleChanged = true,
		) => {
			options.displayedVisualizationRef.current = snapshot;
			options.visualizationSettledRef.current = settled;
			applyStageVisualization(
				options.fixtures,
				snapshot,
				options.fixtureObjectsRef.current,
				options.showBeamGuides,
				options.renderQuality,
				new Set(options.virtualHighlight),
				new Set(options.selected),
				options.showSelection,
			);
			options.appliedRenderQualityRef.current = options.renderQuality;
			recordFrameApplied(snapshot, settled, visibleChanged);
			options.invalidateRef.current?.();
		};
		options.installVisualizationRef.current = (
			target,
			forceVisibleApply = false,
		) => {
			options.latestVisualizationRef.current = target;
			if (options.interactingRef.current || !options.sceneRef.current) return;
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
			const from = options.displayedVisualizationRef.current;
			if (!from || !target) {
				apply(target, true);
				return;
			}
			if (!stageVisualizationChanged(from, target)) {
				options.displayedVisualizationRef.current = target;
				options.visualizationSettledRef.current = true;
				recordFrameApplied(target, true, false);
				if (forceVisibleApply) apply(target, true, false);
				return;
			}
			const interpolationMillis = remainingStageInterpolationMillis(
				target.generated_at,
			);
			if (interpolationMillis === 0) {
				apply(target, true);
				return;
			}
			const startedAt = performance.now();
			const step = (now: number) => {
				const progress = Math.min(
					1,
					Math.max(0, (now - startedAt) / interpolationMillis),
				);
				apply(
					interpolateVisualizationSnapshot(from, target, progress),
					progress >= 1,
				);
				if (progress < 1) frame = requestAnimationFrame(step);
				else frame = null;
			};
			frame = requestAnimationFrame(step);
		};
		options.installVisualizationRef.current(
			options.latestVisualizationRef.current,
			true,
		);
		return () => {
			if (frame !== null) cancelAnimationFrame(frame);
			options.installVisualizationRef.current = () => undefined;
		};
	}, [
		options.fixtures,
		options.showBeamGuides,
		options.renderQuality,
		options.selectedKey,
		options.showSelection,
		options.virtualHighlightKey,
	]);
}

function recordFrameApplied(
	snapshot: VisualizationSnapshot | null,
	settled: boolean,
	visibleChanged: boolean,
) {
	frontendPerformanceDiagnostics.recordStageFrameApplied(
		snapshot?.generated_at,
		settled,
		snapshot?.preload ? "preload" : "normal",
		visibleChanged,
	);
}
