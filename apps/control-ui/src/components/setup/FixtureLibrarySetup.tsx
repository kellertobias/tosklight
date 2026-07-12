import { useMemo, useRef, useState } from "react";
import JSZip from "jszip";
import { useServer } from "../../api/ServerContext";
import type { FixtureDefinition } from "../../api/types";
import { SearchBar } from "../common/SearchBar";
import { groupFixtureFamilies } from "./patchUtils";
import { createPortal } from "react-dom";

export const FIXTURE_TYPES = ["dimmer", "fogger", "profile", "wash", "wash mover", "spot mover", "beam mover", "strobe", "media server", "pixel fixture", "other"];

export function blankDefinition(): FixtureDefinition {
  return {
    schema_version: 1, id: crypto.randomUUID(), revision: 1, manufacturer: "", device_type: "other", name: "", model: "", mode: "Standard", footprint: 1,
    heads: [{ index: 0, name: "Main", shared: true, parameters: [{ attribute: "intensity", components: [{ offset: 0, byte_order: "msb_first" }], default: 0, virtual_dimmer: false, capabilities: [] }]}],
    color_calibration: null, physical: {}, model_asset: null, icon_asset: null, hazardous: false, direct_control_protocols: [], signal_loss_policy: { type: "hold_last" }, safe_values: {},
  };
}

const attrName = (value: string) => value.replace(/([a-z])([A-Z])/g, "$1.$2").replace(/\s+/g, ".").toLowerCase();

type HeadDraft = { name: string; master: boolean; channels: string };

function parseDmx(value: string | null | undefined, fallback: number) {
  const parsed = Number((value ?? "").split("/")[0]);
  return Number.isFinite(parsed) ? Math.max(0, Math.min(255, parsed)) : fallback;
}

function parseDefault(value: string | null | undefined, bytes: number) {
  const parsed = Number((value ?? "").split("/")[0]);
  return Number.isFinite(parsed) ? Math.max(0, Math.min(1, parsed / (2 ** (bytes * 8) - 1))) : 0;
}

export function parseHeadDrafts(heads: HeadDraft[]) {
  let offset = 0;
  const splitChannels = (value: string) => {
    const result: string[] = []; let depth = 0; let start = 0;
    for (let index = 0; index < value.length; index++) {
      if (value[index] === "{" || value[index] === "[") depth++;
      if (value[index] === "}" || value[index] === "]") depth--;
      if (value[index] === "," && depth === 0) { result.push(value.slice(start, index)); start = index + 1; }
    }
    result.push(value.slice(start)); return result;
  };
  const parsedHeads = heads.map((head, headIndex) => ({
    index: headIndex,
    name: head.name.trim() || `Head ${headIndex + 1}`,
    shared: head.master,
    parameters: splitChannels(head.channels).map((raw) => raw.trim()).filter(Boolean).map((raw) => {
      const capabilitiesText = raw.match(/\{(.+)\}/)?.[1] ?? "";
      const rangeText = raw.match(/\[(-?[\d.]+),(-?[\d.]+)(?:,([^\]]+))?\]/);
      const clean = raw.replace(/\{.+\}/, "").replace(/\[.+\]/, "");
      const [attribute, resolution] = clean.split(":");
      const bytes = resolution === "16" ? 2 : 1;
      const start = offset;
      offset += bytes;
      return {
        attribute: attrName(attribute),
        components: Array.from({ length: bytes }, (_, component) => ({ offset: start + component, byte_order: "msb_first" as const })),
        default: attribute.toLowerCase().includes("shutter") ? 1 : 0,
        virtual_dimmer: false,
        metadata: { physical_min: rangeText ? Number(rangeText[1]) : 0, physical_max: rangeText ? Number(rangeText[2]) : 1, unit: rangeText?.[3] ?? null, invert: false, wrap: attribute.toLowerCase().includes("pan"), curve: "linear" },
        capabilities: capabilitiesText.split("|").filter(Boolean).map((entry) => { const [name, range = "0-255"] = entry.split("="); const [from, to = from] = range.split("-").map(Number); return { name: name.trim(), dmx_from: from, dmx_to: to, preset_family: attribute.toLowerCase().includes("gobo") ? "gobo" : null }; }),
      };
    }),
  }));
  return { heads: parsedHeads, footprint: offset };
}

function readFileAsDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = () => reject(reader.error); reader.readAsDataURL(file); });
}

export async function importGdtfData(data: ArrayBuffer | Uint8Array, fileName: string): Promise<FixtureDefinition[]> {
  const zip = await JSZip.loadAsync(data);
  const description = zip.file(/(^|\/)description\.xml$/i)[0];
  if (!description) throw new Error("The GDTF archive has no description.xml");
  const xml = new DOMParser().parseFromString(await description.async("text"), "application/xml");
  const error = xml.querySelector("parsererror");
  if (error) throw new Error("The GDTF description.xml is invalid");
  const root = xml.querySelector("FixtureType");
  if (!root) throw new Error("The GDTF file contains no FixtureType");
  const manufacturer = root.getAttribute("Manufacturer") || "Unknown";
  const model = root.getAttribute("ShortName") || root.getAttribute("Name") || fileName.replace(/\.gdtf$/i, "");
  const modelEntry = Object.values(zip.files).find((entry) => !entry.dir && entry.name.toLowerCase().endsWith(".glb"));
  const modelAsset = modelEntry ? `data:model/gltf-binary;base64,${await modelEntry.async("base64")}` : null;
  const emitters = [...root.querySelectorAll("PhysicalDescriptions > Emitters > Emitter")].map((emitter) => {
    const [x = 0, y = 0, luminance = 1] = (emitter.getAttribute("Color") ?? "").split(",").map(Number);
    return { name: emitter.getAttribute("Name") || `Emitter ${x},${y}`, xyz: { x: y > 0 ? x * luminance / y : 0, y: luminance, z: y > 0 ? (1 - x - y) * luminance / y : 0 }, limit: 1 };
  }).filter((emitter) => emitter.xyz.x >= 0 && emitter.xyz.y >= 0 && emitter.xyz.z >= 0);
  const colorCalibration = emitters.length >= 3 ? { emitters, correction_matrix: [[1,0,0],[0,1,0],[0,0,1]] } : null;
  const modes = [...xml.querySelectorAll("DMXModes > DMXMode")];
  return (modes.length ? modes : [root]).map((mode, modeIndex) => {
    const channels = [...mode.querySelectorAll("DMXChannels > DMXChannel")];
    const headNames = [...new Set(channels.map((channel) => channel.getAttribute("Geometry") || "Main"))];
    let footprint = 1;
    const heads = headNames.map((headName, headIndex) => ({
      index: headIndex, name: headName, shared: headNames.length === 1 || /master|base/i.test(headName),
      parameters: channels.filter((channel) => (channel.getAttribute("Geometry") || "Main") === headName).map((channel) => {
        const offsets = (channel.getAttribute("Offset") || "1").split(",").map(Number).filter(Number.isFinite).map((value) => Math.max(0, value - 1));
        footprint = Math.max(footprint, ...offsets.map((value) => value + 1));
        const logical = channel.querySelector("LogicalChannel");
        const fn = logical?.querySelector("ChannelFunction");
        const attribute = attrName(logical?.getAttribute("Attribute") || fn?.getAttribute("Attribute") || channel.getAttribute("Name") || `channel.${offsets[0] + 1}`);
        const physicalFrom = Number(fn?.getAttribute("PhysicalFrom") ?? 0);
        const physicalTo = Number(fn?.getAttribute("PhysicalTo") ?? 1);
        return { attribute, components: offsets.map((offset) => ({ offset, byte_order: "msb_first" as const })), default: parseDefault(channel.getAttribute("Default") || fn?.getAttribute("Default"), offsets.length), virtual_dimmer: false, metadata: { physical_min: physicalFrom, physical_max: physicalTo === physicalFrom ? physicalFrom + 1 : physicalTo, unit: null, invert: false, wrap: attribute.includes("pan"), curve: "linear" }, capabilities: [...channel.querySelectorAll("ChannelSet")].map((set, index, all) => ({ name: set.getAttribute("Name") || attribute, dmx_from: parseDmx(set.getAttribute("DMXFrom"), Math.round(index * 256 / all.length)), dmx_to: parseDmx(set.getAttribute("DMXTo"), Math.round((index + 1) * 256 / all.length) - 1), preset_family: attribute.includes("gobo") ? "gobo" : null })) };
      }),
    }));
    const allAttributes = heads.flatMap((head) => head.parameters.map((parameter) => parameter.attribute)).join(" ");
    const classify = /fog|haze/.test(`${model} ${allAttributes}`.toLowerCase()) ? "fogger" : /media/.test(model.toLowerCase()) ? "media server" : /pan|tilt/.test(allAttributes) && /wash|color/.test(`${model} ${allAttributes}`.toLowerCase()) ? "wash mover" : /pan|tilt/.test(allAttributes) ? "spot mover" : /wash|color/.test(`${model} ${allAttributes}`.toLowerCase()) ? "wash" : /shutter|gobo|focus/.test(allAttributes) ? "profile" : "other";
    const pan = heads.flatMap((head) => head.parameters).find((parameter) => parameter.attribute.includes("pan"))?.metadata;
    const tilt = heads.flatMap((head) => head.parameters).find((parameter) => parameter.attribute.includes("tilt"))?.metadata;
    return { ...blankDefinition(), id: crypto.randomUUID(), manufacturer, device_type: classify, name: model, model, model_asset: modelAsset, color_calibration: colorCalibration, mode: mode.getAttribute("Name") || `Mode ${modeIndex + 1}`, footprint, heads, physical: { pan_range_degrees: pan ? Math.abs(pan.physical_max - pan.physical_min) : null, tilt_range_degrees: tilt ? Math.abs(tilt.physical_max - tilt.physical_min) : null } };
  });
}

