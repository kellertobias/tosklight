import type {
 ProgrammingColorProgram, ProgrammingColorIntent, ProgrammingNativeColorIdentity,
 ProgrammingNativeColorValue, ProgrammingNativeColorRecipe, ProgrammingScalarIntent,
 ProgrammingPositionIntent, ProgrammingZoomIntent, ProgrammingColorComponent, ProgrammingGroupFamilyAssignment, ProgrammingAttributeValue,
} from "./generated/light-wire";
import { arrayAt, booleanAt, enumAt, exactRecordAt, integerAt, numberAt, printableStringAt, recordAt } from "./playbackWirePrimitives";
import { WireValidationError } from "./wireValidation";

const components = ["red", "green", "blue", "amber", "hue", "saturation", "white_blend", "temperature", "duv", "uv", "relative_output"] as const;
export function decodeProgrammingComponent(value: unknown, path: string): import("./generated/light-wire").ProgrammingComponent {
 const component = recordAt(value,path);
 const kind = enumAt(component.kind,`${path}.kind`,["color","color_wheel","native_color","pan","tilt","target_reference","target_x","target_y","target_z","focus","zoom"]);
 exactRecordAt(value, path, ["color", "color_wheel", "native_color"].includes(kind) ? ["kind", "component"] : ["kind"]);
 if (kind === "color") return {kind,component:enumAt(component.component,`${path}.component`,components)};
 if (kind === "color_wheel") return {kind,component:unsigned(component.component,`${path}.component`,65_535)};
 if (kind === "native_color") {
  const binding = exactRecordAt(component.component,`${path}.component`,["channel_id", "function_id"]);
  return {kind,component:{channel_id:uuid(binding.channel_id,`${path}.component.channel_id`),function_id:uuid(binding.function_id,`${path}.component.function_id`)}};
 }
 return {kind};
}

export function decodeDynamicValueAddress(value: unknown, path: string): import("./generated/light-wire").DynamicValueAddressProjection {
 const item = exactRecordAt(value, path, ["representation", "component"]);
 const repPath = `${path}.representation`;
 const rep = recordAt(item.representation, repPath);
 const kind = enumAt(rep.kind, `${repPath}.kind`, ["angles", "target", "semantic_color", "direct_color", "focus", "zoom"]);
 const fields = kind === "target" ? ["kind", "reference"] : kind === "semantic_color" ? ["kind", "basis"] : kind === "direct_color" ? ["kind", "source"] : kind === "zoom" ? ["kind", "convention"] : ["kind"];
 exactRecordAt(rep, repPath, fields);
 const representation: import("./generated/light-wire").DynamicFamilyRepresentationProjection =
  kind === "target" ? { kind, reference: rep.reference == null ? null : decodeTargetReference(rep.reference, `${repPath}.reference`) } :
  kind === "semantic_color" ? { kind, basis: enumAt(rep.basis, `${repPath}.basis`, ["retain", "recipe", "hue_saturation", "whole"]) } :
  kind === "direct_color" ? { kind, source: nativeIdentity(rep.source, `${repPath}.source`) } :
  kind === "zoom" ? { kind, convention: enumAt(rep.convention, `${repPath}.convention`, ["beam", "field"]) } : { kind };
 const component = item.component == null ? null : decodeProgrammingComponent(item.component, `${path}.component`);
 let valid = false;
 if (!component) valid = representation.kind !== "semantic_color" || representation.basis === "whole";
 else switch (representation.kind) {
  case "angles": valid = component.kind === "pan" || component.kind === "tilt"; break;
  case "target": valid = representation.reference !== null && ["target_x", "target_y", "target_z"].includes(component.kind); break;
  case "semantic_color": valid = component.kind === "color" &&
   (["red", "green", "blue", "amber"].includes(component.component) ? representation.basis === "recipe" :
    ["hue", "saturation"].includes(component.component) ? representation.basis === "hue_saturation" : true); break;
  case "direct_color": valid = component.kind === "native_color"; break;
  case "focus": case "zoom": valid = component.kind === representation.kind; break;
 }
 if (!valid) invalid(path, "compatible Dynamic representation and component", value);
 return { representation, component };
}

