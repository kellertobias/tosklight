import { useRef, useState } from "react";
import { useApp } from "../../state/AppContext";
import type { StageOptionsModel, StageWindowProps } from "./types";

export function useStageOptions({
	compact,
	showGroupShortcuts,
	stageView,
	followPreload: paneFollowPreload,
	showBeamGuides: forcedShowBeamGuides,
	showSelection: forcedShowSelection,
	showFloorGrid: forcedShowFloorGrid,
	environmentBrightness: forcedEnvironmentBrightness,
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
		mode: state.stageMode,
		setMode: (value) => dispatch({ type: "SET_STAGE_MODE", value }),
		view: compact ? (stageView ?? state.stageView) : state.stageView,
		setView: (value) => dispatch({ type: "SET_STAGE_VIEW", value }),
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
