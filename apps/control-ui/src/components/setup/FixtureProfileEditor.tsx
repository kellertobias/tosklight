import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import type {
  AttributeDescriptor,
  ChannelFunctionBehavior,
  ChannelResolution,
  ColorSystem,
  FixtureChannel,
  FixtureMode,
  FixtureProfile,
  GeometryEmitter,
  GeometryNode,
  HeadColorSystem,
} from "../../api/types";
import {
  Button,
  CheckboxField,
  FormLayout,
  ModalTitleBar,
  NumberField,
  SearchBar,
  SelectField,
  TextAreaField,
  TextField,
} from "../common";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";
import { buildFixtureProfileGeometryPreview, disposeScene } from "../../windows/stage3dScene";
import {
  blankChannel,
  blankFunction,
  blankHead,
  blankMode,
  channelSplit,
  cloneProfile,
  derivePrimarySlots,
  geometryTemplate,
  maxRaw,
  reorder,
  reconcileColorSystemHighlightDefaults,
  resolutionBytes,
  semanticHighlightRaw,
  uuid,
  validateProfile,
  xyyToXyz,
  xyzToXyy,
  type GeometryTemplateName,
} from "./fixtureProfileModel";

const FIXTURE_TYPES = ["dimmer", "fogger", "profile", "wash", "wash mover", "spot mover", "beam mover", "strobe", "media server", "pixel fixture", "other"];
const RESOLUTIONS: ChannelResolution[] = ["u8", "u16", "u24", "u32"];

function fileAsDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

function optionalNumber(value: string) {
  return value === "" ? null : Number(value);
}

export function FixtureProfileEditor({
  initialProfile,
  expectedRevision = initialProfile.revision,
  manufacturers,
  attributeRegistry = [],
  onSave,
  onClose,
}: {
  initialProfile: FixtureProfile;
  expectedRevision?: number;
  manufacturers: string[];
  attributeRegistry?: AttributeDescriptor[];
  onSave: (profile: FixtureProfile, expectedRevision: number) => Promise<FixtureProfile>;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState(() => cloneProfile(initialProfile));
  const [tab, setTab] = useState<"generic" | "modes">("generic");
  const [modeId, setModeId] = useState(initialProfile.modes[0]?.id ?? "");
  const [modeEditorId, setModeEditorId] = useState<string | null>(null);
  const [modeTab, setModeTab] = useState<"heads" | "channels" | "color" | "geometry">("heads");
  const [openSplit, setOpenSplit] = useState(initialProfile.modes[0]?.splits[0]?.number ?? 1);
  const [lookup, setLookup] = useState(false);
  const [lookupQuery, setLookupQuery] = useState("");
  const [closeConfirm, setCloseConfirm] = useState(false);
  const [revisionConfirm, setRevisionConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [localErrors, setLocalErrors] = useState<string[]>([]);
  const [dragMode, setDragMode] = useState<number | null>(null);
  const baseline = useMemo(() => JSON.stringify(initialProfile), [initialProfile]);
  const dirty = JSON.stringify(draft) !== baseline;
  const selectedMode = draft.modes.find((mode) => mode.id === modeId) ?? draft.modes[0];
  const editedMode = modeEditorId ? draft.modes.find((mode) => mode.id === modeEditorId) ?? null : null;

  const requestClose = () => dirty ? setCloseConfirm(true) : onClose();
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || lookup || closeConfirm || revisionConfirm) return;
      event.preventDefault();
      event.stopPropagation();
      if (modeEditorId) {
        setModeEditorId(null);
        return;
      }
      requestClose();
    };
    window.addEventListener("keydown", keydown, true);
    return () => window.removeEventListener("keydown", keydown, true);
  });

  const updateMode = (next: FixtureMode) => setDraft((current) => ({
    ...current,
    modes: current.modes.map((mode) => mode.id === next.id ? next : mode),
  }));

  const saveNow = async () => {
    setBusy(true);
    setRevisionConfirm(false);
    try {
      const saved = await onSave(draft, expectedRevision);
      if (saved) onClose();
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason ?? "");
      setLocalErrors([message.trim() || "The fixture profile could not be saved. Check the server error and try again."]);
    } finally {
      setBusy(false);
    }
  };
  const requestSave = () => {
    const errors = validateProfile(draft);
    setLocalErrors(errors);
    if (errors.length) return;
    if (initialProfile.revision > 0) setRevisionConfirm(true);
    else void saveNow();
  };

  const addMode = () => {
    const mode = blankMode(`Mode ${draft.modes.length + 1}`);
    setDraft((current) => ({ ...current, modes: [...current.modes, mode] }));
    setModeId(mode.id);
    setModeTab("heads");
    setOpenSplit(1);
  };
  const moveMode = (index: number, next: number) => setDraft((current) => ({ ...current, modes: reorder(current.modes, index, next) }));
  const moveModeById = (sourceId: string, targetId: string) => setDraft((current) => {
    const from = current.modes.findIndex((mode) => mode.id === sourceId);
    const to = current.modes.findIndex((mode) => mode.id === targetId);
    return from < 0 || to < 0 || from === to ? current : { ...current, modes: reorder(current.modes, from, to) };
  });
  const deleteMode = (id: string) => {
    if (draft.modes.length === 1) return;
    const next = draft.modes.filter((mode) => mode.id !== id);
    setDraft((current) => ({ ...current, modes: next }));
    if (modeId === id) setModeId(next[0].id);
    if (modeEditorId === id) setModeEditorId(null);
  };

  return <div className="stacked-modal-layer fixture-profile-editor-layer" onPointerDown={(event) => event.target === event.currentTarget && requestClose()}>
    <section className="nested-modal fixture-profile-editor-modal" role="dialog" aria-modal="true" aria-label={initialProfile.revision ? "Edit fixture profile" : "Create fixture profile"}>
      <ModalTitleBar
        title={initialProfile.revision ? `Edit ${initialProfile.manufacturer} ${initialProfile.name}` : "Create fixture"}
        tabs={[{ id: "generic", label: "Generic" }, { id: "modes", label: "Modes" }]}
        activeTab={tab}
        onTabChange={(id) => setTab(id as typeof tab)}
        actions={<Button variant="primary" loading={busy} onClick={requestSave}>Save fixture</Button>}
        closeLabel="Close fixture editor"
        onClose={requestClose}
      />

      <div className="fixture-profile-editor-body">
        <datalist id="fixture-attribute-registry">{attributeRegistry.map((descriptor) => <option
          key={descriptor.id}
          value={descriptor.id}
          data-family={descriptor.family}
          data-value-type={descriptor.value_type}
          data-default-unit={descriptor.default_unit ?? ""}
        >{descriptor.family} · {descriptor.label}</option>)}</datalist>
        {localErrors.length > 0 && <section className="fixture-profile-errors" role="alert"><strong>Fixture profile needs attention</strong><ul>{localErrors.map((error) => <li key={error}>{error}</li>)}</ul></section>}
        {tab === "generic" && <div className="fixture-generic-tab">
          <section><h3>Identity</h3><FormLayout columns={3} minColumnWidth={190}>
            <div className="fixture-manufacturer-field"><TextField required label="Manufacturer" clearable value={draft.manufacturer} onChange={(event) => setDraft({ ...draft, manufacturer: event.target.value })}/><Button iconOnly aria-label="Look up manufacturer" title="Look up manufacturer" onClick={() => { setLookupQuery(""); setLookup(true); }}>⌕</Button></div>
            <TextField required label="Fixture name" clearable value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })}/>
            <TextField label="Fixture short name" clearable value={draft.short_name} onChange={(event) => setDraft({ ...draft, short_name: event.target.value })}/>
            <SelectField label="Fixture type" value={draft.fixture_type} options={FIXTURE_TYPES.map((value) => ({ value, label: value }))} onChange={(fixture_type) => setDraft({ ...draft, fixture_type })}/>
            <AssetField label="Fixture icon" value={draft.stage_icon_asset} extensions={["png", "jpg", "jpeg", "gif", "webp", "svg"]} onChange={(stage_icon_asset) => setDraft({ ...draft, stage_icon_asset })}/>
            <AssetField label="Visualizer GLB model" value={draft.model_asset} extensions={["glb"]} onChange={(model_asset) => setDraft({ ...draft, model_asset })}/>
          </FormLayout></section>
          <section><h3>Physical</h3><FormLayout columns={5} minColumnWidth={145}>
            {([
              ["width_millimetres", "Width", "mm"],
              ["height_millimetres", "Height", "mm"],
              ["depth_millimetres", "Depth", "mm"],
              ["weight_kilograms", "Weight", "kg"],
              ["power_watts", "Power consumption", "W"],
            ] as const).map(([key, label, unit]) => <NumberField key={key} label={`${label} (${unit})`} allowDecimal min={0} value={draft.physical[key] ?? ""} onChange={(event) => setDraft({ ...draft, physical: { ...draft.physical, [key]: optionalNumber(event.target.value) } })}/>) }
          </FormLayout></section>
          <section className="fixture-notes-picture"><div><h3>Notes</h3><TextAreaField label="Fixture notes" rows={9} value={draft.notes} onChange={(event) => setDraft({ ...draft, notes: event.target.value })}/></div><div><h3>Fixture photograph</h3><AssetField label="Photograph" preview value={draft.photograph_asset} extensions={["png", "jpg", "jpeg", "gif", "webp"]} onChange={(photograph_asset) => setDraft({ ...draft, photograph_asset })}/></div></section>
        </div>}

        {tab === "modes" && selectedMode && <div className="fixture-modes-tab">
          <aside className="fixture-mode-list"><header><h3>Modes</h3><Button onClick={addMode}>Add mode</Button></header>{draft.modes.map((mode, index) => <article key={mode.id} data-mode-reorder-id={mode.id} className={mode.id === selectedMode.id ? "active" : ""} draggable onDragStart={() => setDragMode(index)} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); if (dragMode != null) moveMode(dragMode, index); setDragMode(null); }}>
            <span className="drag-handle touch-drag-handle" aria-hidden="true" title="Drag to reorder modes" onPointerDown={(event) => { if (event.pointerType === "mouse") return; event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { if (event.pointerType === "mouse" || !event.currentTarget.hasPointerCapture(event.pointerId)) return; const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-mode-reorder-id]")?.dataset.modeReorderId; if (target) moveModeById(mode.id, target); }} onPointerUp={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && event.currentTarget.releasePointerCapture(event.pointerId)} onPointerCancel={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && event.currentTarget.releasePointerCapture(event.pointerId)}>⠿</span>
            <Button className="mode-select" active={mode.id === selectedMode.id} onClick={() => { setModeId(mode.id); setOpenSplit(mode.splits[0]?.number ?? 1); }}>{mode.name || "Unnamed mode"}<small>{mode.channels.length} channels · {mode.splits.length} split{mode.splits.length === 1 ? "" : "s"}</small></Button>
            <Button aria-label={`Edit channels for ${mode.name || "unnamed mode"}`} onClick={() => { setModeId(mode.id); setModeEditorId(mode.id); setOpenSplit(mode.splits[0]?.number ?? 1); setModeTab("channels"); }}>Edit channels</Button>
            <div className="reorder-actions"><Button iconOnly aria-label={`Move ${mode.name} up`} disabled={index === 0} onClick={() => moveMode(index, index - 1)}>▲</Button><Button iconOnly aria-label={`Move ${mode.name} down`} disabled={index === draft.modes.length - 1} onClick={() => moveMode(index, index + 1)}>▼</Button><Button iconOnly aria-label={`Remove ${mode.name}`} disabled={draft.modes.length === 1} title={draft.modes.length === 1 ? "The final mode cannot be removed" : "Remove mode"} onClick={() => deleteMode(mode.id)}>×</Button></div>
          </article>)}</aside>
          <section className="fixture-mode-workspace fixture-mode-summary"><header className="fixture-mode-identity"><TextField label="Mode name" required value={selectedMode.name} onChange={(event) => updateMode({ ...selectedMode, name: event.target.value })}/><TextField label="Mode notes" value={selectedMode.notes} onChange={(event) => updateMode({ ...selectedMode, notes: event.target.value })}/></header>
            <div><h3>Channel configuration</h3><p>{selectedMode.heads.length} head{selectedMode.heads.length === 1 ? "" : "s"} · {selectedMode.channels.length} logical channel{selectedMode.channels.length === 1 ? "" : "s"} · {selectedMode.splits.length} split{selectedMode.splits.length === 1 ? "" : "s"}</p><Button variant="primary" onClick={() => { setModeEditorId(selectedMode.id); setOpenSplit(selectedMode.splits[0]?.number ?? 1); setModeTab("channels"); }}>Edit channels</Button></div>
          </section>
        </div>}
      </div>
    </section>

    {editedMode && <div className="stacked-modal-layer fixture-mode-editor-layer" onPointerDown={(event) => event.target === event.currentTarget && setModeEditorId(null)}><section className="nested-modal fixture-mode-editor-modal" role="dialog" aria-modal="true" aria-label={`Edit ${editedMode.name || "unnamed"} mode`}>
      <ModalTitleBar
        title={`Edit channels · ${editedMode.name || "Unnamed mode"}`}
        tabs={(["heads", "channels", "color", "geometry"] as const).map((id) => ({ id, label: id[0].toUpperCase() + id.slice(1) }))}
        activeTab={modeTab}
        onTabChange={(id) => setModeTab(id as typeof modeTab)}
        closeLabel="Close mode editor"
        onClose={() => setModeEditorId(null)}
      />
      <div className="fixture-mode-editor-body">
        {modeTab === "heads" && <HeadsEditor mode={editedMode} onChange={updateMode}/>} 
        {modeTab === "channels" && <ChannelsEditor mode={editedMode} attributeRegistry={attributeRegistry} openSplit={openSplit} onOpenSplit={setOpenSplit} onChange={updateMode}/>} 
        {modeTab === "color" && <ColorEditor mode={editedMode} onChange={updateMode}/>} 
        {modeTab === "geometry" && <GeometryEditor mode={editedMode} onChange={updateMode}/>} 
      </div>
    </section></div>}

    {lookup && <ManufacturerLookup manufacturers={manufacturers} query={lookupQuery} onQuery={setLookupQuery} onSelect={(manufacturer) => { setDraft({ ...draft, manufacturer }); setLookup(false); }} onClose={() => setLookup(false)}/>} 
    {closeConfirm && <ConfirmDialog title="Discard fixture changes?" description="This fixture profile has unsaved changes." primary="Discard changes" danger onPrimary={onClose} secondary="Stay" onSecondary={() => setCloseConfirm(false)}/>} 
    {revisionConfirm && <ConfirmDialog title="Create a new fixture revision?" description={`Revision ${initialProfile.revision} remains unchanged. The complete fixture profile, including every mode, will be saved as a new atomic revision.`} primary="Save and create revision" onPrimary={() => void saveNow()} secondary="Keep editing" onSecondary={() => setRevisionConfirm(false)}/>} 
  </div>;
}

