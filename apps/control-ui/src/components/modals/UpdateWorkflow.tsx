import { useEffect, useMemo, useState } from "react";
import type {
  CueUpdateMode,
  ExistingContentMode,
  UpdateMenuEntry,
  UpdateMode,
  UpdatePreview,
  UpdatePreviewItem,
  UpdateResult,
  UpdateSettings,
  UpdateTargetFilter,
  UpdateTargetIdentity,
  UpdateTargetRequest,
} from "../../api/types";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { Button, FormLayout, SelectField, SwitchField } from "../common";
import {
  UPDATE_SETTINGS_EVENT,
  UPDATE_ARMED_EVENT,
  UPDATE_TARGET_EVENT,
  UPDATE_TARGET_MENU_EVENT,
  configuredUpdateMode,
  cueUpdateModes,
  defaultUpdateSettings,
  existingContentModes,
  modeLabel,
  targetFamilyLabel,
  updateTargetKey,
} from "../control/updateWorkflow";

type Operation = { request: UpdateTargetRequest; preview: UpdatePreview };

const changingOutcomes = new Set([
  "change_at_source",
  "change_in_current_cue",
  "add_to_current_cue",
  "add_new_to_current_cue",
  "update_existing",
  "add_new",
]);

export function updatePreviewStats(preview: UpdatePreview) {
  const ignored = preview.items.filter((item) => item.outcome.outcome === "ignored").length;
  const changed = preview.items.filter((item) => changingOutcomes.has(item.outcome.outcome)).length;
  const added = preview.items.filter((item) => ["add_to_current_cue", "add_new_to_current_cue", "add_new"].includes(item.outcome.outcome)).length;
  const source = preview.items.filter((item) => item.outcome.outcome === "change_at_source").length;
  const currentCue = preview.items.filter((item) => ["change_in_current_cue", "add_to_current_cue", "add_new_to_current_cue"].includes(item.outcome.outcome)).length;
  return { eligible: preview.items.length - ignored, changed, added, ignored, source, currentCue };
}

function requestFromIdentity(target: UpdateTargetIdentity): UpdateTargetRequest {
  return {
    family: target.family,
    object_id: target.object_id,
    ...(target.playback_number == null ? {} : { playback_number: target.playback_number }),
    ...(target.cue ? { cue_id: target.cue.id, cue_number: target.cue.number } : {}),
    ...(target.playback_number == null ? {} : { validate_active_context: true }),
  };
}

function addressLabel(item: UpdatePreviewItem) {
  if (item.address.type === "fixture_attribute") return `Fixture ${item.address.fixture_id} · ${item.address.attribute}`;
  if (item.address.type === "group_attribute") return `Group ${item.address.group_id} · ${item.address.attribute}`;
  return `Fixture ${item.address.fixture_id} · Group membership`;
}

function outcomeLabel(item: UpdatePreviewItem) {
  const outcome = item.outcome;
  if (outcome.outcome === "change_at_source") return `Change at source Cue ${outcome.source.cue_number}`;
  if (outcome.outcome === "change_in_current_cue") return `Change in current Cue ${outcome.cue.cue_number}`;
  if (outcome.outcome === "add_to_current_cue") return `Add to current Cue ${outcome.cue.cue_number}`;
  if (outcome.outcome === "add_new_to_current_cue") return `Add new to current Cue ${outcome.cue.cue_number}`;
  if (outcome.outcome === "update_existing") return "Update existing stored content";
  if (outcome.outcome === "add_new") return "Add new stored content";
  if (outcome.outcome === "unchanged") return outcome.source ? `Unchanged at Cue ${outcome.source.cue_number}` : "Unchanged";
  return ({
    new_address: "Ignored · address is new to this target",
    not_in_current_cue: "Ignored · not explicitly stored in the current Cue",
    not_in_active_tracked_state: "Ignored · not in the active tracked state",
    new_group_member: "Ignored · fixture is not an existing Group member",
  } as const)[outcome.reason];
}

function targetContext(target: UpdateTargetIdentity) {
  const parts = [targetFamilyLabel(target)];
  if (target.playback_number != null) parts.push(`Playback ${target.playback_number}`);
  if (target.cue) parts.push(`Current Cue ${target.cue.number}`);
  return parts.join(" · ");
}

