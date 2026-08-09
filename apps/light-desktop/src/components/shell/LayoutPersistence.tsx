import { useEffect, useRef } from "react";
import { useDeskConnection } from "../../features/deskConnection/DeskConnectionContext";
import {
	useActiveShowId,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { deskLayoutScopeKey } from "../../features/server/contracts";
import { useApp } from "../../state/AppContext";
import type { AppState } from "../../types";
import {
	collectFixtureSheetCompactModes,
	desksWithoutFixtureSheetCompactModes,
	fixtureSheetCompactModeStorageKey,
	readFixtureSheetCompactModes,
} from "./fixtureSheetCompactModePersistence";

function persistedWindowSettings(state: AppState) {
	return {
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
		stage2dSide: state.stage2dSide,
		stageVizBackground: state.stageVizBackground,
		stageVizQuality: state.stageVizQuality,
		stageVizAtmosphere: state.stageVizAtmosphere,
		stageVizExposure: state.stageVizExposure,
		stageVizLaserBrightness: state.stageVizLaserBrightness,
		stageVizShowLabels: state.stageVizShowLabels,
		stageEnvironmentBrightness: state.stageEnvironmentBrightness,
		dmxDotSize: state.dmxDotSize,
		fixtureSheetOrder: state.fixtureSheetOrder,
		fixtureSheetActiveOnly: state.fixtureSheetActiveOnly,
		fixtureSheetCueListId: state.fixtureSheetCueListId,
		fixtureSheetColumns: state.fixtureSheetColumns,
		fixtureSheetShowType: state.fixtureSheetShowType,
		fixtureSheetIncludedHeads: state.fixtureSheetIncludedHeads,
		fixtureGroupsVisible: state.fixtureGroupsVisible,
		presetGroupsVisible: state.presetGroupsVisible,
	};
}

function compactStorageScope(showId?: string | null, deskId?: string | null) {
	return showId && deskId
		? fixtureSheetCompactModeStorageKey(showId, deskId)
		: null;
}

export function LayoutPersistence() {
	const connection = useDeskConnection();
	const session = useSessionSnapshot();
	const activeShowId = useActiveShowId();
	const { state, dispatch } = useApp();
	const hydratedScope = useRef<string | null>(null);
	const skipInitialSave = useRef<string | null>(null);
	const hydratedCompactScope = useRef<string | null>(null);
	const skipInitialCompactSave = useRef<string | null>(null);
	const saveDeskLayout = useRef(
		connection?.saveDeskLayout ?? (async () => undefined),
	);
	const scope = deskLayoutScopeKey(activeShowId ?? undefined, session?.user.id);
	const compactScope = compactStorageScope(activeShowId, session?.desk.id);
	const portableDesksSignature = JSON.stringify(
		desksWithoutFixtureSheetCompactModes(state.desks),
	);
	const compactModesJson = JSON.stringify(
		collectFixtureSheetCompactModes(state),
	);

	useEffect(() => {
		if (connection) saveDeskLayout.current = connection.saveDeskLayout;
	}, [connection]);

	useEffect(() => {
		if (compactScope) return;
		hydratedCompactScope.current = null;
		skipInitialCompactSave.current = null;
	}, [compactScope]);

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
			!compactScope ||
			!activeShowId ||
			!session?.desk.id ||
			connection?.deskLayoutScope !== scope ||
			hydratedCompactScope.current === compactScope
		)
			return;
		hydratedCompactScope.current = compactScope;
		skipInitialCompactSave.current = compactScope;
		dispatch({
			type: "HYDRATE_FIXTURE_SHEET_COMPACT_MODES",
			...readFixtureSheetCompactModes(activeShowId, session.desk.id),
		});
	}, [
		activeShowId,
		compactScope,
		connection?.deskLayoutScope,
		dispatch,
		scope,
		session?.desk.id,
	]);

	useEffect(() => {
		if (!compactScope || hydratedCompactScope.current !== compactScope) return;
		if (skipInitialCompactSave.current === compactScope) {
			skipInitialCompactSave.current = null;
			return;
		}
		localStorage.setItem(compactScope, compactModesJson);
	}, [compactModesJson, compactScope]);

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
					desks: desksWithoutFixtureSheetCompactModes(state.desks),
					activeDeskId: state.activeDeskId,
					windowSettings: persistedWindowSettings(state),
				}),
			600,
		);
		return () => window.clearTimeout(timer);
	}, [
		portableDesksSignature,
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
		state.stage2dSide,
		state.stageVizBackground,
		state.stageVizQuality,
		state.stageVizAtmosphere,
		state.stageVizExposure,
		state.stageVizLaserBrightness,
		state.stageVizShowLabels,
		state.stageEnvironmentBrightness,
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