function AssetField({ label, value, extensions, preview = false, onChange }: { label: string; value: string | null; extensions: string[]; preview?: boolean; onChange: (value: string | null) => void }) {
  return <div className="fixture-asset-field"><label>{label}</label>{preview && value && <img src={value} alt="Fixture photograph preview"/>}<div><RootConfinedFilePickerButton label={value ? `Replace ${label.toLowerCase()}` : `Choose ${label.toLowerCase()}`} allowedExtensions={extensions} onFiles={(files) => { const file = files[0]; if (file) return fileAsDataUrl(file).then(onChange); }}/>{value && <Button onClick={() => onChange(null)}>Remove</Button>}</div><small>{value ? `${label} assigned` : `No ${label.toLowerCase()} assigned`}</small></div>;
}

function ManufacturerLookup({ manufacturers, query, onQuery, onSelect, onClose }: { manufacturers: string[]; query: string; onQuery: (value: string) => void; onSelect: (value: string) => void; onClose: () => void }) {
  const unique = new Map<string, string>();
  for (const manufacturer of manufacturers) if (!unique.has(manufacturer.toLocaleLowerCase())) unique.set(manufacturer.toLocaleLowerCase(), manufacturer);
  const matches = [...unique.values()].filter((value) => value.toLocaleLowerCase().includes(query.toLocaleLowerCase())).sort((left, right) => left.localeCompare(right));
  return <div className="stacked-modal-layer manufacturer-lookup-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}><section className="nested-modal manufacturer-lookup" role="dialog" aria-modal="true" aria-label="Manufacturer lookup"><ModalTitleBar title="Manufacturer lookup" search={<SearchBar value={query} onChange={onQuery} ariaLabel="Search manufacturers" placeholder="Search manufacturers"/>} closeLabel="Close manufacturer lookup" onClose={onClose}/><div className="manufacturer-results" role="listbox" aria-label="Manufacturers">{matches.map((manufacturer) => <Button role="option" key={manufacturer} onClick={() => onSelect(manufacturer)}>{manufacturer}</Button>)}{!matches.length && <p>No manufacturer matches this search. Close the lookup to keep typing a new manufacturer.</p>}</div></section></div>;
}

function ConfirmDialog({ title, description, primary, secondary, danger = false, onPrimary, onSecondary }: { title: string; description: string; primary: string; secondary: string; danger?: boolean; onPrimary: () => void; onSecondary: () => void }) {
  return <div className="stacked-modal-layer fixture-confirm-layer"><section className="nested-modal fixture-confirm-dialog" role="alertdialog" aria-modal="true" aria-label={title}><ModalTitleBar title={title}/><p>{description}</p><div className="modal-actions"><Button autoFocus onClick={onSecondary}>{secondary}</Button><Button variant={danger ? "danger" : "primary"} onClick={onPrimary}>{primary}</Button></div></section></div>;
}