export async function importGdtf(file: File) {
  return importGdtfData(await file.arrayBuffer(), file.name);
}

export function FixtureLibrarySetup() {
  const server = useServer();
  const [draft, setDraft] = useState(blankDefinition);
  const [heads, setHeads] = useState<HeadDraft[]>([{ name: "Main", master: true, channels: "intensity,pan:16[-270,270,deg],tilt:16[-135,135,deg]" }]);
  const [emitters, setEmitters] = useState("");
  const [busy, setBusy] = useState(false);
  const [selectedFamilyKey, setSelectedFamilyKey] = useState("");
  const [selectedModeKey, setSelectedModeKey] = useState("");
  const [modal, setModal] = useState<"editor" | "import" | null>(null);
  const [query, setQuery] = useState(""); const [submitted, setSubmitted] = useState(""); const [typeFilter, setTypeFilter] = useState(""); const [manufacturer, setManufacturer] = useState("");
  const fileRef = useRef<HTMLInputElement>(null);
  const fixtureTypes = useMemo(() => [...new Set(server.fixtureLibrary.map((item) => item.device_type || "other"))].sort(), [server.fixtureLibrary]);
  const manufacturers = useMemo(() => [...new Set(server.fixtureLibrary.map((item) => item.manufacturer))].sort(), [server.fixtureLibrary]);
  const libraryFamilies = useMemo(() => groupFixtureFamilies(server.fixtureLibrary.filter((item) => { const needle = submitted.toLowerCase().trim(); return (!manufacturer || item.manufacturer === manufacturer) && (!typeFilter || item.device_type === typeFilter) && (!needle || `${item.manufacturer} ${item.name} ${item.model} ${item.mode} ${item.device_type}`.toLowerCase().includes(needle)); })), [server.fixtureLibrary, submitted, manufacturer, typeFilter]);
  const selectedLibraryFamily = libraryFamilies.find((family) => family.key === selectedFamilyKey) ?? libraryFamilies[0] ?? null;
  const selectedMode = selectedLibraryFamily?.modes.find((mode) => `${mode.id}:${mode.revision}` === selectedModeKey) ?? selectedLibraryFamily?.modes[0] ?? null;
  const edit = (definition: FixtureDefinition) => {
    setDraft({ ...definition, revision: definition.revision + 1 });
    setHeads(definition.heads.map((head) => ({ name: head.name, master: head.shared, channels: head.parameters.map((parameter) => {
      const resolution = parameter.components.length > 1 ? ":16" : "";
      const metadata = parameter.metadata;
      const range = metadata && (metadata.physical_min !== 0 || metadata.physical_max !== 1) ? `[${metadata.physical_min},${metadata.physical_max}${metadata.unit ? `,${metadata.unit}` : ""}]` : "";
      const capabilities = parameter.capabilities.length ? `{${parameter.capabilities.map((capability) => `${capability.name}=${capability.dmx_from}-${capability.dmx_to}`).join("|")}}` : "";
      return `${parameter.attribute}${resolution}${range}${capabilities}`;
    }).join(",") })));
    const calibration = definition.color_calibration as { emitters?: Array<{ name: string; xyz: { x: number; y: number; z: number }; limit: number }> } | null;
    setEmitters(calibration?.emitters?.map((emitter) => `${emitter.name},${emitter.xyz.x},${emitter.xyz.y},${emitter.xyz.z},${emitter.limit}`).join("\n") ?? "");
    setModal("editor");
  };
  const save = async () => {
    const parsed = parseHeadDrafts(heads);
    const calibrationEmitters = emitters.split("\n").map((line) => line.trim()).filter(Boolean).map((line) => { const [name, x, y, z, limit = "1"] = line.split(",").map((value) => value.trim()); return { name, xyz: { x: Number(x), y: Number(y), z: Number(z) }, limit: Number(limit) }; });
    const definition = { ...draft, ...parsed, name: draft.name.trim(), model: draft.model.trim() || draft.name.trim(), color_calibration: calibrationEmitters.length >= 3 ? { emitters: calibrationEmitters, correction_matrix: [[1,0,0],[0,1,0],[0,0,1]] } : null };
    if (await server.saveFixtureDefinition(definition)) { setSelectedFamilyKey(`${definition.manufacturer}\0${definition.model}`); setSelectedModeKey(`${definition.id}:${definition.revision}`); setDraft(blankDefinition()); setHeads([{ name: "Main", master: true, channels: "intensity,pan:16[-270,270,deg],tilt:16[-135,135,deg]" }]); setEmitters(""); setModal(null); }
  };
  const importFile = async (file?: File) => {
    if (!file) return; setBusy(true);
    try { const imported = await importGdtf(file); for (const definition of imported) await server.saveFixtureDefinition(definition); if (imported[0]) setSelectedFamilyKey(`${imported[0].manufacturer}\0${imported[0].model}`); setModal(null); }
    finally { setBusy(false); if (fileRef.current) fileRef.current.value = ""; }
  };
  const openCreate = () => { setDraft(blankDefinition()); setHeads([{ name: "Main", master: true, channels: "intensity,pan:16[-270,270,deg],tilt:16[-135,135,deg]" }]); setEmitters(""); setModal("editor"); };
  const toolbarTarget = document.getElementById("setup-section-actions");
  return <div className="fixture-library-setup">{toolbarTarget && createPortal(<><SearchBar value={query} onChange={setQuery} onSearch={() => setSubmitted(query)} filters={[{id:"type",label:"Fixture type",options:fixtureTypes}]} values={{type:typeFilter}} onFilterChange={(_,value) => setTypeFilter(value)} placeholder="Search manufacturer, fixture, mode, or type"/><button onClick={() => setModal("import")}>Import GDTF</button><button onClick={openCreate}>Create fixture</button></>, toolbarTarget)}<header className="fixture-library-title"><div><h2>Fixture library</h2><small>{libraryFamilies.length} fixtures · {server.fixtureLibrary.length} modes</small></div></header>
    <div className="fixture-library-columns"><section><h3>Manufacturer</h3><button className={!manufacturer?"active":""} onClick={() => setManufacturer("")}>All manufacturers</button>{manufacturers.map((name) => <button className={manufacturer===name?"active":""} key={name} onClick={() => setManufacturer(name)}>{name}</button>)}</section><section><h3>Fixture</h3>{libraryFamilies.map((family) => <button className={selectedLibraryFamily?.key===family.key?"active":""} key={family.key} onClick={() => {setSelectedFamilyKey(family.key);setSelectedModeKey(`${family.modes[0].id}:${family.modes[0].revision}`);}}>{family.name}<small>{family.deviceType} · {family.modes.length} modes</small></button>)}</section><section className="fixture-library-detail">{selectedLibraryFamily && selectedMode ? <><h3>{selectedLibraryFamily.manufacturer} {selectedLibraryFamily.name}</h3><label>Mode<select value={`${selectedMode.id}:${selectedMode.revision}`} onChange={(event) => setSelectedModeKey(event.target.value)}>{selectedLibraryFamily.modes.map((mode) => <option value={`${mode.id}:${mode.revision}`} key={`${mode.id}:${mode.revision}`}>{mode.mode} · {mode.footprint}ch</option>)}</select></label><dl><dt>Type</dt><dd>{selectedMode.device_type}</dd><dt>DMX footprint</dt><dd>{selectedMode.footprint} channels</dd><dt>Heads</dt><dd>{selectedMode.heads.length}</dd><dt>Revision</dt><dd>{selectedMode.revision}</dd><dt>Physical</dt><dd>{selectedMode.physical.width_millimetres ?? "?"} × {selectedMode.physical.height_millimetres ?? "?"} × {selectedMode.physical.depth_millimetres ?? "?"} mm</dd></dl><button onClick={() => edit(selectedMode)}>Edit fixture</button></>:<p>No fixture matches this search.</p>}</section></div>
    <input ref={fileRef} hidden type="file" accept=".gdtf,application/zip" onChange={(event) => void importFile(event.target.files?.[0])}/>
    {modal === "import" && <div className="stacked-modal-layer"><section className="nested-modal gdtf-import-modal"><header><h2>Import GDTF</h2><button onClick={() => setModal(null)}>×</button></header><p>Select a GDTF archive. Every DMX mode will be imported into the desk-wide fixture library.</p><button className="primary" disabled={busy} onClick={() => fileRef.current?.click()}>{busy?"Importing…":"Choose GDTF file"}</button></section></div>}
    {modal === "editor" && <div className="stacked-modal-layer"><section className="nested-modal fixture-editor-modal"><header><h2>{draft.revision > 1 ? "Edit fixture as new revision" : "Create fixture"}</h2><button onClick={() => setModal(null)}>×</button></header>
      <div className="configuration-form fixture-editor">
        <label>Manufacturer<input value={draft.manufacturer} onChange={(e) => setDraft({ ...draft, manufacturer: e.target.value })} /></label>
        <label>Name<input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
        <label>Model<input value={draft.model} onChange={(e) => setDraft({ ...draft, model: e.target.value })} /></label>
        <label>Device type<select value={draft.device_type} onChange={(e) => setDraft({ ...draft, device_type: e.target.value })}>{FIXTURE_TYPES.map((type) => <option key={type}>{type}</option>)}</select></label>
        <label>Mode<input value={draft.mode} onChange={(e) => setDraft({ ...draft, mode: e.target.value })} /></label>
        <label>Pan range °<input type="number" value={draft.physical.pan_range_degrees ?? ""} onChange={(e) => setDraft({ ...draft, physical: { ...draft.physical, pan_range_degrees: e.target.value ? Number(e.target.value) : null } })} /></label>
        <label>Tilt range °<input type="number" value={draft.physical.tilt_range_degrees ?? ""} onChange={(e) => setDraft({ ...draft, physical: { ...draft.physical, tilt_range_degrees: e.target.value ? Number(e.target.value) : null } })} /></label>
        <label>Width mm<input type="number" value={draft.physical.width_millimetres ?? ""} onChange={(e) => setDraft({ ...draft, physical: { ...draft.physical, width_millimetres: e.target.value ? Number(e.target.value) : null } })} /></label>
        <label>Height mm<input type="number" value={draft.physical.height_millimetres ?? ""} onChange={(e) => setDraft({ ...draft, physical: { ...draft.physical, height_millimetres: e.target.value ? Number(e.target.value) : null } })} /></label>
        <label>Depth mm<input type="number" value={draft.physical.depth_millimetres ?? ""} onChange={(e) => setDraft({ ...draft, physical: { ...draft.physical, depth_millimetres: e.target.value ? Number(e.target.value) : null } })} /></label>
        <label>Weight kg<input type="number" value={draft.physical.weight_kilograms ?? ""} onChange={(e) => setDraft({ ...draft, physical: { ...draft.physical, weight_kilograms: e.target.value ? Number(e.target.value) : null } })} /></label>
        <label>Stage icon<input type="file" accept="image/*" onChange={(e) => { const file = e.target.files?.[0]; if (file) void readFileAsDataUrl(file).then((icon_asset) => setDraft((current) => ({ ...current, icon_asset }))); }} /><small>{draft.icon_asset ? "Icon assigned" : "PNG, SVG, or other browser image"}</small></label>
        <label>3D fixture model<input type="file" accept=".glb,model/gltf-binary" onChange={(e) => { const file = e.target.files?.[0]; if (file) void readFileAsDataUrl(file).then((model_asset) => setDraft((current) => ({ ...current, model_asset }))); }} /><small>{draft.model_asset ? "Model assigned" : "Binary GLB"}</small></label>
      </div>
      <div className="fixture-head-editor"><h3>Heads and channels</h3>{heads.map((head, index) => <article key={index}><label>Head name<input value={head.name} onChange={(e) => setHeads((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, name: e.target.value } : item))} /></label><label className="head-master"><input type="checkbox" checked={head.master} onChange={(e) => setHeads((current) => current.map((item, itemIndex) => ({ ...item, master: itemIndex === index ? e.target.checked : e.target.checked ? false : item.master })))} /> Master/shared head</label><label>Channels<textarea value={head.channels} onChange={(e) => setHeads((current) => current.map((item, itemIndex) => itemIndex === index ? { ...item, channels: e.target.value } : item))} /></label>{heads.length > 1 && <button onClick={() => setHeads((current) => current.filter((_, itemIndex) => itemIndex !== index))}>Remove head</button>}</article>)}<button onClick={() => setHeads((current) => [...current, { name: `Head ${current.length + 1}`, master: false, channels: "intensity" }])}>Add head / layer</button><small>Syntax: attribute, pan:16[-270,270,deg], gobo&#123;Open=0-31|Dots=32-63&#125;. Channels are assigned sequentially across heads.</small></div>
      <label className="emitter-editor">Color calibration emitters<textarea value={emitters} onChange={(e) => setEmitters(e.target.value)} placeholder={"Red,0.64,0.33,0.03,1\nGreen,0.30,0.60,0.10,1\nBlue,0.15,0.06,0.79,1"} /><small>One emitter per line: name, X, Y, Z, limit. At least three emitters enable calibrated color optimization.</small></label>
      <footer><button onClick={() => setModal(null)}>Cancel</button><button className="primary" disabled={!draft.manufacturer.trim() || !draft.name.trim() || !heads.some((head) => head.channels.trim())} onClick={() => void save()}>Save fixture</button></footer></section></div>}
  </div>;
}
