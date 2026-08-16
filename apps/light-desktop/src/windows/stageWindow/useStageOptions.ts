import { useRef, useState } from "react";
import { useApp } from "../../state/AppContext";
import type { StageOptionsModel, StageWindowProps } from "./types";

export function useStageOptions({
	compact,
	showGroupShortcuts,
	stageView,
	stage2dSide,
	followPreload: paneFollowPreload,
	showSelection: forcedShowSelection,
	showFloorGrid: forcedShowFloorGrid,
	showBeamGuides: forcedShowBeamGuides,
	environmentBrightness: forcedEnvironmentBrightness,
	paneId,
}: StageWindowProps): StageOptionsModel {
	const { state, dispatch } = useApp();
	const [dedicatedFollowPreload, setDedicatedFollowPreload] = useState(false);
	const lastFollowToggle = useRef(0);
	const toggleFollowPreload = () => {
		const now = performance.now();
		if (now - lastFollowToggle.current < 400) return;
		lastFollowToggle.current = now;
		setDedicatedFollowPreload((current) => !current);
	};
	return {
		paneId,
		mode: state.stageMode,
		setMode: (value) => dispatch({ type: "SET_STAGE_MODE", value }),
		view: compact ? (stageView ?? state.stageView) : state.stageView,
		setView: (value) => dispatch({ type: "SET_STAGE_VIEW", value }),
		side2d: compact ? (stage2dSide ?? state.stage2dSide) : state.stage2dSide,
		setSide2d: (side2d) => dispatch({ type: "SET_STAGE_OPTIONS", side2d }),
		followPreload: compact
			? Boolean(paneFollowPreload)
			: dedicatedFollowPreload,
		toggleFollowPreload,
		groupsVisible: compact
			? Boolean(showGroupShortcuts)
			: state.stageGroupsVisible,
		showSelection: forcedShowSelection ?? state.stageShowSelection,
		showFloorGrid: forcedShowFloorGrid ?? state.stageShowFloorGrid,
		showBeamGuides: forcedShowBeamGuides ?? state.stageShowBeamGuides,
		environmentBrightness:
			forcedEnvironmentBrightness ?? state.stageEnvironmentBrightness,
	};
}
