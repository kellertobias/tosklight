import { useState } from "react";
import { useServer } from "../../api/ServerContext";

export function ConnectionState() {
  const server = useServer();
  const [deskToken, setDeskToken] = useState("");
  if (server.status === "connected") return null;
  if (server.bootstrap) return <div className={`connection-banner ${server.status}`} role="status"><span className="status-pulse"/><b>{server.status === "connecting" ? "Reconnecting to server…" : "Server unavailable"}</b><small>{server.error ?? "Playback state remains visible while the connection recovers."}</small></div>;
  const boundaryRequired = server.error?.toLowerCase().includes("desk boundary token") ?? false;
  return <div className="connection-cover" role="status"><div className="connection-card"><div className="app-mark">L</div><span className="status-pulse"/><h1>{boundaryRequired ? "Connect to this desk" : "Connecting to Light"}</h1><p>{server.error ?? "Starting a secure operator session…"}</p>{boundaryRequired ? <form className="desk-token-form" onSubmit={(event) => { event.preventDefault(); if (deskToken.trim()) server.setDeskToken(deskToken); }}><input autoFocus type="password" aria-label="Desk boundary token" value={deskToken} onChange={(event) => setDeskToken(event.target.value)} placeholder="Desk token"/><button disabled={!deskToken.trim()}>Connect</button></form> : <small>Retrying automatically</small>}</div></div>;
}
