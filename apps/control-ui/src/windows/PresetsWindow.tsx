import { presets } from "../data/mockData";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import { useApp } from "../state/AppContext";
import { GroupStrip } from "../components/shared/GroupStrip";
import { GroupsPoolButton } from "../components/shared/GroupsPoolButton";
import { Button, ColorPickerField, FormLayout, IconPickerField, ModalPortal, SwitchField, TextField } from "../components/common";
import { ButtonGrid, WindowHeader, WindowScrollArea, WindowSettings } from "../components/window-kit";
import { useState, type CSSProperties } from "react";
import { RecordModeDialog, type RecordMode } from "../components/shared/RecordModeDialog";
import { requestUpdateTarget } from "../components/control/updateWorkflow";
import { normalizePresetFamily, presetAddress, presetStorageKey, PRESET_FAMILIES } from "../presetFamilies";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";
import { usePresets } from "../features/showObjects/ShowObjectsState";
import { useProgrammingSelectionView } from "../features/programmingInteraction/ProgrammingInteractionView";
import { usePresetRecording } from "../features/presetRecording/PresetRecordingProvider";
import { resolvePresetCards } from "../features/presetRecording/presetCards";
import { submitPresetRecording } from "../features/presetRecording/submitRecording";

type PresetCustomization = { title?: string; icon?: string; color?: string };