export function UpdateOperationDialog({ operation, busy, error, onMode, onApply, onCancel }: {
  operation: Operation;
  busy: boolean;
  error: string | null;
  onMode: (mode: UpdateMode) => void;
  onApply: () => void;
  onCancel: () => void;
}) {
  const { preview } = operation;
  const stats = updatePreviewStats(preview);
  const cueModes = preview.target.family.type === "cue";
  return <div className="modal-backdrop update-workflow-layer" onPointerDown={(event) => event.target === event.currentTarget && onCancel()}>
    <section className="modal-card update-operation-modal" role="dialog" aria-modal="true" aria-label={`Update ${preview.target.name}`}>
      <Button className="modal-close" aria-label="Cancel Update" onClick={onCancel}>×</Button>
      <header className="update-modal-header"><span>UPDATE</span><div><h2>{preview.target.name}</h2><p>{targetContext(preview.target)}</p></div></header>
      <p>Choose how the current programmer changes apply to this existing target. Nothing changes until Update is confirmed.</p>
      <div className="update-mode-grid" aria-label="Update mode">
        {(cueModes ? cueUpdateModes : existingContentModes).map((candidate) => {
          const mode = cueModes
            ? { target_type: "cue" as const, mode: candidate.value as CueUpdateMode }
            : { target_type: "existing_content" as const, mode: candidate.value as ExistingContentMode };
          return <Button className={preview.mode.target_type === mode.target_type && preview.mode.mode === mode.mode ? "active" : ""} disabled={busy} onClick={() => onMode(mode)} key={candidate.value}>{candidate.label}</Button>;
        })}
      </div>
      <div className="update-preview-summary" aria-label="Update preview summary">
        <strong>{modeLabel(preview.mode)}</strong>
        <span>Eligible {stats.eligible}</span><span>Changed {stats.changed}</span><span>Ignored {stats.ignored}</span>
        {stats.source > 0 && <span>At source {stats.source}</span>}
        {stats.currentCue > 0 && <span>In current Cue {stats.currentCue}</span>}
        {stats.added > 0 && <span>Added {stats.added}</span>}
      </div>
      <div className="update-preview-items" aria-label="Eligible and ignored programmer changes">
        {preview.items.length === 0 ? <p className="update-no-op">The programmer contains no applicable content for this target.</p> : preview.items.map((item, index) => <div className={`update-preview-item outcome-${item.outcome.outcome}`} key={`${addressLabel(item)}-${index}`}><b>{addressLabel(item)}</b><span>{outcomeLabel(item)}</span></div>)}
      </div>
      {stats.changed === 0 && <p className="update-no-op" role="status">No show data would change in this mode.</p>}
      {error && <p className="modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><Button disabled={busy} onClick={onCancel}>Cancel</Button><Button className="primary" disabled={busy || stats.changed === 0} onClick={onApply}>{busy ? "Updating…" : `Update ${targetFamilyLabel(preview.target)}`}</Button></div>
    </section>
  </div>;
}

export function UpdateSettingsDialog({ settings, busy, error, onChange, onSave, onCancel }: {
  settings: UpdateSettings;
  busy: boolean;
  error: string | null;
  onChange: (settings: UpdateSettings) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return <div className="modal-backdrop update-workflow-layer" onPointerDown={(event) => event.target === event.currentTarget && onCancel()}>
    <section className="modal-card update-settings-modal" role="dialog" aria-modal="true" aria-label="Update Settings">
      <Button className="modal-close" aria-label="Close Update Settings" onClick={onCancel}>×</Button>
      <h2>Update Settings</h2>
      <p>Desk workflow preferences for Update. These settings do not change show programming.</p>
      <FormLayout labelPlacement="side">
        <SelectField label="Cue/Cuelist default" value={settings.cue_mode} onChange={(value) => onChange({ ...settings, cue_mode: value as CueUpdateMode })} options={cueUpdateModes}/>
        <SelectField label="Preset default" value={settings.preset_mode} onChange={(value) => onChange({ ...settings, preset_mode: value as ExistingContentMode })} options={existingContentModes}/>
        <SelectField label="Group default" value={settings.group_mode} onChange={(value) => onChange({ ...settings, group_mode: value as ExistingContentMode })} options={existingContentModes}/>
        <SwitchField label="Show Update modal on touch" checked={settings.show_update_modal_on_touch} onChange={(event) => onChange({ ...settings, show_update_modal_on_touch: event.target.checked })}/>
      </FormLayout>
      <p className="playback-topology-note">Command-line confirmation with Enter always applies the configured default directly.</p>
      {error && <p className="modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><Button disabled={busy} onClick={onCancel}>Cancel</Button><Button className="primary" disabled={busy} onClick={onSave}>{busy ? "Saving…" : "Save Update Settings"}</Button></div>
    </section>
  </div>;
}

function previewForMode(entry: UpdateMenuEntry, mode: UpdateMode) {
  if (entry.existing_preview.mode.target_type === mode.target_type && entry.existing_preview.mode.mode === mode.mode) return entry.existing_preview;
  if (entry.add_new_preview?.mode.target_type === mode.target_type && entry.add_new_preview.mode.mode === mode.mode) return entry.add_new_preview;
  return null;
}

export function UpdateTargetMenu({ entries, filter, modes, busyKey, error, onFilter, onMode, onApply, onCancel }: {
  entries: UpdateMenuEntry[];
  filter: UpdateTargetFilter;
  modes: Record<string, UpdateMode>;
  busyKey: string | null;
  error: string | null;
  onFilter: (filter: UpdateTargetFilter) => void;
  onMode: (key: string, mode: UpdateMode) => void;
  onApply: (entry: UpdateMenuEntry, mode: UpdateMode) => void;
  onCancel: () => void;
}) {
  return <div className="modal-backdrop update-workflow-layer" onPointerDown={(event) => event.target === event.currentTarget && onCancel()}>
    <section className="modal-card update-target-menu" role="dialog" aria-modal="true" aria-label="Update Update">
      <Button className="modal-close" aria-label="Close Update Update" onClick={onCancel}>×</Button>
      <h2>Update Update</h2>
      <p>Choose an active or referenced target related to the current programmer changes.</p>
      <div className="segmented-control" aria-label="Eligible target filter"><Button className={filter === "eligible_for_update_existing" ? "active" : ""} onClick={() => onFilter("eligible_for_update_existing")}>Eligible for Update Existing</Button><Button className={filter === "show_all_active" ? "active" : ""} onClick={() => onFilter("show_all_active")}>Show All Active</Button></div>
      <div className="update-target-list">
        {entries.length === 0 && <p className="update-no-op">No targets match this filter.</p>}
        {entries.map((entry) => {
          const key = updateTargetKey(entry.target);
          const mode = modes[key] ?? entry.existing_preview.mode;
          const preview = previewForMode(entry, mode);
          const stats = preview ? updatePreviewStats(preview) : { eligible: 0, changed: 0, ignored: 0, added: 0, source: 0, currentCue: 0 };
          const options = [entry.existing_preview, entry.add_new_preview].filter((candidate): candidate is UpdatePreview => Boolean(candidate)).map((candidate) => ({ value: JSON.stringify(candidate.mode), label: modeLabel(candidate.mode) }));
          return <article className={`update-target-row ${stats.changed === 0 ? "no-op" : ""}`} key={key}>
            <div><b>{entry.target.name}</b><span>{targetContext(entry.target)}</span><small>{stats.eligible} eligible · {stats.changed ? `${stats.changed} changes` : "No eligible change"}{stats.ignored ? ` · ${stats.ignored} ignored` : ""}</small></div>
            {filter === "show_all_active" && <SelectField label={`Mode for ${entry.target.name}`} value={JSON.stringify(mode)} onChange={(value) => onMode(key, JSON.parse(value) as UpdateMode)} options={options}/>} 
            <Button className="primary" disabled={busyKey != null || stats.changed === 0} onClick={() => onApply(entry, mode)}>{busyKey === key ? "Updating…" : stats.changed === 0 ? "No changes" : "Update"}</Button>
          </article>;
        })}
      </div>
      {error && <p className="modal-error" role="alert">{error}</p>}
      <div className="modal-actions"><Button onClick={onCancel}>Cancel</Button></div>
    </section>
  </div>;
}

function UpdateResultDialog({ result, onClose }: { result: UpdateResult; onClose: () => void }) {
  return <div className="modal-backdrop update-workflow-layer">
    <section className="modal-card update-result-modal" role="dialog" aria-modal="true" aria-label="Update complete">
      <h2>Update complete</h2>
      <p><b>{targetFamilyLabel(result.target)} · {result.target.name}</b></p>
      <p>{targetContext(result.target)}</p>
      <div className="update-preview-summary"><span>Changed {result.changed_count}</span><span>Added {result.added_count}</span><span>Ineligible {result.ignored_count}</span><span>Revision {result.revision_before} → {result.revision_after}</span></div>
      {result.changed_cues.length > 0 && <p>Changed Cue/source events: {result.changed_cues.map((cue) => `Cue ${cue.cue_number}`).join(", ")}.</p>}
      <p>{result.programmer_values_retained ? "Programmer values were retained." : "Eligible programmer values were cleared."}</p>
      <div className="modal-actions"><Button className="primary" onClick={onClose}>Close</Button></div>
    </section>
  </div>;
}

export function UpdateWorkflow() {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [settings, setSettings] = useState<UpdateSettings>(defaultUpdateSettings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [operation, setOperation] = useState<Operation | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuFilter, setMenuFilter] = useState<UpdateTargetFilter>("eligible_for_update_existing");
  const [menuEntries, setMenuEntries] = useState<UpdateMenuEntry[]>([]);
  const [menuModes, setMenuModes] = useState<Record<string, UpdateMode>>({});
  const [result, setResult] = useState<UpdateResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [busyKey, setBusyKey] = useState<string | null>(null);
  const [localError, setLocalError] = useState<string | null>(null);
  const error = localError ?? server.error;

  const disarm = () => {
    dispatch({ type: "SET_UPDATE_ARMED", value: false });
    dispatch({ type: "SET_SHIFT_ARMED", value: false });
    if (/^UPDATE\b/i.test(server.commandLine.trim())) server.resetCommandLine();
  };

  const loadMenu = async (filter: UpdateTargetFilter) => {
    setMenuFilter(filter);
    setBusy(true);
    setLocalError(null);
    const entries = await server.updateTargets(filter);
    setBusy(false);
    if (!entries) return setLocalError("Eligible Update targets could not be loaded.");
    setMenuEntries(entries);
    setMenuModes(Object.fromEntries(entries.map((entry) => [updateTargetKey(entry.target), entry.existing_preview.mode])));
  };

  useEffect(() => {
    const selectTarget = (event: Event) => {
      if (!state.updateArmed || operation || busy) return;
      const request = (event as CustomEvent<UpdateTargetRequest>).detail;
      void (async () => {
        setBusy(true);
        setLocalError(null);
        const nextSettings = await server.updateSettings();
        if (!nextSettings) {
          setBusy(false);
          setLocalError("Update settings could not be loaded.");
          disarm();
          return;
        }
        setSettings(nextSettings);
        const mode = configuredUpdateMode(nextSettings, request);
        if (!nextSettings.show_update_modal_on_touch) {
          const applied = await server.applyUpdate(request, mode);
          setBusy(false);
          disarm();
          if (applied) setResult(applied); else setLocalError("Update failed; no show data was changed.");
          return;
        }
        const preview = await server.previewUpdate(request, mode);
        setBusy(false);
        if (!preview) {
          setLocalError("Update preview failed; no show data was changed.");
          disarm();
          return;
        }
        setOperation({ request, preview });
        server.setCommandLine(`UPDATE ${targetFamilyLabel(preview.target).toUpperCase()} ${preview.target.name}`, false);
      })();
    };
    const openSettings = () => {
      void (async () => {
        disarm();
        setLocalError(null);
        setBusy(true);
        const next = await server.updateSettings();
        setBusy(false);
        setSettings(next ?? defaultUpdateSettings);
        if (!next) setLocalError("Update settings could not be loaded; deterministic defaults are shown.");
        setSettingsOpen(true);
      })();
    };
    const openMenu = () => {
      disarm();
      setMenuOpen(true);
      void loadMenu("eligible_for_update_existing");
    };
    const synchronizeArmed = (event: Event) => {
      const armed = Boolean((event as CustomEvent<boolean>).detail);
      dispatch({ type: "SET_UPDATE_ARMED", value: armed });
      if (armed) server.setCommandLine("UPDATE ", false);
      else if (/^UPDATE\b/i.test(server.commandLine.trim())) server.resetCommandLine();
    };
    window.addEventListener(UPDATE_TARGET_EVENT, selectTarget);
    window.addEventListener(UPDATE_ARMED_EVENT, synchronizeArmed);
    window.addEventListener(UPDATE_SETTINGS_EVENT, openSettings);
    window.addEventListener(UPDATE_TARGET_MENU_EVENT, openMenu);
    return () => {
      window.removeEventListener(UPDATE_TARGET_EVENT, selectTarget);
      window.removeEventListener(UPDATE_ARMED_EVENT, synchronizeArmed);
      window.removeEventListener(UPDATE_SETTINGS_EVENT, openSettings);
      window.removeEventListener(UPDATE_TARGET_MENU_EVENT, openMenu);
    };
  }, [state.updateArmed, operation, busy, server]);

  const changeOperationMode = async (mode: UpdateMode) => {
    if (!operation) return;
    setBusy(true);
    setLocalError(null);
    const preview = await server.previewUpdate(operation.request, mode);
    setBusy(false);
    if (preview) setOperation({ ...operation, preview }); else setLocalError("This Update mode could not be previewed.");
  };
  const applyOperation = async () => {
    if (!operation) return;
    setBusy(true);
    setLocalError(null);
    const applied = await server.applyUpdate(
      operation.request,
      operation.preview.mode,
      operation.preview.revision,
      operation.preview.programmer_revision,
    );
    setBusy(false);
    if (!applied) return setLocalError("Update failed; no show data was changed.");
    setOperation(null);
    disarm();
    setResult(applied);
  };
  const saveSettings = async () => {
    setBusy(true);
    setLocalError(null);
    const saved = await server.saveUpdateSettings(settings);
    setBusy(false);
    if (saved) setSettingsOpen(false); else setLocalError("Update Settings were not saved.");
  };
  const applyMenuTarget = async (entry: UpdateMenuEntry, mode: UpdateMode) => {
    const key = updateTargetKey(entry.target);
    setBusyKey(key);
    setLocalError(null);
    const selectedPreview = entry.add_new_preview?.mode.target_type === mode.target_type
      && entry.add_new_preview.mode.mode === mode.mode
      ? entry.add_new_preview
      : entry.existing_preview;
    const applied = await server.applyUpdate(
      requestFromIdentity(entry.target),
      mode,
      entry.revision,
      selectedPreview.programmer_revision,
    );
    setBusyKey(null);
    if (!applied) return setLocalError("Update failed; no show data was changed.");
    setMenuOpen(false);
    setResult(applied);
  };

  const operationStats = useMemo(() => operation ? updatePreviewStats(operation.preview) : null, [operation]);
  void operationStats;
  return <>
    {state.updateArmed && !operation && !busy && <div className="update-armed-banner" role="status">UPDATE armed · touch a recordable target or enter its address</div>}
    {busy && !operation && !settingsOpen && !menuOpen && <div className="update-armed-banner busy" role="status">Resolving authoritative Update target…</div>}
    {operation && <UpdateOperationDialog operation={operation} busy={busy} error={error} onMode={(mode) => void changeOperationMode(mode)} onApply={() => void applyOperation()} onCancel={() => { setOperation(null); setLocalError(null); disarm(); }}/>} 
    {settingsOpen && <UpdateSettingsDialog settings={settings} busy={busy} error={error} onChange={setSettings} onSave={() => void saveSettings()} onCancel={() => { setSettingsOpen(false); setLocalError(null); }}/>} 
    {menuOpen && <UpdateTargetMenu entries={menuEntries} filter={menuFilter} modes={menuModes} busyKey={busyKey} error={error} onFilter={(filter) => void loadMenu(filter)} onMode={(key, mode) => setMenuModes((current) => ({ ...current, [key]: mode }))} onApply={(entry, mode) => void applyMenuTarget(entry, mode)} onCancel={() => { setMenuOpen(false); setLocalError(null); }}/>} 
    {result && <UpdateResultDialog result={result} onClose={() => setResult(null)}/>} 
  </>;
}
