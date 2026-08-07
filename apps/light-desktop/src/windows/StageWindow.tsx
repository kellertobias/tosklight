import type { HighlightState } from "../api/types";
import { GroupStrip } from "../components/shared/GroupStrip";
import { useHighlightSnapshot } from "../features/highlight/HighlightState";
import { Stage2dView } from "./stageWindow/Stage2dView";
import { Stage3dView } from "./stageWindow/Stage3dView";
import { StageHeader } from "./stageWindow/StageHeader";
import type { StageWindowProps } from "./stageWindow/types";
import { useStageLayout } from "./stageWindow/useStageLayout";
import { useStageOptions } from "./stageWindow/useStageOptions";
import { useStageSelection } from "./stageWindow/useStageSelection";
import { useStageVisualization } from "./stageWindow/useStageVisualization";

export function StageWindow(props: StageWindowProps) {
	const active = props.active ?? true;
	const interactionActive = active && !props.viewOnly;
	const options = useStageOptions(props);
	const layout = useStageLayout();
	const selection = useStageSelection(interactionActive);
	const highlight = useHighlightSnapshot();
	const patchSelectionPreview = props.patchSelectionPreview ?? false;
	const stage = useStageVisualization(
		active,
		options.followPreload,
		patchSelectionPreview,
		layout,
		selection.fixtureIdSet,
		props.patchedFixtures,
		false,
		props.paneId ??
			(props.compact
				? `compact-stage-${options.followPreload ? "preload" : "live"}`
				: `stage-window-${options.followPreload ? "preload" : "live"}`),
		props.visualizationIntervalMillis,
	);
	const highlightFixtures = options.followPreload
		? []
		: stageHighlightFixtureIds(highlight);
	return (
		<div
			className={`stage-window ${props.compact ? "compact" : ""}`}
			data-visualization-state={
				stage.visualizationError
					? stage.visualization
						? "stale"
						: "unavailable"
					: "ready"
			}
			data-live-visualization-state={stage.visualizationStatus}
			data-visualization-lane={options.followPreload ? "preload" : "live"}
			data-visualization-revision={stage.visualization?.revision}
		>
			{!props.compact && (
				<StageHeader
					layout={layout}
					options={options}
					selectedCount={selection.fixtureIds.length}
					writable={!props.viewOnly}
				/>
			)}
			{/*
			 * A 2D Stage joins the renderer only while its layout is Automatic — which means the
			 * arrangement *is* the projection of the 3D positions, regenerated from them, so the
			 * renderer's own plan view of the same rig is the same picture. A Manual layout is
			 * somewhere an operator put every fixture by hand, and no projection reproduces that,
			 * so it stays with the desk's own drawing. That is the distinction Regenerate 2D
			 * layout exists to make deliberate, and it is not this pane's to make for them.
			 */}
			{options.view === "3d" ||
			options.view === "3d-viz" ||
			(options.view === "2d" &&
				layout.positions2dConfig?.provenance === "automatic") ? (
				<Stage3dView
					fixtures={stage.fixtures3d}
					visualization={stage.visualization}
					options={options}
					patchSelectionPreview={patchSelectionPreview}
					patchPreviewFixtures={stage.patchPreviewFixtures}
					highlightFixtures={highlightFixtures}
					camera3d={props.camera3d}
					pixelRatioCap={props.pixelRatioCap}
					selection={selection}
					active={active}
					paneId={props.paneId}
					interactive={!props.viewOnly}
					projection={layout.positions2dConfig?.projection}
				/>
			) : (
				<Stage2dView
					compact={props.compact}
					fixtures={stage.fixtures}
					layout={layout}
					options={options}
					selection={selection}
					patchedFixtures={stage.stageFixtures}
					patchSelectionPreview={patchSelectionPreview}
					patchPreviewFixtures={stage.patchPreviewFixtures}
					visualizationLane={options.followPreload ? "preload" : "normal"}
					visualizationActive={active}
					interactive={!props.viewOnly}
				/>
			)}
			{active && stage.visualizationError && (
				<div className="stage-visualization-state error" role="status">
					{stage.visualization
						? `${options.followPreload ? "Preload" : "Live"} visualization stale · reconnecting…`
						: "Visualization unavailable · reconnecting…"}
				</div>
			)}
			{options.groupsVisible && <GroupStrip active={active} />}
		</div>
	);
}

/** Immediate Stage-only projection of the authoritative Highlight selection. */
export function stageHighlightFixtureIds(
	highlight: HighlightState | null,
): string[] {
	if (!highlight?.active || !highlight.output_enabled) return [];
	if (highlight.mode === "step")
		return highlight.active_fixture
			? [highlight.active_fixture.fixture_id]
			: [];
	return highlight.remembered.map((fixture) => fixture.fixture_id);
}
