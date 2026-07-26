import { useEffect, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { ModalRegistration } from "@tosklight/ui/modals";

export function QuitConfirmOverlay() {
  const desktop = useDesktopBridge();
  const [visible, setVisible] = useState(false);
  const dismiss = () => { setVisible(false); void desktop.cancelQuit(); };
  useEffect(() => {
    if (!desktop.available) return;
    let cleanup: (() => void) | undefined; let cancelled = false;
    void desktop.onQuitRequested(() => setVisible(true)).then((unlisten) => { if (cancelled) unlisten(); else cleanup = unlisten; });
    return () => { cancelled = true; cleanup?.(); };
  }, [desktop]);
  if (!visible) return null;
  return <ModalRegistration onClose={dismiss}><div className="quit-confirm-cover" onPointerDown={dismiss}>
    <div>
      <h1>Really quit?</h1>
      <p>Press <kbd>⌘Q</kbd> again to quit ToskLight.</p>
      <small>Press Esc or click anywhere to keep it running.</small>
    </div>
  </div></ModalRegistration>;
}
