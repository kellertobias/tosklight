import { useEffect, useRef } from "react";
import { deskLayoutScopeKey, useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export function LayoutPersistence() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const hydratedScope = useRef<string | null>(null);
  const skipInitialSave = useRef<string | null>(null);
  const saveDeskLayout = useRef(server.saveDeskLayout);
  const scope = deskLayoutScopeKey(server.bootstrap?.active_show?.id, server.session?.user.id);

  useEffect(() => {
    saveDeskLayout.current = server.saveDeskLayout;
  }, [server.saveDeskLayout]);

  useEffect(() => {
    if (!scope || server.deskLayoutScope !== scope || hydratedScope.current === scope) return;
    hydratedScope.current = scope;
    skipInitialSave.current = scope;
    if (server.deskLayout) {
      dispatch({
        type: "HYDRATE_LAYOUT",
        desks: server.deskLayout.body.desks,
        activeDeskId: server.deskLayout.body.activeDeskId,
        windowSettings: server.deskLayout.body.windowSettings,
      });
    }
  }, [scope, server.deskLayout, server.deskLayoutScope, dispatch]);

  useEffect(() => {
    if (!scope || server.deskLayoutScope !== scope || hydratedScope.current !== scope) return;
    if (skipInitialSave.current === scope) {
      skipInitialSave.current = null;
      return;
    }
    const timer = window.setTimeout(() => void saveDeskLayout.current({ desks: state.desks, activeDeskId: state.activeDeskId, windowSettings: {
      dockMode: state.dockMode, builtIn: state.builtIn, lastBuiltIn: state.lastBuiltIn, presetFamily: state.presetFamily, presetPoolColors: state.presetPoolColors,
      playbackColumns: state.playbackColumns, playbackRows: state.playbackRows, playbackPage: state.playbackPage,
      stageMode: state.stageMode, stageView: state.stageView, stageZoom: state.stageZoom, stagePanX: state.stagePanX, stagePanY: state.stagePanY,
      stageOrbitX: state.stageOrbitX, stageOrbitY: state.stageOrbitY, stageGroupsVisible: state.stageGroupsVisible,
      stageShowSelection: state.stageShowSelection, stageShowFloorGrid: state.stageShowFloorGrid, stageShowBeamGuides: state.stageShowBeamGuides, stageEnvironmentBrightness: state.stageEnvironmentBrightness, dmxDotSize: state.dmxDotSize,
      fixtureSheetOrder: state.fixtureSheetOrder, fixtureSheetActiveOnly: state.fixtureSheetActiveOnly, fixtureSheetCueListId: state.fixtureSheetCueListId,
      fixtureSheetColumns: state.fixtureSheetColumns, fixtureSheetShowType: state.fixtureSheetShowType, fixtureSheetShowPatch: state.fixtureSheetShowPatch,
      fixtureSheetShowSubheads: state.fixtureSheetShowSubheads, fixtureSheetShowMasterHeads: state.fixtureSheetShowMasterHeads,
      fixtureGroupsVisible: state.fixtureGroupsVisible, presetGroupsVisible: state.presetGroupsVisible,
    } }), 600);
    return () => window.clearTimeout(timer);
  }, [state.desks, state.activeDeskId, state.dockMode, state.builtIn, state.lastBuiltIn, state.presetFamily, state.presetPoolColors, state.playbackColumns, state.playbackRows, state.playbackPage, state.stageMode, state.stageView, state.stageZoom, state.stagePanX, state.stagePanY, state.stageOrbitX, state.stageOrbitY, state.stageGroupsVisible, state.stageShowSelection, state.stageShowFloorGrid, state.stageShowBeamGuides, state.stageEnvironmentBrightness, state.dmxDotSize, state.fixtureSheetOrder, state.fixtureSheetActiveOnly, state.fixtureSheetCueListId, state.fixtureSheetColumns, state.fixtureSheetShowType, state.fixtureSheetShowPatch, state.fixtureSheetShowSubheads, state.fixtureSheetShowMasterHeads, state.fixtureGroupsVisible, state.presetGroupsVisible, scope, server.deskLayoutScope]);

  return null;
}