function HeadsEditor({ mode, onChange }: { mode: FixtureMode; onChange: (mode: FixtureMode) => void }) {
  const [dragHead, setDragHead] = useState<number | null>(null);
  const moveHeadById = (sourceId: string, targetId: string) => {
    const from = mode.heads.findIndex((head) => head.id === sourceId);
    const to = mode.heads.findIndex((head) => head.id === targetId);
    if (from >= 0 && to >= 0 && from !== to) onChange({ ...mode, heads: reorder(mode.heads, from, to) });
  };
  const addSplit = () => {
    const number = Math.max(0, ...mode.splits.map((split) => split.number)) + 1;
    onChange({ ...mode, splits: [...mode.splits, { number, footprint: 1 }] });
  };
  const removeHead = (headId: string) => {
    if (mode.heads.length === 1 || mode.channels.some((channel) => channel.head_id === headId)) return;
    onChange({ ...mode, heads: mode.heads.filter((head) => head.id !== headId), color_systems: mode.color_systems.filter((system) => system.head_id !== headId), geometry: { ...mode.geometry, emitters: mode.geometry.emitters.filter((emitter) => emitter.head_id !== headId) } });
  };
  return <div className="fixture-heads-editor"><section><header><h3>Splits</h3><Button onClick={addSplit}>Add split</Button></header><div className="fixture-split-list">{mode.splits.map((split, index) => { const used = mode.heads.some((head) => head.split === split.number); return <article key={split.number}><strong>Split {split.number}</strong><NumberField label="Footprint" min={1} max={512} value={split.footprint} onChange={(event) => onChange({ ...mode, splits: mode.splits.map((candidate) => candidate.number === split.number ? { ...candidate, footprint: Number(event.target.value) } : candidate) })}/><Button iconOnly aria-label={`Move split ${split.number} up`} disabled={index === 0} onClick={() => onChange({ ...mode, splits: reorder(mode.splits, index, index - 1) })}>▲</Button><Button iconOnly aria-label={`Move split ${split.number} down`} disabled={index === mode.splits.length - 1} onClick={() => onChange({ ...mode, splits: reorder(mode.splits, index, index + 1) })}>▼</Button><Button iconOnly aria-label={`Remove split ${split.number}`} disabled={mode.splits.length === 1 || used} title={used ? "Reassign its heads before removing this split" : "Remove split"} onClick={() => onChange({ ...mode, splits: mode.splits.filter((candidate) => candidate.number !== split.number) })}>×</Button></article>; })}</div></section>
    <section><header><h3>Heads</h3><Button onClick={() => { const head = blankHead(mode.heads.length, mode.splits[0]?.number); onChange({ ...mode, heads: [...mode.heads, { ...head, master_shared: false }] }); }}>Add head</Button></header><div className="fixture-head-list">{mode.heads.map((head, index) => { const ownsChannels = mode.channels.some((channel) => channel.head_id === head.id); return <article key={head.id} data-head-reorder-id={head.id} draggable onDragStart={() => setDragHead(index)} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); if (dragHead != null) onChange({ ...mode, heads: reorder(mode.heads, dragHead, index) }); setDragHead(null); }}><span className="drag-handle touch-drag-handle" aria-hidden="true" title="Drag to reorder heads" onPointerDown={(event) => { if (event.pointerType === "mouse") return; event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { if (event.pointerType === "mouse" || !event.currentTarget.hasPointerCapture(event.pointerId)) return; const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-head-reorder-id]")?.dataset.headReorderId; if (target) moveHeadById(head.id, target); }} onPointerUp={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && event.currentTarget.releasePointerCapture(event.pointerId)} onPointerCancel={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && event.currentTarget.releasePointerCapture(event.pointerId)}>⠿</span><TextField label="Head name" value={head.name} onChange={(event) => onChange({ ...mode, heads: mode.heads.map((candidate) => candidate.id === head.id ? { ...candidate, name: event.target.value } : candidate) })}/><SelectField label="Split" value={String(head.split)} options={mode.splits.map((split) => ({ value: String(split.number), label: `Split ${split.number}` }))} onChange={(value) => onChange({ ...mode, heads: mode.heads.map((candidate) => candidate.id === head.id ? { ...candidate, split: Number(value) } : candidate) })}/><CheckboxField label="Master/shared head" checked={head.master_shared} onChange={(event) => onChange({ ...mode, heads: mode.heads.map((candidate) => ({ ...candidate, master_shared: candidate.id === head.id ? event.target.checked : event.target.checked ? false : candidate.master_shared })) })}/><div className="reorder-actions"><Button iconOnly aria-label={`Move ${head.name} up`} disabled={index === 0} onClick={() => onChange({ ...mode, heads: reorder(mode.heads, index, index - 1) })}>▲</Button><Button iconOnly aria-label={`Move ${head.name} down`} disabled={index === mode.heads.length - 1} onClick={() => onChange({ ...mode, heads: reorder(mode.heads, index, index + 1) })}>▼</Button><Button iconOnly aria-label={`Remove ${head.name}`} disabled={mode.heads.length === 1 || ownsChannels} title={ownsChannels ? "Remove or reassign this head's channels first" : mode.heads.length === 1 ? "The final head cannot be removed" : "Remove head"} onClick={() => removeHead(head.id)}>×</Button></div></article>; })}</div></section>
  </div>;
}

function ChannelsEditor({ mode, attributeRegistry, openSplit, onOpenSplit, onChange }: { mode: FixtureMode; attributeRegistry: AttributeDescriptor[]; openSplit: number; onOpenSplit: (split: number) => void; onChange: (mode: FixtureMode) => void }) {
  const [dragChannel, setDragChannel] = useState<string | null>(null);
  const primary = derivePrimarySlots(mode);
  const activeSplit = mode.splits.some((split) => split.number === openSplit) ? openSplit : mode.splits[0]?.number;
  const setChannel = (channel: FixtureChannel) => onChange({ ...mode, channels: mode.channels.map((candidate) => candidate.id === channel.id ? channel : candidate) });
  const addChannel = (split: number) => {
    const channel = blankChannel(mode, split);
    onChange({
      ...mode,
      splits: mode.splits.map((candidate) => candidate.number === split ? { ...candidate, footprint: Math.min(512, Math.max(candidate.footprint, mode.channels.filter((item) => channelSplit(mode, item) === split).length + 1)) } : candidate),
      channels: [...mode.channels, channel],
    });
  };
  const changeResolution = (channel: FixtureChannel, resolution: ChannelResolution) => {
    const split = channelSplit(mode, channel);
    const footprint = mode.splits.find((candidate) => candidate.number === split)?.footprint ?? 1;
    const occupied = new Set(mode.channels.filter((candidate) => candidate.id !== channel.id && channelSplit(mode, candidate) === split).flatMap((candidate) => candidate.secondary_slots));
    const primarySlot = primary.slots.get(channel.id) ?? 1;
    occupied.add(primarySlot);
    const secondary: number[] = [];
    let candidate = 1;
    while (secondary.length < resolutionBytes(resolution) - 1) {
      while (occupied.has(candidate) || secondary.includes(candidate)) candidate += 1;
      secondary.push(candidate);
      candidate += 1;
    }
    const neededFootprint = Math.max(footprint, primarySlot, ...secondary);
    onChange({
      ...mode,
      splits: mode.splits.map((item) => item.number === split ? { ...item, footprint: Math.min(512, neededFootprint) } : item),
      channels: mode.channels.map((item) => item.id === channel.id ? { ...item, resolution, secondary_slots: secondary, default_raw: Math.min(item.default_raw, maxRaw(resolution)), highlight_raw: Math.min(item.highlight_raw, maxRaw(resolution)) } : item),
    });
  };
  const moveChannel = (channel: FixtureChannel, direction: -1 | 1) => {
    const splitChannels = mode.channels.filter((candidate) => channelSplit(mode, candidate) === channelSplit(mode, channel));
    const within = splitChannels.findIndex((candidate) => candidate.id === channel.id);
    const peer = splitChannels[within + direction];
    if (!peer) return;
    const from = mode.channels.findIndex((candidate) => candidate.id === channel.id);
    const to = mode.channels.findIndex((candidate) => candidate.id === peer.id);
    onChange({ ...mode, channels: reorder(mode.channels, from, to) });
  };
  const moveChannelById = (sourceId: string, targetId: string) => {
    const source = mode.channels.find((channel) => channel.id === sourceId);
    const target = mode.channels.find((channel) => channel.id === targetId);
    if (!source || !target || source.id === target.id || channelSplit(mode, source) !== channelSplit(mode, target)) return;
    const from = mode.channels.findIndex((channel) => channel.id === source.id);
    const to = mode.channels.findIndex((channel) => channel.id === target.id);
    onChange({ ...mode, channels: reorder(mode.channels, from, to) });
  };
  const dropChannel = (target: FixtureChannel) => {
    const source = mode.channels.find((channel) => channel.id === dragChannel);
    setDragChannel(null);
    if (!source || source.id === target.id || channelSplit(mode, source) !== channelSplit(mode, target)) return;
    const from = mode.channels.findIndex((channel) => channel.id === source.id);
    const to = mode.channels.findIndex((channel) => channel.id === target.id);
    onChange({ ...mode, channels: reorder(mode.channels, from, to) });
  };
  const renderSplit = (split: number) => {
    const channels = mode.channels.filter((channel) => channelSplit(mode, channel) === split);
    return <div className="fixture-channel-split"><div className="fixture-channel-table-wrap"><table className="fixture-channel-table"><thead><tr><th>Slot</th><th>Head</th><th>Attribute</th><th>Resolution</th><th>Fine</th><th>Third byte</th><th>Fourth byte</th><th>Default raw</th><th>Highlight raw</th><th>Behavior</th><th>Order</th></tr></thead><tbody>{channels.map((channel, index) => <Fragment key={channel.id}><ChannelRow mode={mode} channel={channel} attributeRegistry={attributeRegistry} primary={primary.slots.get(channel.id) ?? 0} first={index === 0} last={index === channels.length - 1} onChange={setChannel} onResolution={(value) => changeResolution(channel, value)} onMove={(direction) => moveChannel(channel, direction)} onDragStart={() => setDragChannel(channel.id)} onDrop={() => dropChannel(channel)} onTouchMove={(targetId) => moveChannelById(channel.id, targetId)} onRemove={() => onChange({ ...mode, channels: mode.channels.filter((candidate) => candidate.id !== channel.id), control_actions: mode.control_actions.map((action) => ({ ...action, assignments: action.assignments.filter((assignment) => assignment.channel_id !== channel.id) })).filter((action) => action.assignments.length), color_systems: mode.color_systems.map((system) => removeColorChannel(system, channel.id)).filter((system): system is HeadColorSystem => Boolean(system)) })}/><tr className="fixture-channel-details-row" data-channel-reorder-id={channel.id}><td colSpan={11}><ChannelDetails channel={channel} actionIds={mode.control_actions} onChange={setChannel}/></td></tr></Fragment>)}</tbody></table></div>{!channels.length && <p className="empty-editor-message">No logical channels are assigned to split {split}.</p>}<Button onClick={() => addChannel(split)}>Add channel</Button></div>;
  };
  return <div className="fixture-channels-editor">
    {mode.splits.length === 1 ? renderSplit(mode.splits[0].number) : <div className="fixture-split-accordions">{mode.splits.map((split) => <section key={split.number} className={activeSplit === split.number ? "open" : ""}><Button className="fixture-split-accordion-title" aria-expanded={activeSplit === split.number} onClick={() => onOpenSplit(split.number)}><span>Split {split.number}</span><small>{split.footprint} slots · {mode.channels.filter((channel) => channelSplit(mode, channel) === split.number).length} channels</small></Button>{activeSplit === split.number && renderSplit(split.number)}</section>)}</div>}
    <ControlActionsEditor mode={mode} onChange={onChange}/>
    {primary.errors.length > 0 && <div className="fixture-inline-errors" role="alert">{primary.errors.map((error) => <p key={error}>{error}</p>)}</div>}
  </div>;
}

export function applyCanonicalChannelAttribute(
  channel: FixtureChannel,
  attribute: string,
  registry: AttributeDescriptor[],
): FixtureChannel {
  const descriptor = registry.find((candidate) => candidate.id === attribute);
  const choices = channel.functions.flatMap((fn) => fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
    ? [{ semantic_id: fn.behavior.semantic_id, label: fn.behavior.label, raw_value: fn.behavior.raw_value }]
    : []);
  const previousDefault = semanticHighlightRaw(channel.attribute, channel.resolution, channel.default_raw, channel.invert, choices);
  const nextDefault = semanticHighlightRaw(attribute, channel.resolution, channel.default_raw, channel.invert, choices);
  return {
    ...channel,
    attribute,
    highlight_raw: channel.highlight_raw === previousDefault ? nextDefault : channel.highlight_raw,
    unit: descriptor?.default_unit ?? (descriptor ? null : channel.unit),
  };
}

function ChannelRow({ mode, channel, attributeRegistry, primary, first, last, onChange, onResolution, onMove, onDragStart, onDrop, onTouchMove, onRemove }: { mode: FixtureMode; channel: FixtureChannel; attributeRegistry: AttributeDescriptor[]; primary: number; first: boolean; last: boolean; onChange: (channel: FixtureChannel) => void; onResolution: (resolution: ChannelResolution) => void; onMove: (direction: -1 | 1) => void; onDragStart: () => void; onDrop: () => void; onTouchMove: (targetId: string) => void; onRemove: () => void }) {
  const components = resolutionBytes(channel.resolution) - 1;
  const secondary = (index: number, value: string) => {
    const slots = [...channel.secondary_slots];
    slots[index] = Number(value);
    onChange({ ...channel, secondary_slots: slots });
  };
  return <tr className="fixture-channel-row" data-channel-reorder-id={channel.id} draggable onDragStart={onDragStart} onDragOver={(event) => event.preventDefault()} onDrop={(event) => { event.preventDefault(); onDrop(); }}><td className="channel-primary-slot"><span className="drag-handle touch-drag-handle" aria-hidden="true" title="Drag to reorder channels" onPointerDown={(event) => { if (event.pointerType === "mouse") return; event.preventDefault(); event.currentTarget.setPointerCapture(event.pointerId); }} onPointerMove={(event) => { if (event.pointerType === "mouse" || !event.currentTarget.hasPointerCapture(event.pointerId)) return; const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-channel-reorder-id]")?.dataset.channelReorderId; if (target) onTouchMove(target); }} onPointerUp={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && event.currentTarget.releasePointerCapture(event.pointerId)} onPointerCancel={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && event.currentTarget.releasePointerCapture(event.pointerId)}>⠿</span><strong>{primary || "!"}</strong></td><td><SelectField value={channel.head_id} options={mode.heads.map((head) => ({ value: head.id, label: head.name }))} onChange={(head_id) => onChange({ ...channel, head_id })}/></td><td><TextField aria-label="Channel attribute" list="fixture-attribute-registry" value={channel.attribute} onChange={(event) => onChange(applyCanonicalChannelAttribute(channel, event.target.value, attributeRegistry))}/></td><td><SelectField aria-label="Channel resolution" value={channel.resolution} options={RESOLUTIONS.map((value) => ({ value, label: value.slice(1) + " bit" }))} onChange={onResolution}/></td>
    {[0, 1, 2].map((index) => <td key={index}>{components > index ? <NumberField aria-label={`${["Fine", "Third byte", "Fourth byte"][index]} slot for ${channel.attribute}`} min={1} max={512} value={channel.secondary_slots[index] ?? ""} onChange={(event) => secondary(index, event.target.value)}/> : <span aria-label="Not used">—</span>}</td>)}
    <td><NumberField aria-label={`Default raw for ${channel.attribute}`} min={0} max={maxRaw(channel.resolution)} value={channel.default_raw} onChange={(event) => onChange({ ...channel, default_raw: Number(event.target.value) })}/></td><td><NumberField aria-label={`Highlight raw for ${channel.attribute}`} min={0} max={maxRaw(channel.resolution)} value={channel.highlight_raw} onChange={(event) => onChange({ ...channel, highlight_raw: Number(event.target.value) })}/></td><td><SelectField aria-label="Channel behavior" value={channel.behavior} options={[{ value: "controlled", label: "Controlled" }, { value: "static", label: "Static" }]} onChange={(behavior) => onChange({ ...channel, behavior })}/></td><td><div className="reorder-actions"><Button iconOnly aria-label={`Move ${channel.attribute} up`} disabled={first} onClick={() => onMove(-1)}>▲</Button><Button iconOnly aria-label={`Move ${channel.attribute} down`} disabled={last} onClick={() => onMove(1)}>▼</Button><Button iconOnly aria-label={`Remove ${channel.attribute}`} onClick={onRemove}>×</Button></div></td></tr>;
}

function ChannelDetails({ channel, actionIds, onChange }: { channel: FixtureChannel; actionIds: Array<{ id: string; name: string }>; onChange: (channel: FixtureChannel) => void }) {
  return <div className="fixture-channel-details"><FormLayout columns={3} minColumnWidth={150}><NumberField label="Physical minimum" allowDecimal value={channel.physical_min ?? ""} onChange={(event) => onChange({ ...channel, physical_min: optionalNumber(event.target.value) })}/><NumberField label="Physical maximum" allowDecimal value={channel.physical_max ?? ""} onChange={(event) => onChange({ ...channel, physical_max: optionalNumber(event.target.value) })}/><TextField label="Physical unit" value={channel.unit ?? ""} onChange={(event) => onChange({ ...channel, unit: event.target.value || null })}/></FormLayout><div className="fixture-channel-flags"><CheckboxField label="Invert" checked={channel.invert} onChange={(event) => onChange({ ...channel, invert: event.target.checked })}/><CheckboxField label="Snap (never fades)" checked={channel.snap} onChange={(event) => onChange({ ...channel, snap: event.target.checked })}/><CheckboxField label="Reacts to virtual intensity" checked={channel.reacts_to_virtual_intensity} onChange={(event) => onChange({ ...channel, reacts_to_virtual_intensity: event.target.checked })}/><CheckboxField label="Reacts to sequence master" checked={channel.reacts_to_sequence_master} onChange={(event) => onChange({ ...channel, reacts_to_sequence_master: event.target.checked })}/><CheckboxField label="Reacts to group master" checked={channel.reacts_to_group_master} onChange={(event) => onChange({ ...channel, reacts_to_group_master: event.target.checked })}/><CheckboxField label="Reacts to grand master" checked={channel.reacts_to_grand_master} onChange={(event) => onChange({ ...channel, reacts_to_grand_master: event.target.checked })}/></div><details className="fixture-functions"><summary>Channel functions ({channel.functions.length})</summary>{channel.functions.map((fn, index) => <article key={fn.id}><TextField label="Function name" value={fn.name} onChange={(event) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? { ...candidate, name: event.target.value } : candidate) })}/><TextField label="Attribute" list="fixture-attribute-registry" value={fn.attribute} onChange={(event) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? { ...candidate, attribute: event.target.value } : candidate) })}/><NumberField label="DMX from" min={0} max={maxRaw(channel.resolution)} value={fn.dmx_from} onChange={(event) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? { ...candidate, dmx_from: Number(event.target.value) } : candidate) })}/><NumberField label="DMX to" min={0} max={maxRaw(channel.resolution)} value={fn.dmx_to} onChange={(event) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? { ...candidate, dmx_to: Number(event.target.value) } : candidate) })}/><NumberField label="Priority" value={fn.priority} onChange={(event) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? { ...candidate, priority: Number(event.target.value) } : candidate) })}/><SelectField label="Function behavior" value={fn.behavior.type} options={[{ value: "continuous", label: "Continuous mapping" }, { value: "fixed", label: "Named fixed value" }, { value: "indexed", label: "Indexed color or gobo" }, { value: "control", label: "Control action" }]} onChange={(type) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? replaceFunctionBehavior(candidate, type, channel) : candidate) })}/><FunctionBehaviorEditor behavior={fn.behavior} modeChannel={channel} actionIds={actionIds} onChange={(behavior) => onChange({ ...channel, functions: channel.functions.map((candidate) => candidate.id === fn.id ? { ...candidate, behavior } : candidate) })}/><div className="reorder-actions"><Button iconOnly aria-label={`Move function ${fn.name} up`} disabled={index === 0} onClick={() => onChange({ ...channel, functions: reorder(channel.functions, index, index - 1) })}>▲</Button><Button iconOnly aria-label={`Move function ${fn.name} down`} disabled={index === channel.functions.length - 1} onClick={() => onChange({ ...channel, functions: reorder(channel.functions, index, index + 1) })}>▼</Button><Button onClick={() => onChange({ ...channel, functions: channel.functions.filter((candidate) => candidate.id !== fn.id) })}>Remove function</Button></div></article>)}<Button onClick={() => onChange({ ...channel, functions: [...channel.functions, blankFunction(channel)] })}>Add function</Button></details></div>;
}

