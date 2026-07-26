import { useEffect, useRef, useState } from "react";
import { ServerRuntime } from "./api/ServerRuntime";
import { useScreens } from "./features/screens/ScreensContext";
import { ScreenPlaybackSection } from "./features/screens/ScreenPlaybackSection";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { AppProvider, useApp } from "./state/AppContext";
import { LeftDock } from "./components/shell/LeftDock";
import { WorkspaceView } from "./components/shell/WorkspaceView";
import { NativeDragStrip } from "./components/shell/NativeDragStrip";
import { useScreenWindowPersistence } from "./platform/desktop";
import { LoadingSurface } from "./components/common/LoadingSurface";
import { ConnectionState } from "./components/shell/ConnectionState";
import { DeskLoadingOverlay } from "./components/shell/DeskLoadingOverlay";
import { PatchFeatureBoundary } from "./features/patch/PatchFeatureBoundary";

function ScreenSurface({ id }: { id: string }) {
  const server = useScreens();
  const { state, dispatch } = useApp();
  const screen = server.screens?.screens.find((item) => item.id === id);
  const hydrated = useRef(false);
  const [layoutReady, setLayoutReady] = useState(false);
  const screenRef = useRef(screen);
  const closing = useScreenWindowPersistence(screen, server.saveScreen);
  screenRef.current = screen;
  useEffect(() => {
    if (!screen || hydrated.current) return;
    dispatch({
      type: "HYDRATE_LAYOUT",
      desks: screen.layout.desks,
      activeDeskId: screen.layout.activeDeskId,
    });
    hydrated.current = true;
    setLayoutReady(true);
  }, [screen, dispatch]);
  useEffect(() => {
    const currentScreen = screenRef.current;
    if (!currentScreen || !hydrated.current || closing.current) return;
    const timer = window.setTimeout(() => {
      const latest = screenRef.current;
      if (latest && !closing.current)
        void server.saveScreen({
          ...latest,
          layout: { desks: state.desks, activeDeskId: state.activeDeskId },
        });
    }, 600);
    return () => window.clearTimeout(timer);
  }, [state.desks, state.activeDeskId]);
  if (!screen || !layoutReady)
    return (
      <LoadingSurface
        className="connection-cover"
        showMark
        title="Loading screen…"
        detail="Hydrating the assigned desktop and Playback surface"
      />
    );
  return (
    <div className={`screen-shell ${screen.show_dock ? "with-dock" : ""} ${screen.show_playbacks ? "with-playbacks" : ""}`}>
      <NativeDragStrip />
      {screen.show_dock && <LeftDock />}
      <WorkspaceView />
      {screen.show_playbacks && <ScreenPlaybackSection screen={screen} />}
    </div>
  );
}

export function ScreenApp({ id }: { id: string }) {
  return (
    <ServerRuntime sessionRole="secondary">
      <AppProvider>
        <PatchFeatureBoundary>
          <ScreenSurface id={id} />
          <ConnectionState />
          <DeskLoadingOverlay />
        </PatchFeatureBoundary>
      </AppProvider>
      <DeskLockOverlay />
    </ServerRuntime>
  );
}
