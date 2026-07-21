import { useEffect, useRef } from "react";
import { ServerProvider } from "./api/ServerContext";
import { useScreens } from "./features/screens/ScreensContext";
import { ScreenPlaybackSection } from "./features/screens/ScreenPlaybackSection";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { AppProvider, useApp } from "./state/AppContext";
import { LeftDock } from "./components/shell/LeftDock";
import { WorkspaceView } from "./components/shell/WorkspaceView";
import { NativeDragStrip } from "./components/shell/NativeDragStrip";
import { useScreenWindowPersistence } from "./platform/desktop";

function ScreenSurface({ id }: { id: string }) {
  const server = useScreens();
  const { state, dispatch } = useApp();
  const screen = server.screens?.screens.find((item) => item.id === id);
  const hydrated = useRef(false);
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
  if (!screen) return <main className="screen-loading">Loading screen…</main>;
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
    <ServerProvider sessionRole="secondary">
      <AppProvider>
        <ScreenSurface id={id} />
      </AppProvider>
      <DeskLockOverlay />
    </ServerProvider>
  );
}
