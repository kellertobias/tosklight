import { useServer } from "../../api/ServerContext";
import {
  useDeskConfiguration,
  useMatterEnabled,
} from "../../features/configuration/ConfigurationState";
import { Button, SwitchField } from "../common";

export function MatterBridgeSettings() {
  const server = useServer();
  const matter = server.matter;
  const configuration = useDeskConfiguration();
  const enabled = useMatterEnabled();
  const toggleMatter = (enabled: boolean) => {
    if (!configuration) return;
    void server.saveConfiguration({ ...configuration, matter_enabled: enabled });
  };

  return <article className="matter-desk-settings" aria-label="Matter playback bridge">
    <header><div><b>Matter playback bridge</b><small>Desk installation · shared across shows and Desktops</small></div></header>
    <SwitchField label={enabled ? "Matter server enabled" : "Matter server disabled"} checked={enabled} onChange={(event) => toggleMatter(event.target.checked)}/>
    <p>{!enabled ? "Disabled. No Matter lights are advertised." : matter?.transport === "running" ? `${matter.lights.length} assigned playback${matter.lights.length === 1 ? "" : "s"} exposed as dimmable lights.` : matter?.limitation ?? "Starting Matter networking…"}</p>
    {matter?.commissionable && matter.pairing && <div className="matter-pairing"><b>Ready to commission</b><span>Manual pairing code</span><code>{matter.pairing.manual_code}</code><Button onClick={() => void navigator.clipboard?.writeText(matter.pairing?.manual_code ?? "")}>Copy pairing code</Button><details><summary>QR payload</summary><code>{matter.pairing.qr_code}</code></details></div>}
    {matter?.commissioned && <small>Commissioned on the local Matter fabric. Playback changes and controller writes are synchronized in both directions.</small>}
    {enabled && <small>Every assigned global page/playback is exposed, including button-only controls; empty slots are not advertised.</small>}
  </article>;
}