function decodeTargetReference(value: unknown, path: string): import("./generated/light-wire").ProgrammingTargetReference {
 const kind = enumAt(recordAt(value, path).kind, `${path}.kind`, ["origin", "point"]);
 const item = exactRecordAt(value, path, kind === "origin" ? ["kind"] : ["kind", "point_id"]);
 return kind === "origin" ? { kind } : { kind, point_id: uuid(item.point_id, `${path}.point_id`) };
}
const qualities = ["unknown", "estimated", "manufacturer", "measured"] as const;
const u32max = 4_294_967_295;
const maxFloat = 3.4028234663852886e38;
function invalid(path: string, expected: string, actual: unknown): never { throw new WireValidationError(path, expected, actual); }
function bounded(value: unknown, path: string, min: number, max: number) {
 const result = numberAt(value, path);
 if (!Number.isFinite(Math.fround(result)) || Math.fround(result) < Math.fround(min) || Math.fround(result) > Math.fround(max)) invalid(path, `number within ${min}–${max}`, value);
 return result;
}
function unit(value: unknown, path: string) { return bounded(value, path, 0, 1); }
function physical(value: unknown, path: string) { return bounded(value, path, -maxFloat, maxFloat); }
function unsigned(value: unknown, path: string, max = u32max) {
 const result = integerAt(value, path);
 if (result > max) invalid(path, `integer <= ${max}`, value);
 return result;
}
function uuid(value: unknown, path: string) {
 if (typeof value !== "string" || !/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i.test(value) || /^0{8}-(?:0{4}-){3}0{12}$/.test(value)) invalid(path, "non-nil UUID", value);
 return value;
}
function list<T>(value: unknown, path: string, min: number, max: number, decode: (value: unknown, path: string) => T): T[] {
 const items = arrayAt(value, path);
 if (items.length < min || items.length > max) invalid(path, `${min}–${max} items`, value);
 return items.map((item, index) => decode(item, `${path}[${index}]`));
}
function triple<T>(value: unknown, path: string, decode: (value: unknown, path: string) => T): [T, T, T] {
 const values = list(value, path, 3, 3, decode);
 return [values[0], values[1], values[2]];
}
function unique<T>(values: T[], key: (item: T) => string, path: string) {
 if (new Set(values.map(key)).size !== values.length) invalid(path, "unique entries", values);
}
function xyz(value: unknown, path: string) {
 const item = exactRecordAt(value, path, ["x", "y", "z"]);
 return { x: bounded(item.x, `${path}.x`, 0, maxFloat), y: bounded(item.y, `${path}.y`, 0, maxFloat), z: bounded(item.z, `${path}.z`, 0, maxFloat) };
}
function scalar(value: unknown, path: string, decode = physical): ProgrammingScalarIntent {
 const item = exactRecordAt(value, path, ["kind", "value"]);
 const kind = enumAt(item.kind, `${path}.kind`, ["value", "spread"]);
 return kind === "value" ? { kind, value: decode(item.value, `${path}.value`) } : { kind, value: list(item.value, `${path}.value`, 2, 4096, decode) };
}
function nativeIdentity(value: unknown, path: string): ProgrammingNativeColorIdentity {
 const item = exactRecordAt(value, path, ["profile_id", "profile_revision", "profile_digest", "mode_id", "head_id", "path_id", "model_revision", "native_layout_signature"]);
 return {
  profile_id: uuid(item.profile_id, `${path}.profile_id`), profile_revision: unsigned(item.profile_revision, `${path}.profile_revision`),
  profile_digest: printableStringAt(item.profile_digest, `${path}.profile_digest`, 256),
  mode_id: uuid(item.mode_id, `${path}.mode_id`), head_id: uuid(item.head_id, `${path}.head_id`), path_id: uuid(item.path_id, `${path}.path_id`),
  model_revision: unsigned(item.model_revision, `${path}.model_revision`), native_layout_signature: printableStringAt(item.native_layout_signature, `${path}.native_layout_signature`, 256),
 };
}
function nativeValue(value: unknown, path: string): ProgrammingNativeColorValue {
 const item = exactRecordAt(value, path, ["channel_id", "function_id", "raw"]);
 return { channel_id: uuid(item.channel_id, `${path}.channel_id`), function_id: uuid(item.function_id, `${path}.function_id`), raw: unsigned(item.raw, `${path}.raw`) };
}
function nativeRecipe(value: unknown, path: string): ProgrammingNativeColorRecipe {
 const item = exactRecordAt(value, path, ["source", "channels", "spreads"]);
 const channels = list(item.channels, `${path}.channels`, 1, 512, nativeValue);
 unique(channels, item => item.channel_id, `${path}.channels`);
 const spreads = list(item.spreads === undefined ? [] : item.spreads, `${path}.spreads`, 0, channels.length, (value, path) => {
  const item = exactRecordAt(value, path, ["binding", "points"]);
  const binding = exactRecordAt(item.binding, `${path}.binding`, ["channel_id", "function_id"]);
  const channel_id = uuid(binding.channel_id, `${path}.binding.channel_id`);
  const function_id = uuid(binding.function_id, `${path}.binding.function_id`);
  if (!channels.some(channel => channel.channel_id === channel_id && channel.function_id === function_id)) invalid(path, "spread belonging to complete native recipe", value);
  return { binding: { channel_id, function_id }, points: list(item.points, `${path}.points`, 2, 4096, unsigned) };
 });
 unique(spreads, item => item.binding.channel_id, `${path}.spreads`);
 return { source: nativeIdentity(item.source, `${path}.source`), channels, ...(spreads.length ? { spreads } : {}) };
}
function componentValue(component: ProgrammingColorComponent, value: unknown, path: string) {
 switch (component) {
  case "hue": return bounded(value, path, 0, 360);
  case "temperature": return bounded(value, path, 1000, 20000);
  case "duv": return bounded(value, path, -0.03, 0.03);
  case "relative_output": return bounded(value, path, 0, maxFloat);
  default: return unit(value, path);
 }
}
function colorIntent(value: unknown, path: string): ProgrammingColorIntent {
 const item = exactRecordAt(value, path, ["base_xyz", "recipe", "white_blend", "white_target", "uv", "relative_output", "allocation", "wheel_constraints", "spreads"]);
 const recipe = exactRecordAt(item.recipe, `${path}.recipe`, ["version", "rgb", "amber", "approximate"]);
 if (recipe.version !== 1) invalid(`${path}.recipe.version`, "virtual recipe version 1", recipe.version);
 const white = exactRecordAt(item.white_target, `${path}.white_target`, ["kelvin", "duv"]);
 const uv = exactRecordAt(item.uv === undefined ? { amount: 0 } : item.uv, `${path}.uv`, ["amount"]);
 const spreads = list(item.spreads === undefined ? [] : item.spreads, `${path}.spreads`, 0, 11, (value, path) => {
  const item = exactRecordAt(value, path, ["component", "points"]);
  const component = enumAt(item.component, `${path}.component`, components);
  return { component, points: list(item.points, `${path}.points`, 2, 4096, (value, path) => componentValue(component, value, path)) };
 });
 unique(spreads, item => item.component, `${path}.spreads`);
 if (spreads.some(item => ["red", "green", "blue", "amber"].includes(item.component)) && spreads.some(item => ["hue", "saturation"].includes(item.component))) invalid(`${path}.spreads`, "one base Color representation", spreads);
 const wheels = list(item.wheel_constraints === undefined ? [] : item.wheel_constraints, `${path}.wheel_constraints`, 0, 32, (value, path) => {
  const item = exactRecordAt(value, path, ["source", "value"]);
  return { source: nativeIdentity(item.source, `${path}.source`), value: nativeValue(item.value, `${path}.value`) };
 });
 unique(wheels, item => `${item.source.path_id}:${item.value.channel_id}`, `${path}.wheel_constraints`);
 const base = xyz(item.base_xyz, `${path}.base_xyz`);
 const rgb = triple(recipe.rgb, `${path}.recipe.rgb`, unit);
 const amber = unit(recipe.amber, `${path}.recipe.amber`);
 const approximate = booleanAt(recipe.approximate, `${path}.recipe.approximate`);
 if (!approximate) {
  // Version 1 must match light_core::programming::VirtualColorAuthoringV1. XYZ remains
  // authoritative; approximate Easy recipes never replace the retained coordinates here.
  const linear = (value: number) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  const [r, g, b] = rgb.map(linear);
  const a = linear(0.5);
  const expected = [0.4124564*r + 0.3575761*g + 0.1804375*b + amber*(0.4124564 + 0.3575761*a),
   0.2126729*r + 0.7151522*g + 0.072175*b + amber*(0.2126729 + 0.7151522*a),
   0.0193339*r + 0.119192*g + 0.9503041*b + amber*(0.0193339 + 0.119192*a)];
  if ([base.x, base.y, base.z].some((value, i) => Math.abs(value - expected[i]) > 0.000101)) invalid(`${path}.recipe`, "exact virtual recipe matching authoritative XYZ", recipe);
 }
 return {
  base_xyz: base, recipe: { version: 1, rgb, amber, approximate },
  white_blend: unit(item.white_blend, `${path}.white_blend`), white_target: { kelvin: bounded(white.kelvin, `${path}.white_target.kelvin`, 1000, 20000), duv: bounded(white.duv, `${path}.white_target.duv`, -0.03, 0.03) },
  uv: { amount: unit(uv.amount, `${path}.uv.amount`) }, relative_output: bounded(item.relative_output === undefined ? 1 : item.relative_output, `${path}.relative_output`, 0, maxFloat),
  allocation: enumAt(item.allocation === undefined ? "preserve_recipe" : item.allocation, `${path}.allocation`, ["preserve_recipe", "prefer_white", "prefer_colored_emitters"]),
  ...(spreads.length ? { spreads } : {}), ...(wheels.length ? { wheel_constraints: wheels } : {}),
 };
}
export function decodeColorProgram(value: unknown, path: string): ProgrammingColorProgram {
 const kind = enumAt(recordAt(value, path).kind, `${path}.kind`, ["semantic", "direct"]);
 if (kind === "semantic") {
  const item = exactRecordAt(value, path, ["kind", "intent"]);
  return { kind, intent: colorIntent(item.intent, `${path}.intent`) };
 }
 const item = exactRecordAt(value, path, ["kind", "recipe", "portable"]);
 const recipe = nativeRecipe(item.recipe, `${path}.recipe`);
 const portable = exactRecordAt(item.portable, `${path}.portable`, ["model_revision", "visible", "uv", "quality", "limitations"]);
 const model_revision = unsigned(portable.model_revision, `${path}.portable.model_revision`);
 if (model_revision !== recipe.source.model_revision) invalid(`${path}.portable.model_revision`, "pinned source model revision", model_revision);
 const visible = portable.visible == null ? null : exactRecordAt(portable.visible, `${path}.portable.visible`, ["xyz", "relative_output"]);
 const uv = portable.uv == null ? null : exactRecordAt(portable.uv, `${path}.portable.uv`, ["amount", "quality"]);
 return { kind, recipe, portable: { model_revision,
  visible: visible && { xyz: xyz(visible.xyz, `${path}.portable.visible.xyz`), relative_output: bounded(visible.relative_output, `${path}.portable.visible.relative_output`, 0, maxFloat) },
  uv: uv && { amount: unit(uv.amount, `${path}.portable.uv.amount`), quality: enumAt(uv.quality, `${path}.portable.uv.quality`, qualities) },
  quality: enumAt(portable.quality, `${path}.portable.quality`, qualities), limitations: list(portable.limitations, `${path}.portable.limitations`, 0, 64, (value, path) => {
   if (typeof value !== "string" || new TextEncoder().encode(value).length > 1024) invalid(path, "string up to 1024 bytes", value);
   return value;
  }),
 } };
}
export function decodePositionIntent(value: unknown, path: string): ProgrammingPositionIntent {
 const kind = enumAt(recordAt(value, path).kind, `${path}.kind`, ["angles", "target"]);
 if (kind === "angles") {
  const item = exactRecordAt(value, path, ["kind", "pan_degrees", "tilt_degrees"]);
  return { kind, pan_degrees: scalar(item.pan_degrees, `${path}.pan_degrees`), tilt_degrees: scalar(item.tilt_degrees, `${path}.tilt_degrees`) };
 }
 const item = exactRecordAt(value, path, ["kind", "reference", "offset_metres"]);
 const referenceKind = enumAt(recordAt(item.reference, `${path}.reference`).kind, `${path}.reference.kind`, ["origin", "point"]);
 const reference = exactRecordAt(item.reference, `${path}.reference`, referenceKind === "origin" ? ["kind"] : ["kind", "point_id"]);
 return { kind, reference: referenceKind === "origin" ? { kind: referenceKind } : { kind: referenceKind, point_id: uuid(reference.point_id, `${path}.reference.point_id`) }, offset_metres: triple(item.offset_metres, `${path}.offset_metres`, scalar) };
}
export function decodeZoomIntent(value: unknown, path: string): ProgrammingZoomIntent {
 const item = exactRecordAt(value, path, ["opening_degrees", "convention"]);
 return { opening_degrees: scalar(item.opening_degrees, `${path}.opening_degrees`, (value, path) => bounded(value, path, 0, 180)), convention: enumAt(item.convention, `${path}.convention`, ["beam", "field"]) };
}