function functionBehavior(type: ChannelFunctionBehavior["type"], channel: FixtureChannel): ChannelFunctionBehavior {
  if (type === "continuous") return { type, physical_min: channel.physical_min ?? 0, physical_max: channel.physical_max ?? 1, unit: channel.unit };
  if (type === "control") return { type, action_id: "" };
  return { type, semantic_id: "", label: "", raw_value: 0 };
}

export function replaceFunctionBehavior(
  fn: FixtureChannel["functions"][number],
  type: ChannelFunctionBehavior["type"],
  channel: FixtureChannel,
): FixtureChannel["functions"][number] {
  return {
    ...fn,
    priority: type === "continuous" ? 0 : type === "control" ? 200 : 100,
    behavior: functionBehavior(type, channel),
  };
}

function FunctionBehaviorEditor({ behavior, modeChannel, actionIds, onChange }: { behavior: ChannelFunctionBehavior; modeChannel: FixtureChannel; actionIds: Array<{ id: string; name: string }>; onChange: (behavior: ChannelFunctionBehavior) => void }) {
  if (behavior.type === "continuous") return <><NumberField label="Function physical minimum" allowDecimal value={behavior.physical_min} onChange={(event) => onChange({ ...behavior, physical_min: Number(event.target.value) })}/><NumberField label="Function physical maximum" allowDecimal value={behavior.physical_max} onChange={(event) => onChange({ ...behavior, physical_max: Number(event.target.value) })}/><TextField label="Function unit" value={behavior.unit ?? ""} onChange={(event) => onChange({ ...behavior, unit: event.target.value || null })}/></>;
  if (behavior.type === "control") return <SelectField label="Control action" value={behavior.action_id} options={[{ value: "", label: "Choose action" }, ...actionIds.map((action) => ({ value: action.id, label: action.name }))]} onChange={(action_id) => onChange({ ...behavior, action_id })}/>;
  return <><TextField label="Portable semantic ID" value={behavior.semantic_id} onChange={(event) => onChange({ ...behavior, semantic_id: event.target.value })}/><TextField label="Fixture label" value={behavior.label} onChange={(event) => onChange({ ...behavior, label: event.target.value })}/><NumberField label="Exact raw value" min={0} max={maxRaw(modeChannel.resolution)} value={behavior.raw_value} onChange={(event) => onChange({ ...behavior, raw_value: Number(event.target.value) })}/></>;
}

