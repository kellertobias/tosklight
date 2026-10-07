import type {
 ColorPhysicalModel, FixtureMode, FixtureChannel, ChannelFunction, HeadOpticalPath, NativeColorBinding,
 OpticalProvenance, SpectrumSample, XyzValue,
} from "../../fixtureProfile";
import { uuid } from "./utilities";

export const unknownOpticalProvenance = (): OpticalProvenance => ({quality: "unknown", revision: 0});
const xyzValid = (xyz: XyzValue) => [xyz.x, xyz.y, xyz.z].every((v) => Number.isFinite(Math.fround(v)) && v >= 0);
const provenanceValid = (p: OpticalProvenance) =>
 Number.isInteger(p.revision) && p.revision >= 0 && p.revision <= 0xffff_ffff &&
 (!["manufacturer", "measured"].includes(p.quality) || Boolean(p.source?.trim()));
const spectrumValid = (s: SpectrumSample[], filter: boolean) => s.length >= 2 && s.every((v, i) =>
 Number.isFinite(Math.fround(v.wavelength_nm)) && v.wavelength_nm >= 200 && v.wavelength_nm <= 2500 &&
 Number.isFinite(Math.fround(v.value)) && v.value >= 0 && (!filter || v.value <= 1) &&
 (!i || Math.fround(v.wavelength_nm) > Math.fround(s[i-1].wavelength_nm)));
const rawValid = (v: number) => Number.isInteger(v) && v >= 0 && v <= 0xffff_ffff;

const isColorAttribute = (attribute: string) => attribute === "color" || attribute.startsWith("color.");
export const nativeColorFunctionAllowed = (channel: FixtureChannel, fn: ChannelFunction) =>
 fn.behavior.type !== "control" && (isColorAttribute(fn.attribute) ||
  ![channel.fixture_attribute, channel.attribute, fn.attribute].includes("fixture.control"));

export function declaredColorControls(mode: FixtureMode, headId: string): string[] {
 const ids = new Set(mode.channels.filter((c) => c.head_id === headId &&
  ([c.fixture_attribute,c.attribute].some(isColorAttribute) || c.functions.some((f) => isColorAttribute(f.attribute)))).map((c) => c.id));
 for (const record of mode.color_systems.filter((s) => s.head_id === headId)) {
  const system = record.system;
  if (system.type === "additive") system.emitters.forEach((e) => ids.add(e.channel_id));
  else if (system.type === "subtractive") [system.cyan_channel_id,system.magenta_channel_id,system.yellow_channel_id].forEach((id) => ids.add(id));
  else if (system.type === "hue_saturation") [system.hue_channel_id,system.saturation_channel_id,system.intensity_channel_id].forEach((id) => { if (id) ids.add(id); });
  else ids.add(system.channel_id);
 }
 return [...ids];
}

/** Explicit authoring action; copying nominal emitter data does not manufacture a measurement. */
export function createHeadOpticalPath(mode: FixtureMode, headId: string): HeadOpticalPath {
 const controls = new Set(declaredColorControls(mode, headId));
 mode.channels.filter((c) => c.head_id === headId && c.fixture_attribute.startsWith("color.")).forEach((c) => controls.add(c.id));
 const path: HeadOpticalPath = {id: uuid(), head_id: headId, controls: [...controls], source: {type: "unknown"}, filters: [], measurements: []};
 const records = mode.color_systems.filter((s) => s.head_id === headId);
 const additive = records.find((r) => r.system.type === "additive");
 if (additive?.system.type === "additive") {
  const emitters = additive.system.emitters.flatMap((e) => {
   const channel = mode.channels.find((c) => c.id === e.channel_id);
   const functions = channel?.functions.filter((f) => f.behavior.type === "continuous") ?? [];
   if (functions.length !== 1) return [];
   const calibration = additive.calibration;
   return [{id: uuid(), name: e.name, binding: {channel_id: e.channel_id, function_id: functions[0].id},
    // A declared UV/IR control keeps its role even when its output is partly visible.
    // Legacy nonvisible zero XYZ was a placeholder, not a measured absence of visible light.
    xyz: !e.visible && e.xyz.x === 0 && e.xyz.y === 0 && e.xyz.z === 0 && !(calibration?.status === "measured" && calibration.source?.trim()) ? null : e.xyz,
    spectrum: [], band: channel?.fixture_attribute === "color.uv" ? "ultraviolet" as const : channel?.fixture_attribute === "color.ir" ? "infrared" as const : e.visible ? "visible" as const : "other_non_visible" as const,
    native_reversed: channel?.invert ?? false, maximum_level: e.maximum_level, response_exponent: e.response_curve,
    provenance: {quality: calibration?.status === "measured" && calibration.source?.trim() ? "measured" as const : calibration?.status === "uncalibrated" ? "unknown" as const : "estimated" as const,
     source: calibration?.source ?? "Existing nominal emitter data", revision: calibration?.revision ?? 0}}];
  });
  if (emitters.length) path.source = {type: "additive", emitters};
 }
 // The legacy list is not an optical order: let the author add and order filter stages.
 return path;
}

