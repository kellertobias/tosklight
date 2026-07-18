import { useEffect, useRef } from "react";
import type { ScreenConfiguration } from "../../api/types";
import { useScreens } from "../../features/screens/ScreensContext";
import { type DesktopBridge, useDesktopBridge } from "../../platform/desktop";

async function reconcileScreen(
  desktop: DesktopBridge,
  screen: ScreenConfiguration,
  displays: ReadonlySet<string>,
) {
  if (!screen.desired_open) return desktop.closeConsoleScreen(screen.id);
  if (screen.display_id && !displays.has(screen.display_id)) return desktop.hideConsoleScreen(screen.id);
  return desktop.openConsoleScreen({
    screenId: screen.id,
    title: screen.name,
    displayId: screen.display_id,
    bounds: screen.bounds,
    fullscreen: screen.fullscreen,
  });
}

async function reconcileWindows(
  desktop: DesktopBridge,
  screens: readonly ScreenConfiguration[],
  cancelled: () => boolean,
) {
  const available = new Set((await desktop.listDisplays()).map((display) => display.id));
  for (const screen of screens) {
    if (cancelled()) return;
    await reconcileScreen(desktop, screen, available);
  }
}

function createReconciler(
  desktop: DesktopBridge,
  screens: () => readonly ScreenConfiguration[],
  cancelled: () => boolean,
) {
  let running = false;
  let requested = false;
  return async function request() {
    requested = true;
    if (running) return;
    running = true;
    try {
      while (requested && !cancelled()) {
        requested = false;
        await reconcileWindows(desktop, screens(), cancelled);
      }
    } finally {
      running = false;
    }
  };
}

export function ScreenWindowManager() {
  const desktop = useDesktopBridge();
  const screens = useScreens().screens;
  const screensRef = useRef(screens);
  const requestReconcile = useRef<() => void>(() => undefined);
  screensRef.current = screens;
  useEffect(() => {
    if (!desktop.available) return;
    let cancelled = false;
    const request = createReconciler(desktop, () => screensRef.current?.screens ?? [], () => cancelled);
    requestReconcile.current = () => void request();
    requestReconcile.current();
    const timer = window.setInterval(requestReconcile.current, 2_000);
    return () => {
      cancelled = true;
      requestReconcile.current = () => undefined;
      window.clearInterval(timer);
    };
  }, [desktop]);
  useEffect(() => requestReconcile.current(), [screens]);
  return null;
}
