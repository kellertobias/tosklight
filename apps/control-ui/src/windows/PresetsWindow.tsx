import { presets } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import { useApp } from "../state/AppContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { GroupsPoolButton } from "../components/shared/GroupsPoolButton";
import { Button, ColorPickerField, FormLayout, IconPickerField, SwitchField, TextField } from "../components/common";
import { ButtonGrid, WindowHeader, WindowScrollArea, WindowSettings } from "../components/window-kit";
import { useState, type CSSProperties } from "react";

type PresetCustomization = { title?: string; icon?: string; color?: string };
export function PresetsWindow({ compact, paneId, showGroupShortcuts, presetFamily, presetPoolColors }: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const family = compact ? (presetFamily ?? state.presetFamily) : state.presetFamily;
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
  const colorsEnabled = compact ? (presetPoolColors ?? true) : state.presetPoolColors;
  const [customizations, setCustomizations] = useState<Record<string, PresetCustomization>>(() => { try { return JSON.parse(localStorage.getItem("light.preset-button-customizations") ?? "{}"); } catch { return {}; } });
  const [configureIndex, setConfigureIndex] = useState<number | null>(null);
  const [configureDraft, setConfigureDraft] = useState<PresetCustomization>({});
  const setFamily = (next: typeof state.presetFamily) => dispatch(compact && paneId ? { type: "SET_PANE_PRESET_FAMILY", id: paneId, family: next } : { type: "SET_PRESET_FAMILY", family: next });
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
    if (state.presetSetArmed) { const saved = customizations[String(index + 1)] ?? {}; setConfigureDraft({ title: saved.title ?? preset?.body.name ?? `Preset ${index + 1}`, icon: saved.icon ?? preset?.body.icon ?? "◇", color: saved.color ?? preset?.body.color ?? "#d98236" }); setConfigureIndex(index); dispatch({ type: "SET_PRESET_SET_ARMED", value: false }); return; }
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
    <div className={`pool-window preset-pool-window ${colorsEnabled ? "pool-colors" : "pool-colors-disabled"} pool-family-${family.toLowerCase()}`}>
      {!compact && <WindowHeader title="Preset Pools" info={{ primary: family === "All" ? "All preset families" : `${family} presets` }} actions={[[...families.map((name) => ({ id: name, label: name, active: family === name, onClick: () => setFamily(name as typeof state.presetFamily) }))],[{ id: "groups", label: "Groups", onClick: () => dispatch({ type: "OPEN_BUILTIN", kind: "groups" }) }]]} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
      <WindowScrollArea><ButtonGrid className="card-pool">
        {cards.map((preset, index) => {
          const filtered = Boolean(preset && family !== "All" && preset.body.family !== family);
          return (
            <Button
              disabled={filtered}
              key={index + 1}
              className={`preset-card pool-cell preset-family-${String(preset?.body.family ?? family).toLowerCase()} ${!preset ? "empty" : ""} ${filtered ? "filtered" : ""} ${state.storeArmed ? "store-target" : ""} ${state.presetSetArmed ? "set-target" : ""}`}
              style={colorsEnabled && customizations[String(index + 1)]?.color ? { "--preset-family": customizations[String(index + 1)].color } as CSSProperties : undefined}
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
                    {customizations[String(index + 1)]?.icon ?? preset.body.icon ?? "◇"}
                  </span>
                  <b>{customizations[String(index + 1)]?.title ?? preset.body.name}</b>
                  <small>
                    {preset.body.family ?? "All"} ·{" "}
                    {Object.keys(preset.body.values).length} fixtures
                  </small>
                </>
              ) : filtered ? <small>Other family</small> : (
                <>
                  {customizations[String(index + 1)]?.icon && <span className="preset-art">{customizations[String(index + 1)].icon}</span>}
                  <b>{customizations[String(index + 1)]?.title ?? "Empty"}</b>
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
      </ButtonGrid></WindowScrollArea>
      {groupsVisible && <GroupStrip />}
      {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title="Preset Settings" onClose={() => setSettingsAnchor(null)} tabs={[{ id: "pool", label: "Pool", content: <><h3>Preset family</h3><div className="button-group">{families.map((name) => <Button key={name} className={family === name ? "active" : ""} onClick={() => setFamily(name as typeof state.presetFamily)}>{name}</Button>)}</div><SwitchField label="Enable pool colors" checked={colorsEnabled} onChange={(event) => dispatch(compact && paneId ? { type: "SET_PANE_PRESET_COLORS", id: paneId, value: event.target.checked } : { type: "SET_PRESET_POOL_COLORS", value: event.target.checked })}/></> }]} />}
      {configureIndex != null && <div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setConfigureIndex(null)}><section className="nested-modal preset-button-settings" role="dialog" aria-modal="true" aria-label="Configure preset button"><Button className="modal-close" onClick={() => setConfigureIndex(null)}>×</Button><h3>Configure preset {configureIndex + 1}</h3><FormLayout labelPlacement="side"><TextField label="Title" clearable value={configureDraft.title ?? ""} onChange={(event) => setConfigureDraft({ ...configureDraft, title: event.target.value })}/><IconPickerField label="Icon" value={configureDraft.icon ?? "◇"} onChange={(icon) => setConfigureDraft({ ...configureDraft, icon })}/><ColorPickerField label="Button color" value={configureDraft.color ?? "#d98236"} onChange={(color) => setConfigureDraft({ ...configureDraft, color })}/></FormLayout><footer><Button onClick={() => setConfigureIndex(null)}>Cancel</Button><Button className="primary" onClick={() => { const next = { ...customizations, [String(configureIndex + 1)]: configureDraft }; setCustomizations(next); localStorage.setItem("light.preset-button-customizations", JSON.stringify(next)); setConfigureIndex(null); }}>Save button</Button></footer></section></div>}
    </div>
  );
}