export function PresetsWindow({ active = true, compact, paneId, showGroupShortcuts, presetFamily, presetPoolColors }: WindowProps) {
  useShowObjectView("preset", active);
  const server = useServer();
  const selection = useProgrammingSelectionView(active);
  const storedPresets = usePresets();
  const presetRecording = usePresetRecording();
  const { state, dispatch } = useApp();
  const family = compact ? (presetFamily ?? state.presetFamily) : state.presetFamily;
  const [settingsAnchor, setSettingsAnchor] = useState<DOMRect | null>(null);
  const colorsEnabled = compact ? (presetPoolColors ?? true) : state.presetPoolColors;
  const [customizations, setCustomizations] = useState<Record<string, PresetCustomization>>(() => { try { return JSON.parse(localStorage.getItem("light.preset-button-customizations") ?? "{}"); } catch { return {}; } });
  const [configureIndex, setConfigureIndex] = useState<number | null>(null);
  const [configureDraft, setConfigureDraft] = useState<PresetCustomization>({});
  const [recordPresetIndex, setRecordPresetIndex] = useState<number | null>(null);
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
            number: preset.id,
            values: {},
            family: normalizePresetFamily(preset.family),
            color: preset.color,
            icon: preset.icon,
          },
        }));
  const stored = server.bootstrap?.active_show ? storedPresets : fallback;
  const cards = resolvePresetCards(stored, family);

  const cancelRecording = () => {
    setRecordPresetIndex(null);
    dispatch({ type: "SET_STORE_ARMED", value: false });
  };
  const recordPreset = (index: number, mode: RecordMode) => {
    setRecordPresetIndex(null);
    dispatch({ type: "SET_STORE_ARMED", value: false });
    submitPresetRecording({
      card: cards[index],
      index,
      family,
      mode,
      preloadActive: state.preload !== "idle",
      actions: presetRecording,
      storePreload: server.storePreload,
    });
  };

  const activate = (index: number) => {
    const preset = cards[index];
    if (state.updateArmed) {
      requestUpdateTarget({ family: { type: "preset" }, object_id: preset?.id ?? presetStorageKey(presetAddress(family, index + 1)) });
      return;
    }
    if (state.presetSetArmed) { const id = preset?.id ?? presetStorageKey(presetAddress(family, index + 1)); const saved = customizations[id] ?? {}; setConfigureDraft({ title: saved.title ?? preset?.body.name ?? `Preset ${index + 1}`, icon: saved.icon ?? preset?.body.icon ?? "◇", color: saved.color ?? preset?.body.color ?? "#d98236" }); setConfigureIndex(index); dispatch({ type: "SET_PRESET_SET_ARMED", value: false }); return; }
    if (!preset && !state.storeArmed) return;
    if (state.storeArmed) {
      if (preset) setRecordPresetIndex(index);
      else recordPreset(index, "overwrite");
    } else if (preset) {
      void server.applyPreset(presetAddress(normalizePresetFamily(preset.body.family), preset.body.number));
    }
  };

  return (
    <div className={`pool-window preset-pool-window ${colorsEnabled ? "pool-colors" : "pool-colors-disabled"} pool-family-${family.toLowerCase()}`}>
      {!compact && <WindowHeader title="Preset Pools" info={{ primary: `${family} presets` }} actions={[[...PRESET_FAMILIES.map((name) => ({ id: name, label: name, active: family === name, onClick: () => setFamily(name) }))],[{ id: "groups", label: "Groups", onClick: () => dispatch({ type: "OPEN_BUILTIN", kind: "groups" }) }]]} settings onSettings={(anchor) => setSettingsAnchor(anchor.getBoundingClientRect())} />}
      <WindowScrollArea><ButtonGrid className="card-pool">
        {cards.map((preset, index) => {
          const storedFamily = normalizePresetFamily(preset?.body.family);
          const filtered = Boolean(preset && storedFamily !== family);
          const customizationId = preset?.id ?? presetStorageKey(presetAddress(family, index + 1));
          const customization = customizations[customizationId];
          return (
            <Button
              disabled={filtered}
              key={index + 1}
              className={`preset-card pool-cell preset-family-${preset ? storedFamily.toLowerCase() : family.toLowerCase()} ${!preset ? "empty" : ""} ${filtered ? "filtered" : ""} ${state.storeArmed ? "store-target" : ""} ${state.updateArmed ? "update-target" : ""} ${state.presetSetArmed ? "set-target" : ""}`}
              style={colorsEnabled && customization?.color ? { "--preset-family": customization.color } as CSSProperties : undefined}
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
                    {customization?.icon ?? preset.body.icon ?? "◇"}
                  </span>
                  <b>{customization?.title ?? preset.body.name}</b>
                  <small>
                    {storedFamily} ·{" "}
                    {Object.keys(preset.body.values).length} fixtures
                  </small>
                </>
              ) : filtered ? <small>Other family</small> : (
                <>
                  {customization?.icon && <span className="preset-art">{customization.icon}</span>}
                  <b>{customization?.title ?? "Empty"}</b>
                  <small>
                    {state.updateArmed
                      ? "Touch to check Update eligibility"
                      : selection?.selected.length
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
      {groupsVisible && <GroupStrip active={active} />}
      {settingsAnchor && <WindowSettings modal={false} anchor={settingsAnchor} title="Preset Settings" onClose={() => setSettingsAnchor(null)} tabs={[{ id: "pool", label: "Pool", content: <><h3>Preset family</h3><div className="button-group">{PRESET_FAMILIES.map((name) => <Button key={name} className={family === name ? "active" : ""} onClick={() => setFamily(name)}>{name}</Button>)}</div><SwitchField label="Enable pool colors" checked={colorsEnabled} onChange={(event) => dispatch(compact && paneId ? { type: "SET_PANE_PRESET_COLORS", id: paneId, value: event.target.checked } : { type: "SET_PRESET_POOL_COLORS", value: event.target.checked })}/></> }]} />}
      {recordPresetIndex != null && cards[recordPresetIndex] && <RecordModeDialog target={cards[recordPresetIndex].body.name ?? `Preset ${recordPresetIndex + 1}`} onChoose={(mode) => recordPreset(recordPresetIndex, mode)} onCancel={cancelRecording}/>}
      {configureIndex != null && <ModalPortal><div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && setConfigureIndex(null)}><section className="nested-modal preset-button-settings" role="dialog" aria-modal="true" aria-label="Configure preset button"><Button className="modal-close" onClick={() => setConfigureIndex(null)}>×</Button><h3>Configure preset {configureIndex + 1}</h3><FormLayout labelPlacement="side"><TextField label="Title" clearable value={configureDraft.title ?? ""} onChange={(event) => setConfigureDraft({ ...configureDraft, title: event.target.value })}/><IconPickerField label="Icon" value={configureDraft.icon ?? "◇"} onChange={(icon) => setConfigureDraft({ ...configureDraft, icon })}/><ColorPickerField label="Button color" value={configureDraft.color ?? "#d98236"} onChange={(color) => setConfigureDraft({ ...configureDraft, color })}/></FormLayout><footer><Button onClick={() => setConfigureIndex(null)}>Cancel</Button><Button className="primary" onClick={() => { const id = cards[configureIndex]?.id ?? presetStorageKey(presetAddress(family, configureIndex + 1)); const next = { ...customizations, [id]: configureDraft }; setCustomizations(next); localStorage.setItem("light.preset-button-customizations", JSON.stringify(next)); setConfigureIndex(null); }}>Save button</Button></footer></section></div></ModalPortal>}
    </div>
  );
}
