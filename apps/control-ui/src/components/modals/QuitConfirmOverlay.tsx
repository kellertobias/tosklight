import { useEffect, useState } from "react";

export function QuitConfirmOverlay() {
  const [visible, setVisible] = useState(false);
  const dismiss = () => { setVisible(false); void import("@tauri-apps/api/core").then(({ invoke }) => invoke("cancel_quit")); };
  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    let cleanup: (() => void) | undefined; let cancelled = false;
    void import("@tauri-apps/api/event").then(({ listen }) => listen("quit-requested", () => setVisible(true))).then((unlisten) => { if (cancelled) unlisten(); else cleanup = unlisten; });
    return () => { cancelled = true; cleanup?.(); };
  }, []);
  useEffect(() => {
    if (!visible) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") dismiss(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [visible]);
  if (!visible) return null;
  return <div className="quit-confirm-cover" onClick={dismiss}>
    <div>
      <h1>Really quit?</h1>
      <p>Press <kbd>⌘Q</kbd> again to quit ToskLight.</p>
      <small>Press Esc or click anywhere to keep it running.</small>
    </div>
  </div>;
}
