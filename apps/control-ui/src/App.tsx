import { AppProvider } from "./state/AppContext";
import { AppShell } from "./components/shell/AppShell";
import { QuitConfirmOverlay } from "./components/modals/QuitConfirmOverlay";
import { ServerProvider, useServer } from "./api/ServerContext";
import { useEffect } from "react";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { FileManagerPickerHost } from "./windows/FileManagerPickerHost";
import { useDesktopBridge } from "./platform/desktop";

function DesktopReady() {
  const server = useServer();
  const desktop = useDesktopBridge();
  useEffect(() => {
    if (server.status !== "connected" || !server.bootstrap || !desktop.available) return;
    void desktop.frontendReady();
  }, [server.status, server.bootstrap, desktop]);
  return null;
}

export function App() {
  return (
    <ServerProvider>
      <DesktopReady />
      <AppProvider>
        <AppShell />
        <QuitConfirmOverlay />
        <FileManagerPickerHost />
      </AppProvider>
      <DeskLockOverlay />
    </ServerProvider>
  );
}