export function colorPhysicalErrors(mode: FixtureMode): string[] {
 const model = mode.color_physical;
 if (!model) return [];
 const errors: string[] = [];
 if (model.version !== 1) return ["Unsupported physical color model version"];
 if (!Number.isInteger(model.revision) || model.revision < 0 || model.revision > 0xffff_ffff) errors.push("Optical model revision must be a nonnegative integer");
 if (!model.paths.length) errors.push("Physical color model needs at least one head path");
 const ids = new Set<string>();
 const heads = new Set<string>();
 const identity = (id: string) => { const valid = Boolean(id) && !/^0{8}-0{4}-0{4}-0{4}-0{12}$/.test(id) && !ids.has(id); ids.add(id); return valid; };
 for (const path of model.paths) {
  if (!identity(path.id) || heads.has(path.head_id) || !mode.heads.some((h) => h.id === path.head_id)) errors.push("Physical color paths need unique identities and existing distinct heads");
  heads.add(path.head_id);
  const controls = new Set<string>();
  for (const id of path.controls) {
   const channel = mode.channels.find((c) => c.id === id);
   if (!channel) { errors.push("Physical color control references a missing channel"); continue; }
   if (controls.has(id) || (channel.head_id !== path.head_id && !mode.heads.some((h) => h.id === channel.head_id && h.master_shared))) errors.push("Physical color controls must be distinct and belong to this or the shared head");
   controls.add(id);
   if (!channel.functions.length) errors.push("Native color controls require explicit functions");
  }
  if (declaredColorControls(mode,path.head_id).some((id) => !controls.has(id))) errors.push("Physical path omits a declared native Color channel or existing color system control");
  const modeled = new Set<string>();
  const binding = (b: NativeColorBinding) => {
   const key = `${b.channel_id}:${b.function_id}`;
   if (!controls.has(b.channel_id) || modeled.has(key)) errors.push("Optical bindings must name a path control and cannot be modeled twice");
   modeled.add(key);
   const channel = mode.channels.find((c) => c.id === b.channel_id);
   const fn = channel?.functions.find((f) => f.id === b.function_id);
   if (channel && fn && !nativeColorFunctionAllowed(channel,fn)) errors.push("Service or unclassified fixture-control functions cannot be optical color bindings");
   if (!fn) errors.push("Optical binding references a missing native function");
   return fn;
  };
  if (path.source.type === "fixed") {
   const s = path.source;
   if ((s.xyz && !xyzValid(s.xyz)) || (s.spectrum.length && !spectrumValid(s.spectrum,false)) || !provenanceValid(s.provenance)) errors.push("Fixed optical source data or provenance is invalid");
  } else if (path.source.type === "additive") {
   if (!path.source.emitters.length) errors.push("Additive optical source needs emitters");
   for (const e of path.source.emitters) {
    const fn = binding(e.binding);
    if (fn && (fn.behavior.type !== "continuous" || fn.dmx_from >= fn.dmx_to)) errors.push("Optical emitter must reference a continuous native function");
    if (!identity(e.id) || !e.name.trim() || !Number.isFinite(Math.fround(e.maximum_level)) || Math.fround(e.maximum_level) <= 0 || e.maximum_level > 1 || !Number.isFinite(Math.fround(e.response_exponent)) || Math.fround(e.response_exponent) <= 0 || (e.xyz && !xyzValid(e.xyz)) || (e.spectrum.length && !spectrumValid(e.spectrum,false)) || !provenanceValid(e.provenance)) errors.push("Optical emitter identity, data or provenance is invalid");
   }
  }
  for (const filter of path.filters) {
   const fn = binding(filter.binding);
   if (fn?.behavior.type === "control") errors.push("Service control functions cannot be optical color filters");
   if (!identity(filter.id) || !filter.name.trim() || !provenanceValid(filter.provenance)) errors.push("Optical filter identity or provenance is invalid");
   if (filter.transmission.type === "spectral") {
    const samples = filter.transmission.samples;
    if (!samples.length || samples.some((s,i) => !rawValid(s.raw_from) || !rawValid(s.raw_to) || s.raw_from > s.raw_to || (fn && (s.raw_from < fn.dmx_from || s.raw_to > fn.dmx_to)) || !spectrumValid(s.spectrum,true) || (i > 0 && samples[i-1].raw_to >= s.raw_from))) errors.push("Filter spectra need sorted non-overlapping native ranges and valid transmission");
   }
  }
  for (const measurement of path.measurements ?? []) {
   if (!xyzValid(measurement.xyz) || !provenanceValid(measurement.provenance)) errors.push("Whole-path color measurement or provenance is invalid");
   const seen = new Set<string>();
   for (const value of measurement.recipe) {
    const channel = mode.channels.find((c) => c.id === value.channel_id);
    const fn = channel?.functions.find((f) => f.id === value.function_id);
    if (channel && fn && !nativeColorFunctionAllowed(channel,fn)) errors.push("Service control functions cannot be captured as native color");
    if (!fn || !controls.has(value.channel_id) || seen.has(value.channel_id) || !rawValid(value.raw) || value.raw < fn.dmx_from || value.raw > fn.dmx_to) errors.push("Native color recipe has missing, duplicate, foreign or out-of-function values");
    seen.add(value.channel_id);
   }
   if (seen.size !== controls.size) errors.push("Native color recipe must contain every path control");
  }
 }
 return [...new Set(errors)];
}

