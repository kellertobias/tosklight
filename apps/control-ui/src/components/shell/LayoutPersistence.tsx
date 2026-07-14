import { useEffect, useRef } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";

export function LayoutPersistence() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const appliedRevision = useRef<number | null>(null);
  const hydrated = useRef(false);
  const saveDeskLayout = useRef(server.saveDeskLayout);

  useEffect(() => {
    saveDeskLayout.current = server.saveDeskLayout;
  }, [server.saveDeskLayout]);

  useEffect(() => {
    if (!server.deskLayout) { if (server.bootstrap?.active_show) hydrated.current = true; return; }
    if (appliedRevision.current === server.deskLayout.revision) return;
    appliedRevision.current = server.deskLayout.revision;
    if (hydrated.current) return;
    hydrated.current = true;
    dispatch({
      type: "HYDRATE_LAYOUT",
      desks: server.deskLayout.body.desks,
      activeDeskId: server.deskLayout.body.activeDeskId,
      windowSettings: server.deskLayout.body.windowSettings,
    });
  }, [server.deskLayout, server.bootstrap?.active_show, dispatch]);

  useEffect(() => {
    if (!hydrated.current || !server.bootstrap?.active_show) return;
    const timer = window.setTimeout(() => void saveDeskLayout.current({ desks: state.desks, activeDeskId: state.activeDeskId, windowSettings: {
      dockMode: state.dockMode, builtIn: state.builtIn, lastBuiltIn: state.lastBuiltIn, presetFamily: state.presetFamily, presetPoolColors: state.presetPoolColors,
      playbackColumns: state.playbackColumns, playbackRows: state.playbackRows, playbackPage: state.playbackPage,
      stageMode: state.stageMode, stageView: state.stageView, stageZoom: state.stageZoom, stagePanX: state.stagePanX, stagePanY: state.stagePanY,
      stageOrbitX: state.stageOrbitX, stageOrbitY: state.stageOrbitY, stageGroupsVisible: state.stageGroupsVisible,
      stageShowSelection: state.stageShowSelection, stageEnvironmentBrightness: state.stageEnvironmentBrightness, dmxDotSize: state.dmxDotSize,
      fixtureGroupsVisible: state.fixtureGroupsVisible, presetGroupsVisible: state.presetGroupsVisible,
    } }), 600);
    return () => window.clearTimeout(timer);
  }, [state.desks, state.activeDeskId, state.dockMode, state.builtIn, state.lastBuiltIn, state.presetFamily, state.presetPoolColors, state.playbackColumns, state.playbackRows, state.playbackPage, state.stageMode, state.stageView, state.stageZoom, state.stagePanX, state.stagePanY, state.stageOrbitX, state.stageOrbitY, state.stageGroupsVisible, state.stageShowSelection, state.stageEnvironmentBrightness, state.dmxDotSize, state.fixtureGroupsVisible, state.presetGroupsVisible, server.bootstrap?.active_show?.id]);

  return null;
}
