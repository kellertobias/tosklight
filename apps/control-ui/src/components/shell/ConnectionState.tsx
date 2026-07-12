import { useEffect, useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { configuredServerUrl } from "../../api/LightApiClient";
import appIcon from "../../../src-tauri/icons/icon.svg";

export function ConnectionState() {
  const server = useServer();
  const [deskToken, setDeskToken] = useState("");
  const [serverUrl, setServerUrl] = useState(configuredServerUrl());
  const [startupGrace, setStartupGrace] = useState(true);
  const isTauri = "__TAURI_INTERNALS__" in window;
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
  if (server.status === "connected") return null;
  if (server.bootstrap)
    return (
      <div className={`connection-banner ${server.status}`} role="status">
        <span className="status-pulse" />
        <b>
          {server.status === "connecting"
            ? "Reconnecting to server…"
            : "Server unavailable"}
        </b>
        <small>
          {server.error ??
            "Playback state remains visible while the connection recovers."}
        </small>
      </div>
    );
  const boundaryRequired =
    server.error?.toLowerCase().includes("desk boundary token") ?? false;
  const startingBuiltIn = isTauri && usesBuiltInServer && startupGrace && !boundaryRequired;
  return (
    <div className="connection-cover" role="status">
      <div className="connection-card">
        <div className="app-mark" aria-label="ToskLight application">
          <img src={appIcon} alt="" />
        </div>
        <span className="status-pulse" />
        <h1>
          {startingBuiltIn
            ? "Starting ToskLight"
            : boundaryRequired
            ? "Connect to this desk"
            : "Connecting to ToskLight"}
        </h1>
        <p>{startingBuiltIn ? "Starting built-in server…" : boundaryRequired ? server.error : usesBuiltInServer ? server.error ?? "Built-in server is unavailable." : server.error ?? "Starting a secure operator session…"}</p>
        {startingBuiltIn ? <small>Preparing the show engine and control surface</small> : boundaryRequired ? (
          <form
            className="desk-token-form"
            onSubmit={(event) => {
              event.preventDefault();
              if (deskToken.trim()) server.setDeskToken(deskToken);
            }}
          >
            <input
              autoFocus
              type="password"
              aria-label="Desk boundary token"
              value={deskToken}
              onChange={(event) => setDeskToken(event.target.value)}
              placeholder="Desk token"
            />
            <button disabled={!deskToken.trim()}>Connect</button>
          </form>
        ) : (
          <small>Retrying automatically</small>
        )}
        {isTauri && !startingBuiltIn && (
          <form
            className="desk-token-form"
            onSubmit={(event) => {
              event.preventDefault();
              server.setServerUrl(serverUrl);
            }}
          >
            <input
              aria-label="Light server URL"
              value={serverUrl}
              onChange={(event) => setServerUrl(event.target.value)}
              placeholder="http://desk.local:5000"
            />
            <button>Use server</button>
          </form>
        )}
      </div>
    </div>
  );
}