export function opticalPathWarnings(path: HeadOpticalPath, mode?: FixtureMode): string[] {
 const warnings: string[] = [];
 const bindings = [...(path.source.type === "additive" ? path.source.emitters.map((e) => e.binding) : []), ...path.filters.map((f) => f.binding)];
 if (path.controls.some((id) => !bindings.some((b) => b.channel_id === id)) || mode?.channels.some((c) => path.controls.includes(c.id) && c.functions.some((f) => f.behavior.type !== "control" && !bindings.some((b) => b.channel_id === c.id && b.function_id === f.id))))
  warnings.push("Some native Color controls or functions have no physical appearance model; their output remains unknown.");
 if (path.source.type === "unknown") warnings.push("Source appearance is unknown.");
 if (path.source.type === "fixed" && !path.source.xyz && !path.source.spectrum.length) warnings.push("Source appearance is unknown.");
 if (path.source.type === "additive" && path.source.emitters.some((e) => !e.xyz && !e.spectrum.length)) warnings.push("Some emitter appearances are unknown.");
 if (path.filters.some((f) => f.transmission.type === "unknown")) warnings.push("Filter transmission is unknown; serial color cannot be predicted from isolated XYZ swatches.");
 if (path.filters.length && path.source.type !== "unknown" && (path.source.type === "fixed" ? !path.source.spectrum.length : path.source.emitters.some((e) => !e.spectrum.length))) warnings.push("Serial spectral prediction also needs source spectra. Exact whole-path measurements may describe individual recipes.");
 return warnings;
}

export function withOpticalPath(mode: FixtureMode, path: HeadOpticalPath): FixtureMode {
 const model: ColorPhysicalModel = mode.color_physical ?? {version: 1, revision: 0, paths: []};
 return {...mode, color_physical: {...model, paths: model.paths.some((p) => p.head_id === path.head_id) ? model.paths.map((p) => p.head_id === path.head_id ? path : p) : [...model.paths,path]}};
}