function ControlActionsEditor({ mode, onChange }: { mode: FixtureMode; onChange: (mode: FixtureMode) => void }) {
  return <section className="fixture-control-actions"><header><h3>Typed control actions</h3><Button onClick={() => onChange({ ...mode, control_actions: [...mode.control_actions, { id: uuid(), name: `Action ${mode.control_actions.length + 1}`, kind: "momentary", duration_millis: null, assignments: [] }] })}>Add control action</Button></header>{mode.control_actions.map((action) => <article key={action.id}><TextField label="Action name" value={action.name} onChange={(event) => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, name: event.target.value } : candidate) })}/><SelectField label="Action kind" value={action.kind} options={[{ value: "latched", label: "Latched" }, { value: "momentary", label: "Momentary" }, { value: "timed_pulse", label: "Timed pulse" }]} onChange={(kind) => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, kind, duration_millis: kind === "timed_pulse" ? candidate.duration_millis ?? 1000 : null } : candidate) })}/>{action.kind === "timed_pulse" && <NumberField label="Duration (ms)" min={1} value={action.duration_millis ?? 1000} onChange={(event) => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, duration_millis: Number(event.target.value) } : candidate) })}/>}<Button onClick={() => onChange({ ...mode, control_actions: mode.control_actions.filter((candidate) => candidate.id !== action.id) })}>Remove action</Button><div className="control-assignments">{action.assignments.map((assignment, index) => { const channel = mode.channels.find((candidate) => candidate.id === assignment.channel_id); return <div key={`${assignment.channel_id}-${index}`}><SelectField label="Channel" value={assignment.channel_id} options={mode.channels.map((candidate) => ({ value: candidate.id, label: candidate.attribute }))} onChange={(channel_id) => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, assignments: candidate.assignments.map((item, itemIndex) => itemIndex === index ? { ...item, channel_id } : item) } : candidate) })}/><NumberField label="Active raw" min={0} max={channel ? maxRaw(channel.resolution) : 0xffffffff} value={assignment.active_raw} onChange={(event) => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, assignments: candidate.assignments.map((item, itemIndex) => itemIndex === index ? { ...item, active_raw: Number(event.target.value) } : item) } : candidate) })}/><NumberField label="Inactive raw" min={0} max={channel ? maxRaw(channel.resolution) : 0xffffffff} value={assignment.inactive_raw} onChange={(event) => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, assignments: candidate.assignments.map((item, itemIndex) => itemIndex === index ? { ...item, inactive_raw: Number(event.target.value) } : item) } : candidate) })}/><Button iconOnly aria-label="Remove control assignment" onClick={() => onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, assignments: candidate.assignments.filter((_, itemIndex) => itemIndex !== index) } : candidate) })}>×</Button></div>; })}<Button disabled={!mode.channels.length} onClick={() => { const channel = mode.channels[0]; onChange({ ...mode, control_actions: mode.control_actions.map((candidate) => candidate.id === action.id ? { ...candidate, assignments: [...candidate.assignments, { channel_id: channel.id, active_raw: maxRaw(channel.resolution), inactive_raw: 0 }] } : candidate) }); }}>Add channel assignment</Button></div></article>)}</section>;
}

