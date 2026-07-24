import { useConnectionStatus, useServerError } from "../../features/shellStatus/ShellStatusState";
import { useEffect, useMemo, useState } from "react";
import { useDeskConnection } from "../../features/deskConnection/DeskConnectionContext";
import { useBootstrapReady } from "../../features/deskSnapshot/DeskSnapshotState";
import { configuredServerUrl } from "../../api/client/serverLocation";
import { Button, LoadingSurface, TextField } from "../common";
import { useDesktopBridge } from "../../platform/desktop";

export function ConnectionState() {
  const connection = useDeskConnection();
  const bootstrapReady = useBootstrapReady();
  const connectionStatus = useConnectionStatus();
  const serverError = useServerError();
  const desktop = useDesktopBridge();
  const [deskToken, setDeskToken] = useState("");
  const [serverUrl, setServerUrl] = useState(configuredServerUrl());
  const [startupGrace, setStartupGrace] = useState(true);
  const [hasConnected, setHasConnected] = useState(false);
  const isTauri = desktop.available;
  const usesBuiltInServer = useMemo(() => {
    try {
      const host = new URL(serverUrl).hostname;
      return host === "127.0.0.1" || host === "localhost" || host === "[::1]" || host === "::1";
    } catch {
      return false;
    }
  }, [serverUrl]);
  useEffect(() => {
    const timer = window.setTimeout(() => setStartupGrace(false), 10_000);
    return () => window.clearTimeout(timer);
  }, []);
  useEffect(() => {
    if (connectionStatus === "connected") setHasConnected(true);
  }, [connectionStatus]);
  if (connectionStatus === "connected") return null;
  if (hasConnected && bootstrapReady)
    return (
      <div className={`connection-banner ${connectionStatus}`} role="status">
        <span className="status-pulse" />
        <b>
          {connectionStatus === "connecting"
            ? "Reconnecting to server…"
            : "Server unavailable"}
        </b>
        <small>
          {serverError ??
            "Playback state remains visible while the connection recovers."}
        </small>
      </div>
    );
  const boundaryRequired =
    serverError?.toLowerCase().includes("desk boundary token") ?? false;
  const startingBuiltIn = isTauri && usesBuiltInServer && startupGrace && !boundaryRequired;
  if (!boundaryRequired && (startingBuiltIn || !serverError))
    return (
      <LoadingSurface
        className="connection-cover"
        showMark
        title={startingBuiltIn ? "Starting ToskLight" : "Connecting to ToskLight"}
        detail={startingBuiltIn ? "Starting built-in server…" : "Starting a secure operator session…"}
        note={startingBuiltIn ? "Preparing the show engine and control surface" : "Waiting for bootstrap, operator session, and desk stores"}
      />
    );
  return (
    <div className="connection-cover" role="status">
      <div className="connection-card">
        <h1>
          {boundaryRequired
            ? "Connect to this desk"
            : "Connecting to ToskLight"}
        </h1>
        <p>{boundaryRequired ? serverError : usesBuiltInServer ? serverError ?? "Built-in server is unavailable." : serverError ?? "Starting a secure operator session…"}</p>
        {boundaryRequired ? (
          <form
            className="connection-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (deskToken.trim()) connection?.setDeskToken(deskToken);
            }}
          >
            <TextField
              label="Desk boundary token"
              autoFocus
              secure
              clearable
              aria-label="Desk boundary token"
              value={deskToken}
              onChange={(event) => setDeskToken(event.target.value)}
              placeholder="Desk token"
            />
            <div className="connection-form-actions"><Button disabled={!deskToken.trim()}>Connect</Button></div>
          </form>
        ) : <small>Retrying automatically</small>}
        {isTauri && (
          <form
            className="connection-form"
            onSubmit={(event) => {
              event.preventDefault();
              connection?.setServerUrl(serverUrl);
            }}
          >
            <TextField
              label="Server"
              clearable
              aria-label="Light server URL"
              value={serverUrl}
              onChange={(event) => setServerUrl(event.target.value)}
              placeholder="http://desk.local:5000"
            />
            <div className="connection-form-actions"><Button>Use server</Button></div>
          </form>
        )}
      </div>
    </div>
  );
}
