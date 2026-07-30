import { useEffect, useRef } from "react";
import { useDeskConnection } from "../../features/deskConnection/DeskConnectionContext";
import {
	useActiveShowId,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { deskLayoutScopeKey } from "../../features/server/contracts";
import { useApp } from "../../state/AppContext";

export function LayoutPersistence() {
	const connection = useDeskConnection();
	const session = useSessionSnapshot();
	const activeShowId = useActiveShowId();
	const { state, dispatch } = useApp();
	const hydratedScope = useRef<string | null>(null);
	const skipInitialSave = useRef<string | null>(null);
	const saveDeskLayout = useRef(
		connection?.saveDeskLayout ?? (async () => undefined),
	);
	const scope = deskLayoutScopeKey(activeShowId ?? undefined, session?.user.id);

	useEffect(() => {
		if (connection) saveDeskLayout.current = connection.saveDeskLayout;
	}, [connection]);

	useEffect(() => {
		if (
			!scope ||
			connection?.deskLayoutScope !== scope ||
			hydratedScope.current === scope
		)
			return;
		hydratedScope.current = scope;
		skipInitialSave.current = scope;
		if (connection?.deskLayout) {
			dispatch({
				type: "HYDRATE_LAYOUT",
				desks: connection?.deskLayout.body.desks,
				activeDeskId: connection?.deskLayout.body.activeDeskId,
				windowSettings: connection?.deskLayout.body.windowSettings,
			});
		}
	}, [scope, connection?.deskLayout, connection?.deskLayoutScope, dispatch]);

	useEffect(() => {
		if (
			!scope ||
			connection?.deskLayoutScope !== scope ||
			hydratedScope.current !== scope
		)
			return;
		if (skipInitialSave.current === scope) {
			skipInitialSave.current = null;
			return;
		}
		const timer = window.setTimeout(
			() =>
				void saveDeskLayout.current({
					desks: state.desks,
					activeDeskId: state.activeDeskId,
					windowSettings: {
						dockMode: state.dockMode,
						builtIn: state.builtIn,
						lastBuiltIn: state.lastBuiltIn,
						presetFamily: state.presetFamily,
						presetPoolColors: state.presetPoolColors,
						playbackColumns: state.playbackColumns,
						playbackRows: state.playbackRows,
						playbackPage: state.playbackPage,
						stageMode: state.stageMode,
						stageView: state.stageView,
						stageZoom: state.stageZoom,
						stagePanX: state.stagePanX,
						stagePanY: state.stagePanY,
						stageOrbitX: state.stageOrbitX,
						stageOrbitY: state.stageOrbitY,
						stageGroupsVisible: state.stageGroupsVisible,
						stageShowSelection: state.stageShowSelection,
						stageShowFloorGrid: state.stageShowFloorGrid,
						stageShowBeamGuides: state.stageShowBeamGuides,
						stageRenderQuality: state.stageRenderQuality,
						stageEnvironmentBrightness: state.stageEnvironmentBrightness,
						layoutGroupId: state.layoutGroupId,
						dmxDotSize: state.dmxDotSize,
						fixtureSheetOrder: state.fixtureSheetOrder,
						fixtureSheetActiveOnly: state.fixtureSheetActiveOnly,
						fixtureSheetCueListId: state.fixtureSheetCueListId,
						fixtureSheetColumns: state.fixtureSheetColumns,
						fixtureSheetShowType: state.fixtureSheetShowType,
						fixtureSheetIncludedHeads: state.fixtureSheetIncludedHeads,
						fixtureGroupsVisible: state.fixtureGroupsVisible,
						presetGroupsVisible: state.presetGroupsVisible,
					},
				}),
			600,
		);
		return () => window.clearTimeout(timer);
	}, [
		state.desks,
		state.activeDeskId,
		state.dockMode,
		state.builtIn,
		state.lastBuiltIn,
		state.presetFamily,
		state.presetPoolColors,
		state.playbackColumns,
		state.playbackRows,
		state.playbackPage,
		state.stageMode,
		state.stageView,
		state.stageZoom,
		state.stagePanX,
		state.stagePanY,
		state.stageOrbitX,
		state.stageOrbitY,
		state.stageGroupsVisible,
		state.stageShowSelection,
		state.stageShowFloorGrid,
		state.stageShowBeamGuides,
		state.stageRenderQuality,
		state.stageEnvironmentBrightness,
		state.layoutGroupId,
		state.dmxDotSize,
		state.fixtureSheetOrder,
		state.fixtureSheetActiveOnly,
		state.fixtureSheetCueListId,
		state.fixtureSheetColumns,
		state.fixtureSheetShowType,
		state.fixtureSheetIncludedHeads,
		state.fixtureGroupsVisible,
		state.presetGroupsVisible,
		scope,
		connection?.deskLayoutScope,
	]);

	return null;
}