function removeColorChannel(system: HeadColorSystem, channelId: string): HeadColorSystem | null {
  if (system.system.type === "additive") return { ...system, system: { ...system.system, emitters: system.system.emitters.filter((emitter) => emitter.channel_id !== channelId) } };
  if (system.system.type === "subtractive" && [system.system.cyan_channel_id, system.system.magenta_channel_id, system.system.yellow_channel_id].includes(channelId)) return null;
  if (system.system.type === "discrete_wheel" && system.system.channel_id === channelId) return null;
  return system;
}

const identityColorCorrectionMatrix = (): HeadColorSystem["correction_matrix"] => [
  [1, 0, 0],
  [0, 1, 0],
  [0, 0, 1],
];

export function replaceHeadColorSystem(
  systems: HeadColorSystem[],
  headId: string,
  system: ColorSystem | null,
): HeadColorSystem[] {
  if (!system) return systems.filter((candidate) => candidate.head_id !== headId);
  const existing = systems.find((candidate) => candidate.head_id === headId);
  return [
    ...systems.filter((candidate) => candidate.head_id !== headId),
    existing
      ? { ...existing, system }
      : { head_id: headId, correction_matrix: identityColorCorrectionMatrix(), system },
  ];
}

function ColorEditor({ mode, onChange }: { mode: FixtureMode; onChange: (mode: FixtureMode) => void }) {
  const setSystem = (headId: string, system: ColorSystem | null) => onChange(reconcileColorSystemHighlightDefaults(
    mode,
    replaceHeadColorSystem(mode.color_systems, headId, system),
  ));
  const setCorrection = (headId: string, row: number, column: number, value: number) => onChange(reconcileColorSystemHighlightDefaults(
    mode,
    mode.color_systems.map((candidate) => candidate.head_id === headId
      ? {
          ...candidate,
          correction_matrix: candidate.correction_matrix.map((values, rowIndex) => values.map(
            (entry, columnIndex) => rowIndex === row && columnIndex === column ? value : entry,
          )) as HeadColorSystem["correction_matrix"],
        }
      : candidate),
  ));
  return <div className="fixture-color-editor"><p>Abstract XYZ color is resolved through one color system per logical head. Direct emitter channels remain available to the programmer.</p>{mode.heads.map((head) => {
    const record = mode.color_systems.find((candidate) => candidate.head_id === head.id);
    const channels = mode.channels.filter((channel) => channel.head_id === head.id);
    const options = channels.map((channel) => ({ value: channel.id, label: channel.attribute }));
    const type = record?.system.type ?? "none";
    return <section key={head.id}><header><h3>{head.name}</h3><SelectField label="Color system" value={type} options={[{ value: "none", label: "No abstraction" }, { value: "additive", label: "Additive emitters" }, { value: "subtractive", label: "Subtractive CMY" }, { value: "discrete_wheel", label: "Discrete color wheel" }]} onChange={(next) => {
      if (next === "none") return setSystem(head.id, null);
      const first = channels[0]?.id ?? "";
      if (next === "additive") return setSystem(head.id, { type: next, emitters: [] });
      if (next === "subtractive") return setSystem(head.id, { type: next, cyan_channel_id: first, magenta_channel_id: first, yellow_channel_id: first });
      setSystem(head.id, { type: next, channel_id: first, slots: [] });
    }}/></header>
      {record && <fieldset className="color-correction-matrix"><legend>XYZ correction matrix</legend><p>Applied before calibrated color matching. Identity leaves requested XYZ unchanged.</p><FormLayout columns={3}>{record.correction_matrix.flatMap((row, rowIndex) => row.map((value, columnIndex) => <NumberField key={`${rowIndex}-${columnIndex}`} aria-label={`${head.name} correction row ${rowIndex + 1} column ${columnIndex + 1}`} allowDecimal step={0.001} value={value} onChange={(event) => setCorrection(head.id, rowIndex, columnIndex, Number(event.target.value))}/>))}</FormLayout></fieldset>}
      {record?.system.type === "additive" && <AdditiveColorEditor system={record.system} channels={channels} options={options} onChange={(system) => setSystem(head.id, system)}/>} 
      {record?.system.type === "subtractive" && <SubtractiveColorEditor system={record.system} options={options} onChange={(system) => setSystem(head.id, system)}/>} 
      {record?.system.type === "discrete_wheel" && <DiscreteColorEditor system={record.system} options={options} onChange={(system) => setSystem(head.id, system)}/>} 
    </section>;
  })}</div>;
}

function AdditiveColorEditor({ system, channels, options, onChange }: { system: Extract<ColorSystem, { type: "additive" }>; channels: FixtureChannel[]; options: Array<{ value: string; label: string }>; onChange: (system: Extract<ColorSystem, { type: "additive" }>) => void }) {
  const setEmitter = (index: number, patch: Partial<(typeof system.emitters)[number]>) => onChange({ ...system, emitters: system.emitters.map((candidate, itemIndex) => itemIndex === index ? { ...candidate, ...patch } : candidate) });
  return <div className="color-emitter-list">{system.emitters.map((emitter, index) => <article key={`${emitter.channel_id}-${index}`}><SelectField label="Emitter channel" value={emitter.channel_id} options={options} onChange={(channel_id) => setEmitter(index, { channel_id })}/><TextField label="Emitter name" value={emitter.name} onChange={(event) => setEmitter(index, { name: event.target.value })}/>{(["x", "y", "z"] as const).map((axis) => <NumberField key={axis} label={`Measured XYZ ${axis.toUpperCase()}`} allowDecimal min={0} value={emitter.xyz[axis]} onChange={(event) => setEmitter(index, { xyz: { ...emitter.xyz, [axis]: Number(event.target.value) } })}/>) }<XyyFields xyz={emitter.xyz} onChange={(xyz) => setEmitter(index, { xyz })}/><NumberField label="Maximum level" allowDecimal min={0} max={1} step={0.01} value={emitter.maximum_level} onChange={(event) => setEmitter(index, { maximum_level: Number(event.target.value) })}/><NumberField label="Response curve" allowDecimal min={0.01} step={0.01} value={emitter.response_curve} onChange={(event) => setEmitter(index, { response_curve: Number(event.target.value) })}/><CheckboxField label="Participates in visible color matching" checked={emitter.visible} onChange={(event) => setEmitter(index, { visible: event.target.checked })}/><Button onClick={() => onChange({ ...system, emitters: system.emitters.filter((_, itemIndex) => itemIndex !== index) })}>Remove emitter</Button></article>)}<Button disabled={!channels.length} onClick={() => { const channel = channels[0]; if (channel) onChange({ ...system, emitters: [...system.emitters, { channel_id: channel.id, name: channel.attribute, xyz: { x: 0.33, y: 0.33, z: 0.34 }, maximum_level: 1, response_curve: 1, visible: !channel.attribute.endsWith("uv") }] }); }}>Add emitter</Button></div>;
}

function XyyFields({ xyz, onChange }: { xyz: { x: number; y: number; z: number }; onChange: (xyz: { x: number; y: number; z: number }) => void }) {
  const value = xyzToXyy(xyz);
  const set = (patch: Partial<typeof value>) => onChange(xyyToXyz({ ...value, ...patch }));
  return <details className="xyy-entry"><summary>Enter measured xyY</summary><NumberField label="Chromaticity x" allowDecimal min={0} max={1} step={0.0001} value={value.x} onChange={(event) => set({ x: Number(event.target.value) })}/><NumberField label="Chromaticity y" allowDecimal min={0} max={1} step={0.0001} value={value.y} onChange={(event) => set({ y: Number(event.target.value) })}/><NumberField label="Luminance Y" allowDecimal min={0} value={value.luminance} onChange={(event) => set({ luminance: Number(event.target.value) })}/></details>;
}

function SubtractiveColorEditor({ system, options, onChange }: { system: Extract<ColorSystem, { type: "subtractive" }>; options: Array<{ value: string; label: string }>; onChange: (system: Extract<ColorSystem, { type: "subtractive" }>) => void }) {
  return <FormLayout columns={3}>{(["cyan_channel_id", "magenta_channel_id", "yellow_channel_id"] as const).map((key) => <SelectField key={key} label={key.split("_")[0][0].toUpperCase() + key.split("_")[0].slice(1)} value={system[key]} options={options} onChange={(value) => onChange({ ...system, [key]: value })}/>)}</FormLayout>;
}

