import { AppProvider } from "./state/AppContext";
import { AppShell } from "./components/shell/AppShell";
import { QuitConfirmOverlay } from "./components/modals/QuitConfirmOverlay";
import { ServerProvider, useServer } from "./api/ServerContext";
import { useEffect } from "react";
import { DeskLockOverlay } from "./components/modals/DeskLockOverlay";
import { FileManagerPickerHost } from "./windows/FileManagerPickerHost";

function DesktopReady() {
  const server = useServer();
  useEffect(() => {
    if (server.status !== "connected" || !server.bootstrap || !("__TAURI_INTERNALS__" in window)) return;
    void import("@tauri-apps/api/core").then(({ invoke }) => invoke("frontend_ready"));
  }, [server.status, server.bootstrap]);
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
