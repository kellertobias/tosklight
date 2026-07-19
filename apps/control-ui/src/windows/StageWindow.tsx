import { useState } from "react";
import { useServer } from "../api/ServerContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { Stage2dView } from "./stageWindow/Stage2dView";
import { Stage3dView } from "./stageWindow/Stage3dView";
import { StageHeader } from "./stageWindow/StageHeader";
import type { StageWindowProps } from "./stageWindow/types";
import { useStageLayout } from "./stageWindow/useStageLayout";
import { useStageOptions } from "./stageWindow/useStageOptions";
import { useStageVisualization } from "./stageWindow/useStageVisualization";

export function StageWindow(props: StageWindowProps) {
	const active = props.active ?? true;
	const server = useServer();
	const options = useStageOptions(props);
	const layout = useStageLayout();
	const patchSelectionPreview = props.patchSelectionPreview ?? false;
	const stage = useStageVisualization(
		options.followPreload,
		patchSelectionPreview,
		layout,
		props.patchedFixtures,
	);
	const [setupFixtureId, setSetupFixtureId] = useState<string | null>(null);
	return (
		<div className={`stage-window ${props.compact ? "compact" : ""}`}>
			{!props.compact && (
				<StageHeader
					options={options}
					selectedCount={server.selectedFixtures.length}
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
				/>
			) : (
				<Stage2dView
					compact={props.compact}
					fixtures={stage.fixtures}
					layout={layout}
					options={options}
				/>
			)}
			{options.groupsVisible && <GroupStrip active={active} />}
		</div>
	);
}