function DiscreteColorEditor({ system, options, onChange }: { system: Extract<ColorSystem, { type: "discrete_wheel" }>; options: Array<{ value: string; label: string }>; onChange: (system: Extract<ColorSystem, { type: "discrete_wheel" }>) => void }) {
  const setSlot = (index: number, patch: Partial<(typeof system.slots)[number]>) => onChange({ ...system, slots: system.slots.map((slot, itemIndex) => itemIndex === index ? { ...slot, ...patch } : slot) });
  return <div className="color-wheel-editor">
    <SelectField label="Wheel channel" value={system.channel_id} options={options} onChange={(channel_id) => onChange({ ...system, channel_id })}/>
    {system.slots.map((slot, index) => <article key={`${slot.semantic_id}-${index}`}>
      <TextField label="Portable color ID" value={slot.semantic_id} onChange={(event) => setSlot(index, { semantic_id: event.target.value })}/>
      <TextField label="Fixture label" value={slot.label} onChange={(event) => setSlot(index, { label: event.target.value })}/>
      <NumberField label="DMX from" min={0} value={slot.dmx_from} onChange={(event) => setSlot(index, { dmx_from: Number(event.target.value) })}/>
      <NumberField label="DMX to" min={0} value={slot.dmx_to} onChange={(event) => setSlot(index, { dmx_to: Number(event.target.value) })}/>
      <CheckboxField label="Measured XYZ available" checked={Boolean(slot.measured_xyz)} onChange={(event) => setSlot(index, { measured_xyz: event.target.checked ? { x: 0.33, y: 0.33, z: 0.34 } : null })}/>
      {slot.measured_xyz && <>{(["x", "y", "z"] as const).map((axis) => <NumberField key={axis} label={`Measured XYZ ${axis.toUpperCase()}`} allowDecimal min={0} value={slot.measured_xyz![axis]} onChange={(event) => setSlot(index, { measured_xyz: { ...slot.measured_xyz!, [axis]: Number(event.target.value) } })}/>)}<XyyFields xyz={slot.measured_xyz} onChange={(measured_xyz) => setSlot(index, { measured_xyz })}/></>}
      <Button onClick={() => onChange({ ...system, slots: system.slots.filter((_, itemIndex) => itemIndex !== index) })}>Remove slot</Button>
    </article>)}
    <Button onClick={() => onChange({ ...system, slots: [...system.slots, { semantic_id: "color.open", label: "Open", dmx_from: 0, dmx_to: 0, measured_xyz: null }] })}>Add color slot</Button>
  </div>;
}

function GeometryEditor({ mode, onChange }: { mode: FixtureMode; onChange: (mode: FixtureMode) => void }) {
  const [selected, setSelected] = useState<{ type: "node" | "emitter"; id: string } | null>(() => mode.geometry.nodes[0] ? { type: "node", id: mode.geometry.nodes[0].id } : null);
  const setNode = (node: GeometryNode) => onChange({ ...mode, geometry: { ...mode.geometry, nodes: mode.geometry.nodes.map((candidate) => candidate.id === node.id ? node : candidate) } });
  const setEmitter = (emitter: GeometryEmitter) => onChange({ ...mode, geometry: { ...mode.geometry, emitters: mode.geometry.emitters.map((candidate) => candidate.id === emitter.id ? emitter : candidate) } });
  const selectedNode = selected?.type === "node" ? mode.geometry.nodes.find((node) => node.id === selected.id) : null;
  const selectedEmitter = selected?.type === "emitter" ? mode.geometry.emitters.find((emitter) => emitter.id === selected.id) : null;
  const depth = (node: GeometryNode) => {
    let result = 0; let parent = node.parent_id;
    while (parent && result < mode.geometry.nodes.length) { result += 1; parent = mode.geometry.nodes.find((candidate) => candidate.id === parent)?.parent_id ?? null; }
    return result;
  };
  const useTemplate = (template: GeometryTemplateName) => {
    const geometry = geometryTemplate(template, mode.heads.map((head) => head.id));
    onChange({ ...mode, geometry });
    setSelected(geometry.nodes[0] ? { type: "node", id: geometry.nodes[0].id } : null);
  };
  const addNode = () => {
    const node: GeometryNode = { id: uuid(), name: `Part ${mode.geometry.nodes.length + 1}`, parent_id: selectedNode?.id ?? mode.geometry.nodes[0]?.id ?? null, transform: { translation: { x: 0, y: 0, z: 0 }, rotation_degrees: { x: 0, y: 0, z: 0 }, scale: { x: 1, y: 1, z: 1 } }, pivot: { x: 0, y: 0, z: 0 }, glb_node: null, motion: null };
    onChange({ ...mode, geometry: { ...mode.geometry, nodes: [...mode.geometry.nodes, node] } });
    setSelected({ type: "node", id: node.id });
  };
  const addEmitter = () => {
    const node = selectedNode ?? mode.geometry.nodes[0]; const head = mode.heads[0];
    if (!node || !head) return;
    const emitter: GeometryEmitter = { id: uuid(), name: `Emitter ${mode.geometry.emitters.length + 1}`, node_id: node.id, head_id: head.id, origin: { x: 0, y: 0, z: 0 }, orientation_degrees: { x: 0, y: 0, z: 0 }, beam_angle_degrees: 20, field_angle_degrees: 24, feather: 0, focus: 1, layout: { type: "point" } };
    onChange({ ...mode, geometry: { ...mode.geometry, emitters: [...mode.geometry.emitters, emitter] } });
    setSelected({ type: "emitter", id: emitter.id });
  };
  return <div className="fixture-geometry-editor"><section className="geometry-templates"><h3>Geometry templates</h3>{([['fixed','Fixed fixture'],['moving_head','Moving head'],['bar','Bar'],['matrix','Matrix'],['shared_pan_multi_head','Shared-pan multi-head']] as const).map(([id, label]) => <Button key={id} onClick={() => useTemplate(id)}>{label}</Button>)}</section><div className="geometry-workspace"><aside><header><h3>Parts and emitters</h3><Button onClick={addNode}>Add part</Button><Button onClick={addEmitter}>Add emitter</Button></header><div className="geometry-tree" role="tree">{mode.geometry.nodes.map((node) => <Button role="treeitem" aria-level={depth(node) + 1} key={node.id} active={selected?.type === "node" && selected.id === node.id} style={{ paddingLeft: `${12 + depth(node) * 18}px` }} onClick={() => setSelected({ type: "node", id: node.id })}>◇ {node.name}</Button>)}{mode.geometry.emitters.map((emitter) => <Button role="treeitem" key={emitter.id} active={selected?.type === "emitter" && selected.id === emitter.id} onClick={() => setSelected({ type: "emitter", id: emitter.id })}>⌁ {emitter.name}</Button>)}</div></aside><section className="geometry-properties">{selectedNode && <GeometryNodeForm node={selectedNode} nodes={mode.geometry.nodes} onChange={setNode} onRemove={() => { if (mode.geometry.nodes.some((candidate) => candidate.parent_id === selectedNode.id) || mode.geometry.emitters.some((emitter) => emitter.node_id === selectedNode.id)) return; onChange({ ...mode, geometry: { nodes: mode.geometry.nodes.filter((candidate) => candidate.id !== selectedNode.id), emitters: mode.geometry.emitters } }); setSelected(null); }}/>} {selectedEmitter && <GeometryEmitterForm emitter={selectedEmitter} mode={mode} onChange={setEmitter} onRemove={() => { onChange({ ...mode, geometry: { ...mode.geometry, emitters: mode.geometry.emitters.filter((candidate) => candidate.id !== selectedEmitter.id) } }); setSelected(null); }}/>}</section><GeometryPreview mode={mode}/></div></div>;
}

