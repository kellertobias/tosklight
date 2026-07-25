import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { Button } from "../common";

export function NativeDragStrip() {
  const desktop = useDesktopBridge();
  const [fullscreen, setFullscreen] = useState(false);

  useEffect(() => {
    if (!desktop.available) return;
    let active = true;
    void desktop.currentWindowFullscreen().then((next) => {
      if (active) setFullscreen(next);
    });
    return () => { active = false; };
  }, [desktop]);

  const closeWindow = () => {
    if (desktop.available) void desktop.closeCurrentWindow();
  };
  const toggleFullscreen = () => {
    if (!desktop.available) return;
    void desktop.currentWindowFullscreen().then(async (current) => {
      const next = !current;
      await desktop.setCurrentWindowFullscreen(next);
      setFullscreen(next);
    });
  };
  const startDragging = (event: React.PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0 || !desktop.available) return;
    event.preventDefault();
    void desktop.startCurrentWindowDrag();
  };
  return <div className="native-drag-strip">
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
