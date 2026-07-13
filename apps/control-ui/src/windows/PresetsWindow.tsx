import { presets } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import { useApp } from "../state/AppContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { GroupsPoolButton } from "../components/shared/GroupsPoolButton";
import { Button } from "../components/common";

export function PresetsWindow({ compact, showGroupShortcuts }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const family = state.presetFamily;
  const groupsVisible = compact ? Boolean(showGroupShortcuts) : state.presetGroupsVisible;
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
    { length: Math.max(200, ...stored.map((preset) => Number(preset.id) || 0)) },
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
          <div className="preset-toolbar-groups">
            <div className="preset-family-controls">
            {families.map((name) => (
              <Button
                key={name}
                onClick={() => dispatch({ type: "SET_PRESET_FAMILY", family: name as typeof state.presetFamily })}
                className={`preset-family-button family-${name.toLowerCase()} ${family === name ? "active" : ""}`}
              >
                {name}
              </Button>
            ))}
            </div>
            <i className="preset-groups-divider" aria-hidden="true" />
            <GroupsPoolButton shortcutsVisible={groupsVisible} onToggleShortcuts={() => dispatch({type:"SET_BUILTIN_GROUPS_VISIBLE",window:"presets",value:!groupsVisible})} />
          </div>
      </header>}
      <div className="card-pool">
        {cards.map((preset, index) => {
          const filtered = Boolean(preset && family !== "All" && preset.body.family !== family);
          return (
            <Button
              disabled={filtered}
              key={index + 1}
              className={`preset-card pool-cell ${preset ? `preset-family-${String(preset.body.family ?? "all").toLowerCase()}` : ""} ${!preset ? "empty" : ""} ${filtered ? "filtered" : ""} ${state.storeArmed ? "store-target" : ""}`}
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
            </Button>
          );
        })}
      </div>
      {groupsVisible && <GroupStrip />}
    </div>
  );
}
