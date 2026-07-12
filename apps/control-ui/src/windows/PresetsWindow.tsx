import { useState } from "react";
import { presets } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import { useApp } from "../state/AppContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { GroupsPoolButton } from "../components/shared/GroupsPoolButton";

export function PresetsWindow({ compact, showGroupShortcuts }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const family = state.presetFamily;
  const [groupsVisible, setGroupsVisible] = useState(!compact);
  const fallback = server.bootstrap
    ? []
    : presets
        .filter((preset) => preset.name)
        .map((preset) => ({
          id: String(preset.id),
          body: {
            name: preset.name!,
            values: {},
            family: preset.family,
            color: preset.color,
            icon: preset.icon,
          },
        }));
  const stored = server.bootstrap?.active_show ? server.presets : fallback;
  const cards = Array.from(
    { length: 40 },
    (_, index) =>
      stored.find((preset) => preset.id === String(index + 1)) ?? null,
  );

  const activate = (index: number) => {
    const preset = cards[index];
    if (!preset && !state.storeArmed) return;
    if (state.storeArmed) {
      const armedMode = preset ? (window.confirm("Merge current values into this preset? Choose Cancel to overwrite it instead.") ? "merge" : "overwrite") : "overwrite";
      if (state.preload !== "idle") void server.storePreload({ target: "preset", target_id: String(index + 1), name: preset?.body.name ?? `Preset ${index + 1}`, mode: armedMode }, preset && "revision" in preset ? preset.revision : 0);
      else void server.storePreset(String(index + 1), preset?.body.name ?? `Preset ${index + 1}`, armedMode, preset?.body.family ?? family);
      dispatch({ type: "SET_STORE_ARMED", value: false });
    } else if (preset) {
      void server.applyPreset(preset.id);
    }
  };

  const families = ["All", "Intensity", "Color", "Position", "Beam"];
  return (
    <div className="pool-window">
      {!compact && <header className="window-toolbar">
        <h1>Preset Pools</h1>
        <span className="spacer" />
          <>
            {families.map((name) => (
              <button
                key={name}
                onClick={() => dispatch({ type: "SET_PRESET_FAMILY", family: name as typeof state.presetFamily })}
                className={family === name ? "active" : ""}
              >
                {name}
              </button>
            ))}
            <GroupsPoolButton shortcutsVisible={groupsVisible} onToggleShortcuts={() => setGroupsVisible(!groupsVisible)} />
          </>
      </header>}
      <div className="card-pool">
        {cards.map((preset, index) => {
          const filtered = Boolean(preset && family !== "All" && preset.body.family !== family);
          return (
            <button
              disabled={filtered}
              key={index + 1}
              className={`preset-card pool-cell ${!preset ? "empty" : ""} ${filtered ? "filtered" : ""} ${state.storeArmed ? "store-target" : ""}`}
              onClick={() => activate(index)}
            >
              <span className="number">{index + 1}</span>
              {preset && !filtered ? (
                <>
                  <span
                    className="preset-art"
                    style={{
                      background: `${preset.body.color ?? "#2cb7d6"}44`,
                    }}
                  >
                    {preset.body.icon ?? "◇"}
                  </span>
                  <b>{preset.body.name}</b>
                  <small>
                    {preset.body.family ?? "All"} ·{" "}
                    {Object.keys(preset.body.values).length} fixtures
                  </small>
                </>
              ) : filtered ? <small>Other family</small> : (
                <>
                  <b>Empty</b>
                  <small>
                    {server.selectedFixtures.length
                      ? state.storeArmed
                        ? "Record here"
                        : "Tap to record programmer"
                      : "Select fixtures to record"}
                  </small>
                </>
              )}
            </button>
          );
        })}
      </div>
      {(compact ? showGroupShortcuts : groupsVisible) && <GroupStrip />}
    </div>
  );
}
