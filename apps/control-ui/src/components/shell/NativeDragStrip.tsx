export function NativeDragStrip() {
  const startDragging = (event: React.PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || !("__TAURI_INTERNALS__" in window)) return;
    event.preventDefault();
    void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => getCurrentWindow().startDragging());
  };
  return <div className="native-drag-strip" data-tauri-drag-region onPointerDown={startDragging} aria-hidden="true" />;
}
