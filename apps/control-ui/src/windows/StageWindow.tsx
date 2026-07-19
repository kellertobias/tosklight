import { useState } from "react";
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
	const options = useStageOptions(props);
	const layout = useStageLayout();
	const selection = useStageSelection(active);
	const patchSelectionPreview = props.patchSelectionPreview ?? false;
	const stage = useStageVisualization(
		active,
		options.followPreload,
		patchSelectionPreview,
		layout,
		selection.fixtureIdSet,
		props.patchedFixtures,
	);
	const [setupFixtureId, setSetupFixtureId] = useState<string | null>(null);
	return (
		<div className={`stage-window ${props.compact ? "compact" : ""}`}>
			{!props.compact && (
				<StageHeader
					options={options}
					selectedCount={selection.fixtureIds.length}
				/>
			)}
			{options.view === "3d" ? (
				<Stage3dView
					fixtures={stage.fixtures3d}
					visualization={stage.visualization}
					options={options}
					layout={layout}
					patchSelectionPreview={patchSelectionPreview}
					patchPreviewFixtures={stage.patchPreviewFixtures}
					camera3d={props.camera3d}
					setupFixtureId={setupFixtureId}
					setSetupFixtureId={setSetupFixtureId}
					selection={selection}
				/>
			) : (
				<Stage2dView
					compact={props.compact}
					fixtures={stage.fixtures}
					layout={layout}
					options={options}
					selection={selection}
				/>
			)}
			{options.groupsVisible && <GroupStrip active={active} />}
		</div>
	);
}
