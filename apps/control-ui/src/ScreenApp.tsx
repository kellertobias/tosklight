import { useEffect, useRef, useState } from "react";
import { ServerProvider, useServer } from "./api/ServerContext";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { AppProvider, useApp } from "./state/AppContext";
import { LeftDock } from "./components/shell/LeftDock";
import { WorkspaceView } from "./components/shell/WorkspaceView";
import { PlaybackFaderBank } from "./components/control/PlaybackFaderBank";
import type { ScreenConfiguration } from "./api/types";
import { Button } from "./components/common";
import { NativeDragStrip } from "./components/shell/NativeDragStrip";
import { canAdvancePlaybackPage, nextPlaybackPageNumber } from "./components/control/PlaybackPageDialogs";

function ScreenPageControls({ screen, page }: { screen: ScreenConfiguration; page: number }) {
  const server = useServer();
  const [picker, setPicker] = useState(false);
  const pages = server.playbacks?.pages ?? [];
  const setPage = (next: number) => screen.page_mode === "independent" && void server.setScreenPage(screen.id, next);
  const advance = async () => {
    const next = page + 1;
    if (!pages.some((item) => item.number === next) && !await server.savePlaybackPage({ number: next, name: `Page ${next}`, slots: {} })) return;
    setPage(next);
  };
  const addPage = async () => {
    const next = nextPlaybackPageNumber(pages);
    if (next == null || !await server.savePlaybackPage({ number: next, name: `Page ${next}`, slots: {} })) return;
    setPage(next);
    setPicker(false);
  };
  return (
    <div className="screen-page-controls">
      <Button disabled={screen.page_mode !== "independent" || page <= 1} onClick={() => setPage(page - 1)}>
        ▲ PAGE UP
      </Button>
      <Button onClick={() => screen.page_mode === "independent" && setPicker(true)}>
        <strong>{page}</strong>
        <span>{server.playbacks?.pages.find((item) => item.number === page)?.name ?? `Page ${page}`}</span>
      </Button>
      <Button disabled={screen.page_mode !== "independent" || !canAdvancePlaybackPage(pages, page)} onClick={() => void advance()}>
        PAGE DOWN ▼
      </Button>
      {picker && (
        <div className="screen-page-picker">
          <Button onClick={() => setPicker(false)}>×</Button>
          {(server.playbacks?.pages ?? []).map((item) => (
            <Button
              className={item.number === page ? "active" : ""}
              key={item.number}
              onClick={() => {
                setPage(item.number);
                setPicker(false);
              }}
            >
              {item.number} · {item.name}
            </Button>
          ))}
          <Button disabled={nextPlaybackPageNumber(pages) == null} onClick={() => void addPage()}>Add new page</Button>
        </div>
      )}
    </div>
  );
}

function ScreenSurface({ id }: { id: string }) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const screen = server.screens?.screens.find((item) => item.id === id);
  const hydrated = useRef(false);
  const screenRef = useRef(screen);
  const closing = useRef(false);
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
  useEffect(() => {
    if (!screen || !("__TAURI_INTERNALS__" in window)) return;
    let cleanups: Array<() => void> = [];
    let timer: number | undefined;
    let shuttingDown = false;
    void Promise.all([import("@tauri-apps/api/window"), import("@tauri-apps/api/event")]).then(async ([{ getCurrentWindow, currentMonitor }, { listen }]) => {
      const current = getCurrentWindow();
      cleanups.push(
        await listen("app-shutting-down", () => {
          shuttingDown = true;
        }),
      );
      const persist = () => {
        window.clearTimeout(timer);
        if (closing.current) return;
        timer = window.setTimeout(async () => {
          if (closing.current) return;
          const position = await current.outerPosition();
          const size = await current.outerSize();
          const scale = await current.scaleFactor();
          const fullscreen = await current.isFullscreen();
          const monitor = await currentMonitor();
          const latest = screenRef.current;
          if (!latest || closing.current) return;
          const display_id = monitor ? `${monitor.name ?? "Display"}|${monitor.position.x},${monitor.position.y}|${monitor.size.width}x${monitor.size.height}` : latest.display_id;
          void server.saveScreen({
            ...latest,
            display_id,
            bounds: {
              x: position.x / scale,
              y: position.y / scale,
              width: size.width / scale,
              height: size.height / scale,
            },
            fullscreen,
          });
        }, 300);
      };
      cleanups.push(
        await current.onMoved(persist),
        await current.onResized(persist),
        await current.onCloseRequested(async (event) => {
          event.preventDefault();
          closing.current = true;
          window.clearTimeout(timer);
          const latest = screenRef.current;
          if (!shuttingDown && latest) await server.saveScreen({ ...latest, desired_open: false });
          await current.destroy();
        }),
      );
      persist();
    });
    return () => {
      window.clearTimeout(timer);
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [screen?.id]);
  if (!screen) return <main className="screen-loading">Loading screen…</main>;
  const page = server.screens?.active_pages[id] ?? server.playbacks?.active_page ?? 1;
  return (
    <div className={`screen-shell ${screen.show_dock ? "with-dock" : ""} ${screen.show_playbacks ? "with-playbacks" : ""}`}>
      <NativeDragStrip />
      {screen.show_dock && <LeftDock />}
      <WorkspaceView />
      {screen.show_playbacks && (
        <section className="screen-playbacks">
          <PlaybackFaderBank pageNumber={page} firstSlot={screen.first_playback_slot} count={screen.playback_count} rows={screen.playback_rows} playbackLayout={screen.playback_layout} />
          {screen.show_page_controls && <ScreenPageControls screen={screen} page={page} />}
        </section>
      )}
    </div>
  );
}

export function ScreenApp({ id }: { id: string }) {
  return (
    <ServerProvider>
      <AppProvider>
        <ScreenSurface id={id} />
      </AppProvider>
      <DeskLockOverlay />
    </ServerProvider>
  );
}