export function decodeGroupFamily(value: unknown, path: string): ProgrammingGroupFamilyAssignment {
 const item = exactRecordAt(value, path, ["owner", "template", "members"]);
 const owner = enumAt(item.owner, `${path}.owner`, ["color", "position", "focus", "zoom"]);
 const member = (value: unknown, path: string): ProgrammingAttributeValue => {
  const entry = exactRecordAt(value, path, ["kind", "value"]);
  const expected = owner === "color" ? "color_program" : owner === "focus" ? ["normalized", "spread"] : owner;
  if (Array.isArray(expected) ? !expected.includes(String(entry.kind)) : entry.kind !== expected) invalid(path, "complete non-nested value for its declared Group family", value);
  if (owner === "color") return { kind: "color_program", value: decodeColorProgram(entry.value, `${path}.value`) };
  if (owner === "position") return { kind: "position", value: decodePositionIntent(entry.value, `${path}.value`) };
  if (owner === "zoom") return { kind: "zoom", value: decodeZoomIntent(entry.value, `${path}.value`) };
  return entry.kind === "normalized" ? { kind: "normalized", value: unit(entry.value, `${path}.value`) } : { kind: "spread", value: list(entry.value, `${path}.value`, 2, 4096, unit) };
 };
 const entries = Object.entries(recordAt(item.members === undefined ? {} : item.members, `${path}.members`));
 if (entries.length > 10000) invalid(`${path}.members`, "at most 10000 member exceptions", item.members);
 const members = Object.fromEntries(entries.map(([id, value]) => [uuid(id, `${path}.members`), member(value, `${path}.members.${id}`)]));
 return { owner, template: member(item.template, `${path}.template`), ...(entries.length ? { members } : {}) };
}

/** Shared leaf validators for outbound semantic component edits (programmingComponentEditWire). */
export {
 decodeTargetReference as decodeProgrammingTargetReference,
 scalar as decodeProgrammingScalarIntent,
 xyz as decodeProgrammingColorXyz,
 unsigned as programmingUnsignedAt,
 uuid as programmingNonNilUuidAt,
 u32max as PROGRAMMING_U32_MAX,
};
