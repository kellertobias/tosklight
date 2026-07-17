import { useServer } from "../../api/ServerContext";
import { Button, CheckboxField } from "../common";

export function MatterBridgeSettings() {
  const server = useServer();
  const matter = server.matter;
  const toggleMatter = (enabled: boolean) => {
    if (!server.configuration) return;
    void server.saveConfiguration({ ...server.configuration, matter_enabled: enabled });
  };

  return <article className="matter-desk-settings" aria-label="Matter playback bridge">
    <header><div><b>Matter playback bridge</b><small>Desk installation · shared across shows and Desktops</small></div></header>
    <CheckboxField label="Enable this desk as a Matter bridge" checked={server.configuration?.matter_enabled ?? false} onChange={(event) => toggleMatter(event.target.checked)}/>
    <p>{!server.configuration?.matter_enabled ? "Disabled. No Matter lights are advertised." : matter?.transport === "running" ? `${matter.lights.length} assigned playback${matter.lights.length === 1 ? "" : "s"} exposed as dimmable lights.` : matter?.limitation ?? "Starting Matter networking…"}</p>
    {matter?.commissionable && matter.pairing && <div className="matter-pairing"><b>Ready to commission</b><span>Manual pairing code</span><code>{matter.pairing.manual_code}</code><Button onClick={() => void navigator.clipboard?.writeText(matter.pairing?.manual_code ?? "")}>Copy pairing code</Button><details><summary>QR payload</summary><code>{matter.pairing.qr_code}</code></details></div>}
    {matter?.commissioned && <small>Commissioned on the local Matter fabric. Playback changes and controller writes are synchronized in both directions.</small>}
    {server.configuration?.matter_enabled && <small>Every assigned global page/playback is exposed, including button-only controls; empty slots are not advertised.</small>}
  </article>;
}
