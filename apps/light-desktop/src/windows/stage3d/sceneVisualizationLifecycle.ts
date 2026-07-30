import { type MutableRefObject, useEffect } from "react";
import type * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import type { StageRenderQuality } from "../../types";
import { applyStageVisualization, type Stage3dFixture } from "../stage3dScene";
import {
	changedStageFixtureIds,
	interpolateVisualizationSnapshot,
	remainingStageInterpolationMillis,
	shouldInterpolateStageSceneChanges,
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
	invalidateRef: MutableRefObject<((immediate?: boolean) => void) | null>;
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
		const virtualHighlight = new Set(options.virtualHighlight);
		const selected = new Set(options.selected);
		const apply = (
			snapshot: VisualizationSnapshot | null,
			settled: boolean,
			visibleChanged = true,
			changedFixtureIds?: ReadonlySet<string> | null,
		) => {
			options.displayedVisualizationRef.current = snapshot;
			options.visualizationSettledRef.current = settled;
			applyStageVisualization(
				options.fixtures,
				snapshot,
				options.fixtureObjectsRef.current,
				options.showBeamGuides,
				options.renderQuality,
				virtualHighlight,
				selected,
				options.showSelection,
				changedFixtureIds ?? undefined,
			);
			options.appliedRenderQualityRef.current = options.renderQuality;
			recordFrameApplied(snapshot, settled, visibleChanged);
			// This apply already runs at the browser's chosen interpolation
			// boundary. Submitting here avoids adding a second animation-frame
			// queue before the authoritative values reach the canvas.
			options.invalidateRef.current?.(true);
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
			// A dependency such as render quality or selection can require every
			// retained fixture to be revisited even when only a subset of values
			// changed between snapshots.
			const changedFixtureIds = forceVisibleApply
				? null
				: changedStageFixtureIds(from, target);
			// Large fan-out previews are latest-value and eventually consistent.
			// Applying hundreds of interpolated fixture mutations before the final
			// authoritative state creates a long WebView task and delays the desk.
			if (
				!shouldInterpolateStageSceneChanges(
					options.fixtures.length,
					changedFixtureIds,
				)
			) {
				apply(target, true, true, changedFixtureIds);
				return;
			}
			const interpolationMillis = remainingStageInterpolationMillis(
				target.generated_at,
			);
			if (interpolationMillis === 0) {
				apply(target, true, true, changedFixtureIds);
				return;
			}
			const startedAt = performance.now();
			let intermediateApplied = false;
			const step = (now: number) => {
				const progress = Math.min(
					1,
					Math.max(0, (now - startedAt) / interpolationMillis),
				);
				if (progress >= 1) apply(target, true, true, changedFixtureIds);
				else if (!intermediateApplied) {
					intermediateApplied = true;
					apply(
						interpolateVisualizationSnapshot(from, target, progress),
						false,
						true,
						changedFixtureIds,
					);
				}
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
