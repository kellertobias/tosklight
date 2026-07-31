import { GroupStrip } from "../components/shared/GroupStrip";
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
			{options.view === "3d" ? (
				<Stage3dView
					fixtures={stage.fixtures3d}
					visualization={stage.visualization}
					options={options}
					patchSelectionPreview={patchSelectionPreview}
					patchPreviewFixtures={stage.patchPreviewFixtures}
					camera3d={props.camera3d}
					pixelRatioCap={props.pixelRatioCap}
					selection={selection}
					active={active}
					paneId={props.paneId}
					interactive={!props.viewOnly}
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