function GeometryNodeForm({ node, nodes, onChange, onRemove }: { node: GeometryNode; nodes: GeometryNode[]; onChange: (node: GeometryNode) => void; onRemove: () => void }) {
  return <div><h3>Part properties</h3><TextField label="Part name" value={node.name} onChange={(event) => onChange({ ...node, name: event.target.value })}/><SelectField label="Parent part" value={node.parent_id ?? ""} options={[{ value: "", label: "Root" }, ...nodes.filter((candidate) => candidate.id !== node.id).map((candidate) => ({ value: candidate.id, label: candidate.name }))]} onChange={(parent_id) => onChange({ ...node, parent_id: parent_id || null })}/><TextField label="GLB node binding" value={node.glb_node ?? ""} onChange={(event) => onChange({ ...node, glb_node: event.target.value || null })}/><VectorFields label="Translation" value={node.transform.translation} onChange={(translation) => onChange({ ...node, transform: { ...node.transform, translation } })}/><VectorFields label="Base rotation °" value={node.transform.rotation_degrees} onChange={(rotation_degrees) => onChange({ ...node, transform: { ...node.transform, rotation_degrees } })}/><VectorFields label="Scale" value={node.transform.scale} onChange={(scale) => onChange({ ...node, transform: { ...node.transform, scale } })}/><VectorFields label="Pivot" value={node.pivot} onChange={(pivot) => onChange({ ...node, pivot })}/><CheckboxField label="Attribute-driven motion" checked={Boolean(node.motion)} onChange={(event) => onChange({ ...node, motion: event.target.checked ? { attribute: "pan", kind: "rotation", axis: { x: 0, y: 1, z: 0 }, physical_min: -270, physical_max: 270 } : null })}/>{node.motion && <div className="geometry-motion"><TextField label="Motion attribute" list="fixture-attribute-registry" value={node.motion.attribute} onChange={(event) => onChange({ ...node, motion: { ...node.motion!, attribute: event.target.value } })}/><SelectField label="Motion kind" value={node.motion.kind} options={[{ value: "rotation", label: "Rotation" }, { value: "translation", label: "Translation" }]} onChange={(kind) => onChange({ ...node, motion: { ...node.motion!, kind } })}/><VectorFields label="Motion axis" value={node.motion.axis} onChange={(axis) => onChange({ ...node, motion: { ...node.motion!, axis } })}/><NumberField label="Physical minimum" allowDecimal value={node.motion.physical_min} onChange={(event) => onChange({ ...node, motion: { ...node.motion!, physical_min: Number(event.target.value) } })}/><NumberField label="Physical maximum" allowDecimal value={node.motion.physical_max} onChange={(event) => onChange({ ...node, motion: { ...node.motion!, physical_max: Number(event.target.value) } })}/></div>}<Button variant="danger" disabled={nodes.length === 1} onClick={onRemove}>Remove part</Button></div>;
}

function GeometryEmitterForm({ emitter, mode, onChange, onRemove }: { emitter: GeometryEmitter; mode: FixtureMode; onChange: (emitter: GeometryEmitter) => void; onRemove: () => void }) {
  const layout = emitter.layout;
  return <div><h3>Emitter properties</h3>
    <TextField label="Emitter name" value={emitter.name} onChange={(event) => onChange({ ...emitter, name: event.target.value })}/>
    <SelectField label="Geometry part" value={emitter.node_id} options={mode.geometry.nodes.map((node) => ({ value: node.id, label: node.name }))} onChange={(node_id) => onChange({ ...emitter, node_id })}/>
    <SelectField label="Logical head" value={emitter.head_id} options={mode.heads.map((head) => ({ value: head.id, label: head.name }))} onChange={(head_id) => onChange({ ...emitter, head_id })}/>
    <VectorFields label="Origin" value={emitter.origin} onChange={(origin) => onChange({ ...emitter, origin })}/>
    <VectorFields label="Orientation °" value={emitter.orientation_degrees} onChange={(orientation_degrees) => onChange({ ...emitter, orientation_degrees })}/>
    <NumberField label="Beam angle °" allowDecimal min={0} value={emitter.beam_angle_degrees} onChange={(event) => onChange({ ...emitter, beam_angle_degrees: Number(event.target.value) })}/>
    <NumberField label="Field angle °" allowDecimal min={0} value={emitter.field_angle_degrees} onChange={(event) => onChange({ ...emitter, field_angle_degrees: Number(event.target.value) })}/>
    <NumberField label="Feather" allowDecimal min={0} max={1} step={0.01} value={emitter.feather} onChange={(event) => onChange({ ...emitter, feather: Number(event.target.value) })}/>
    <NumberField label="Focus" allowDecimal min={0} max={1} step={0.01} value={emitter.focus} onChange={(event) => onChange({ ...emitter, focus: Number(event.target.value) })}/>
    <SelectField label="Source layout" value={layout.type} options={[{ value: "point", label: "Point" }, { value: "matrix", label: "Matrix" }, { value: "ring", label: "Ring" }, { value: "strip", label: "Strip" }, { value: "explicit_pixels", label: "Explicit pixels" }]} onChange={(type) => onChange({ ...emitter, layout: type === "point" ? { type } : type === "matrix" ? { type, columns: 4, rows: 4, spacing: { x: 50, y: 50, z: 0 } } : type === "ring" ? { type, count: 12, radius_millimetres: 100 } : type === "strip" ? { type, count: 8, spacing_millimetres: 50 } : { type, positions: [] } })}/>
    {layout.type === "matrix" && <><NumberField label="Matrix columns" min={1} value={layout.columns} onChange={(event) => onChange({ ...emitter, layout: { ...layout, columns: Number(event.target.value) } })}/><NumberField label="Matrix rows" min={1} value={layout.rows} onChange={(event) => onChange({ ...emitter, layout: { ...layout, rows: Number(event.target.value) } })}/><VectorFields label="Matrix spacing (mm)" value={layout.spacing} onChange={(spacing) => onChange({ ...emitter, layout: { ...layout, spacing } })}/></>}
    {layout.type === "ring" && <><NumberField label="Ring source count" min={1} value={layout.count} onChange={(event) => onChange({ ...emitter, layout: { ...layout, count: Number(event.target.value) } })}/><NumberField label="Ring radius (mm)" allowDecimal min={0} value={layout.radius_millimetres} onChange={(event) => onChange({ ...emitter, layout: { ...layout, radius_millimetres: Number(event.target.value) } })}/></>}
    {layout.type === "strip" && <><NumberField label="Strip source count" min={1} value={layout.count} onChange={(event) => onChange({ ...emitter, layout: { ...layout, count: Number(event.target.value) } })}/><NumberField label="Strip spacing (mm)" allowDecimal min={0} value={layout.spacing_millimetres} onChange={(event) => onChange({ ...emitter, layout: { ...layout, spacing_millimetres: Number(event.target.value) } })}/></>}
    {layout.type === "explicit_pixels" && <div className="geometry-explicit-pixels">{layout.positions.map((position, index) => <article key={index}><VectorFields label={`Pixel ${index + 1} position`} value={position} onChange={(next) => onChange({ ...emitter, layout: { ...layout, positions: layout.positions.map((candidate, itemIndex) => itemIndex === index ? next : candidate) } })}/><Button onClick={() => onChange({ ...emitter, layout: { ...layout, positions: layout.positions.filter((_, itemIndex) => itemIndex !== index) } })}>Remove pixel</Button></article>)}<Button onClick={() => onChange({ ...emitter, layout: { ...layout, positions: [...layout.positions, { x: 0, y: 0, z: 0 }] } })}>Add pixel position</Button></div>}
    <Button variant="danger" onClick={onRemove}>Remove emitter</Button>
  </div>;
}

function VectorFields({ label, value, onChange }: { label: string; value: { x: number; y: number; z: number }; onChange: (value: { x: number; y: number; z: number }) => void }) {
  return <fieldset className="geometry-vector"><legend>{label}</legend>{(["x", "y", "z"] as const).map((axis) => <NumberField key={axis} label={axis.toUpperCase()} allowDecimal value={value[axis]} onChange={(event) => onChange({ ...value, [axis]: Number(event.target.value) })}/>)}</fieldset>;
}

function GeometryPreview({ mode }: { mode: FixtureMode }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const container = host.current;
    if (!container || typeof WebGLRenderingContext === "undefined") return;
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x080b0e);
    scene.add(new THREE.HemisphereLight(0xbfe9ff, 0x101820, 2));
    scene.add(buildFixtureProfileGeometryPreview(mode));
    const camera = new THREE.PerspectiveCamera(45, 1, .01, 100);
    camera.position.set(3.5, 2.5, 6.5);
    camera.lookAt(0, -1.5, 0);
    let renderer: THREE.WebGLRenderer;
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    } catch {
      disposeScene(scene);
      return;
    }
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    container.replaceChildren(renderer.domElement);
    const render = () => {
      const width = Math.max(260, container.clientWidth);
      const height = Math.max(260, container.clientHeight);
      renderer.setSize(width, height, false);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
      renderer.render(scene, camera);
    };
    render();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(render);
    observer?.observe(container);
    return () => {
      observer?.disconnect();
      disposeScene(scene);
      renderer.dispose();
      renderer.domElement.remove();
    };
  }, [mode]);
  return <section className="geometry-live-preview" aria-label="Live geometry preview"><h3>Live 3D preview</h3><div ref={host} className="geometry-preview-stage" role="img" aria-label="Fixture geometry hierarchy and beams in three dimensions"/><small>{mode.geometry.nodes.length} parts · {mode.geometry.emitters.length} emitters. Preview uses the Stage renderer's hierarchy, transforms, source layouts, and beam angles.</small></section>;
}
