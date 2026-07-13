import { useEffect, useState } from "react";
import { Button } from "../common";

const nativeWindow = () => import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow());

export function NativeDragStrip() {
  const [fullscreen, setFullscreen] = useState(false);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let active = true;
    void nativeWindow().then(async (current) => {
      const next = await current.isFullscreen();
      if (active) setFullscreen(next);
    });
    return () => { active = false; };
  }, []);

  const closeWindow = () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void nativeWindow().then((current) => current.close());
  };
  const toggleFullscreen = () => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void nativeWindow().then(async (current) => {
      const next = !(await current.isFullscreen());
      await current.setFullscreen(next);
      setFullscreen(next);
    });
  };
  const startDragging = (event: React.PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0 || !("__TAURI_INTERNALS__" in window)) return;
    event.preventDefault();
    void nativeWindow().then((current) => current.startDragging());
  };
  return <div className="native-drag-strip" aria-label="Window controls">
    <Button className="native-window-close" aria-label="Close window" title="Close window" onClick={closeWindow}>
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" /></svg>
    </Button>
    <Button aria-label={fullscreen ? "Exit fullscreen" : "Enter fullscreen"} aria-pressed={fullscreen} title={fullscreen ? "Exit fullscreen" : "Enter fullscreen"} onClick={toggleFullscreen}>
      {fullscreen
        ? <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 2v4H2M10 2v4h4M6 14v-4H2M10 14v-4h4" /></svg>
        : <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 2H2v4M10 2h4v4M6 14H2v-4M10 14h4v-4" /></svg>}
    </Button>
    <Button className="native-window-drag" data-tauri-drag-region aria-label="Move window" title="Drag to move window" onPointerDown={startDragging}>
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 1v14M1 8h14M8 1L6 3m2-2 2 2M8 15l-2-2m2 2 2-2M1 8l2-2M1 8l2 2m12-2-2-2m2 2-2 2" /></svg>
    </Button>
  </div>;
}
