import { useState } from "react";
import { presets } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";

type StoreMode = "Merge" | "Overwrite" | "AddMissingFixtures";

export function PresetsWindow({ compact }: WindowProps) {
  const server = useServer();
  const [storeArmed, setStoreArmed] = useState(false);
  const [storeMode, setStoreMode] = useState<StoreMode>("Merge");
  const [family, setFamily] = useState("All");
  const fallback = server.bootstrap ? [] : presets.filter((preset) => preset.name).map((preset) => ({ id: String(preset.id), body: { name: preset.name!, values: {}, family: preset.family, color: preset.color, icon: preset.icon } }));
  const stored = server.bootstrap?.active_show ? server.presets : fallback;
  const cards = Array.from({ length: 40 }, (_, index) => stored.find((preset) => preset.id === String(index + 1)) ?? null);

  const activate = (index: number) => {
    const preset = cards[index];
    if (storeArmed || !preset) {
      void server.storePreset(String(index + 1), preset?.body.name ?? `Preset ${index + 1}`, preset ? storeMode : "Overwrite");
      setStoreArmed(false);
    } else {
      void server.applyPreset(preset.id);
    }
  };

  const families = ["All", "Intensity", "Color", "Position", "Beam"];
  return <div className="pool-window"><header className="window-toolbar"><h1>{compact ? `Presets · ${family}` : "Preset Pools"}</h1><span className="spacer"/>{compact ? <button aria-label="Next preset family" onClick={() => setFamily(families[(families.indexOf(family) + 1) % families.length])}>⚙</button> : <>{families.map((name) => <button key={name} onClick={() => setFamily(name)} className={family === name ? "active" : ""}>{name}</button>)}<select aria-label="Preset store mode" value={storeMode} onChange={(event) => setStoreMode(event.target.value as StoreMode)}><option value="Merge">Merge</option><option value="Overwrite">Overwrite</option><option value="AddMissingFixtures">Add missing</option></select><button disabled={!server.selectedFixtures.length} className={storeArmed ? "active" : ""} onClick={() => setStoreArmed(!storeArmed)}>{storeArmed ? "Choose preset…" : "Store"}</button></>}</header><div className="card-pool">{cards.map((preset, index) => { if (preset && family !== "All" && preset.body.family !== family) return null; return <button disabled={!preset && !server.selectedFixtures.length} key={index + 1} className={`preset-card ${!preset ? "empty" : ""} ${storeArmed ? "store-target" : ""}`} onClick={() => activate(index)}><span className="number">{index + 1}</span>{preset ? <><span className="preset-art" style={{ background: `${preset.body.color ?? "#2cb7d6"}44` }}>{preset.body.icon ?? "◇"}</span><b>{preset.body.name}</b><small>{preset.body.family ?? "All"} · {Object.keys(preset.body.values).length} fixtures</small></> : <><b>Empty</b><small>{server.selectedFixtures.length ? (storeArmed ? `Store · ${storeMode}` : "Tap to store programmer") : "Select fixtures to store"}</small></>}</button>; })}</div></div>;
}
