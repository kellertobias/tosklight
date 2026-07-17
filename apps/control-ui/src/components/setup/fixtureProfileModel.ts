import type {
  ChannelFunction,
  ChannelResolution,
  ColorSystem,
  FixtureChannel,
  FixtureDefinition,
  FixtureHead,
  FixtureMode,
  FixtureProfile,
  GeometryGraph,
  HeadColorSystem,
  XyzValue,
} from "../../api/types";

export const uuid = () => crypto.randomUUID();

export interface XyyValue { x: number; y: number; luminance: number }

export function xyzToXyy(value: XyzValue): XyyValue {
  const sum = value.x + value.y + value.z;
  return sum > 0
    ? { x: value.x / sum, y: value.y / sum, luminance: value.y }
    : { x: 0, y: 0, luminance: 0 };
}

export function xyyToXyz(value: XyyValue): XyzValue {
  if (value.y <= 0 || value.luminance <= 0) return { x: 0, y: Math.max(0, value.luminance), z: 0 };
  return {
    x: Math.max(0, value.x * value.luminance / value.y),
    y: Math.max(0, value.luminance),
    z: Math.max(0, (1 - value.x - value.y) * value.luminance / value.y),
  };
}

const vector = (value = 0) => ({ x: value, y: value, z: value });

export function blankGeometry(headIds: string[] = []): GeometryGraph {
  const root = uuid();
  return {
    nodes: [{
      id: root,
      name: "Chassis",
      parent_id: null,
      transform: { translation: vector(), rotation_degrees: vector(), scale: vector(1) },
      pivot: vector(),
      glb_node: null,
      motion: null,
    }],
    emitters: headIds.map((headId, index) => ({
      id: uuid(),
      name: headIds.length === 1 ? "Beam" : `Beam ${index + 1}`,
      node_id: root,
      head_id: headId,
      origin: vector(),
      orientation_degrees: vector(),
      beam_angle_degrees: 20,
      field_angle_degrees: 24,
      feather: 0,
      focus: 1,
      layout: { type: "point" as const },
    })),
  };
}

export type GeometryTemplateName = "fixed" | "moving_head" | "bar" | "matrix" | "shared_pan_multi_head";

export function geometryTemplate(template: GeometryTemplateName, headIds: string[]): GeometryGraph {
  const graph = blankGeometry([]);
  const root = graph.nodes[0].id;
  let emitterParents = headIds.map(() => root);
  if (template === "moving_head" || template === "shared_pan_multi_head") {
    const pan = uuid();
    graph.nodes.push({
      id: pan,
      name: "Pan arm",
      parent_id: root,
      transform: { translation: vector(), rotation_degrees: vector(), scale: vector(1) },
      pivot: vector(),
      glb_node: null,
      motion: { attribute: "pan", kind: "rotation", axis: { x: 0, y: 1, z: 0 }, physical_min: -270, physical_max: 270 },
    });
    emitterParents = headIds.map((_, index) => {
      const tilt = uuid();
      graph.nodes.push({
        id: tilt,
        name: headIds.length === 1 ? "Tilt head" : `Tilt head ${index + 1}`,
        parent_id: pan,
        transform: { translation: vector(), rotation_degrees: vector(), scale: vector(1) },
        pivot: vector(),
        glb_node: null,
        motion: { attribute: "tilt", kind: "rotation", axis: { x: 1, y: 0, z: 0 }, physical_min: -135, physical_max: 135 },
      });
      return tilt;
    });
  }
  graph.emitters = headIds.map((headId, index) => ({
    id: uuid(),
    name: headIds.length === 1 ? "Beam" : `Beam ${index + 1}`,
    node_id: emitterParents[index],
    head_id: headId,
    origin: vector(),
    orientation_degrees: vector(),
    beam_angle_degrees: 20,
    field_angle_degrees: 24,
    feather: 0,
    focus: 1,
    layout: template === "bar"
      ? { type: "strip", count: 8, spacing_millimetres: 50 }
      : template === "matrix"
        ? { type: "matrix", columns: 4, rows: 4, spacing: { x: 50, y: 50, z: 0 } }
        : { type: "point" },
  }));
  return graph;
}

export function blankHead(index = 0, split = 1): FixtureHead {
  return { id: uuid(), name: index === 0 ? "Main" : `Head ${index + 1}`, master_shared: index === 0, split };
}

export function blankMode(name = "Default"): FixtureMode {
  const head = blankHead();
  return {
    id: uuid(),
    name,
    notes: "",
    splits: [{ number: 1, footprint: 1 }],
    heads: [head],
    channels: [],
    color_systems: [],
    control_actions: [],
    geometry: blankGeometry([head.id]),
  };
}

export function blankFixtureProfile(): FixtureProfile {
  return {
    schema_version: 2,
    id: uuid(),
    revision: 0,
    manufacturer: "",
    name: "",
    short_name: "",
    fixture_type: "other",
    notes: "",
    photograph_asset: null,
    stage_icon_asset: null,
    model_asset: null,
    physical: {
      width_millimetres: null,
      height_millimetres: null,
      depth_millimetres: null,
      weight_kilograms: null,
      power_watts: null,
    },
    modes: [blankMode()],
    hazardous: false,
    direct_control_protocols: [],
    signal_loss_policy: { type: "hold_last" },
    reserved_source: null,
  };
}

export function cloneProfile(profile: FixtureProfile): FixtureProfile {
  return structuredClone(profile);
}

export function resolutionBytes(resolution: ChannelResolution) {
  return { u8: 1, u16: 2, u24: 3, u32: 4 }[resolution];
}

export function maxRaw(resolution: ChannelResolution) {
  return { u8: 0xff, u16: 0xffff, u24: 0xffffff, u32: 0xffffffff }[resolution];
}

export interface HighlightRawChoice {
  semantic_id?: string;
  label?: string;
  name?: string;
  raw_value?: number;
  dmx_from?: number;
  dmx_to?: number;
}

function identifiesOpenOrWhite(value: string | undefined) {
  const normalized = (value ?? "").trim().toLowerCase().replaceAll(/[^a-z0-9]+/g, " ").trim();
  return ["open", "white", "clear", "color open", "colour open", "color white", "colour white", "open white", "no color", "no colour"].includes(normalized);
}

/**
 * Derive a physical raw Highlight default for a newly authored/imported channel. Existing profile
 * values are not normalized through this helper, so an explicitly authored raw look stays exact.
 */
export function semanticHighlightRaw(
  attribute: string,
  resolution: ChannelResolution,
  defaultRaw: number,
  invert = false,
  choices: HighlightRawChoice[] = [],
) {
  const maximum = maxRaw(resolution);
  const endpoint = (full: boolean) => full !== invert ? maximum : 0;
  if (attribute === "intensity") return endpoint(true);
  if (["color.red", "color.green", "color.blue", "color.white", "color.cold_white", "color.warm_white"].includes(attribute)) return endpoint(true);
  if (["color.cyan", "color.magenta", "color.yellow"].includes(attribute)) return endpoint(false);
  if (/^color\.emitter\.(red|green|blue|white|cold_white|warm_white)$/.test(attribute)) return endpoint(true);
  if (attribute.startsWith("color.wheel")) {
    const choice = choices.find((candidate) => identifiesOpenOrWhite(candidate.semantic_id) || identifiesOpenOrWhite(candidate.label) || identifiesOpenOrWhite(candidate.name));
    if (choice?.raw_value != null) return Math.max(0, Math.min(maximum, Math.round(choice.raw_value)));
    if (choice?.dmx_from != null) {
      const midpoint = choice.dmx_from + Math.floor(((choice.dmx_to ?? choice.dmx_from) - choice.dmx_from) / 2);
      return Math.round(Math.max(0, Math.min(255, midpoint)) * maximum / 255);
    }
  }
  return Math.max(0, Math.min(maximum, Math.round(defaultRaw)));
}

const semanticWhite: XyzValue = { x: 0.95047, y: 1, z: 1.08883 };

function correctedWhite(matrix: HeadColorSystem["correction_matrix"]): XyzValue {
  return {
    x: matrix[0][0] * semanticWhite.x + matrix[0][1] * semanticWhite.y + matrix[0][2] * semanticWhite.z,
    y: matrix[1][0] * semanticWhite.x + matrix[1][1] * semanticWhite.y + matrix[1][2] * semanticWhite.z,
    z: matrix[2][0] * semanticWhite.x + matrix[2][1] * semanticWhite.y + matrix[2][2] * semanticWhite.z,
  };
}

function additiveWhiteLevels(system: Extract<ColorSystem, { type: "additive" }>, matrix: HeadColorSystem["correction_matrix"]) {
  const visible = system.emitters.filter((emitter) => emitter.visible);
  if (visible.length < 3) {
    return visible.map((emitter) => /red|green|blue|white/i.test(emitter.name) ? 1 : 0);
  }
  const target = correctedWhite(matrix);
  const opticalLimits = visible.map((emitter) => Math.pow(emitter.maximum_level, emitter.response_curve));
  const levels = visible.map(() => 0);
  const norm = Math.max(0.001, visible.reduce((sum, emitter) => sum + emitter.xyz.x ** 2 + emitter.xyz.y ** 2 + emitter.xyz.z ** 2, 0));
  const rate = 0.8 / norm;
  for (let iteration = 0; iteration < 256; iteration += 1) {
    const produced = visible.reduce((sum, emitter, index) => ({
      x: sum.x + emitter.xyz.x * levels[index],
      y: sum.y + emitter.xyz.y * levels[index],
      z: sum.z + emitter.xyz.z * levels[index],
    }), { x: 0, y: 0, z: 0 });
    const error = { x: produced.x - target.x, y: produced.y - target.y, z: produced.z - target.z };
    visible.forEach((emitter, index) => {
      const gradient = 2 * (error.x * emitter.xyz.x + error.y * emitter.xyz.y + error.z * emitter.xyz.z);
      levels[index] = Math.max(0, Math.min(opticalLimits[index], levels[index] - rate * gradient));
    });
  }
  return levels;
}

/** Return the automatic physical Highlight raw for every channel in the current complete mode. */
export function semanticHighlightDefaultsForMode(mode: FixtureMode) {
  const values = new Map(mode.channels.map((channel) => {
    const choices = channel.functions.flatMap((fn) => fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
      ? [{ semantic_id: fn.behavior.semantic_id, label: fn.behavior.label, raw_value: fn.behavior.raw_value }]
      : []);
    return [channel.id, semanticHighlightRaw(channel.attribute, channel.resolution, channel.default_raw, channel.invert, choices)] as const;
  }));
  for (const record of mode.color_systems) {
    if (record.system.type === "additive") {
      const visible = record.system.emitters.filter((emitter) => emitter.visible);
      const levels = additiveWhiteLevels(record.system, record.correction_matrix);
      visible.forEach((emitter, index) => {
        const channel = mode.channels.find((candidate) => candidate.id === emitter.channel_id);
        const level = levels[index];
        if (!channel || !Number.isFinite(level) || emitter.response_curve <= 0) return;
        const drive = Math.max(0, Math.min(emitter.maximum_level, Math.pow(Math.max(0, Math.min(1, level)), 1 / emitter.response_curve)));
        const maximum = maxRaw(channel.resolution);
        const raw = Math.round(drive * maximum);
        values.set(channel.id, channel.invert ? maximum - raw : raw);
      });
    } else if (record.system.type === "subtractive") {
      for (const channelId of [record.system.cyan_channel_id, record.system.magenta_channel_id, record.system.yellow_channel_id]) {
        const channel = mode.channels.find((candidate) => candidate.id === channelId);
        if (channel) values.set(channel.id, channel.invert ? maxRaw(channel.resolution) : 0);
      }
    } else {
      const slot = record.system.slots.find((candidate) => identifiesOpenOrWhite(candidate.semantic_id) || identifiesOpenOrWhite(candidate.label))
        ?? record.system.slots.filter((candidate) => candidate.measured_xyz).sort((left, right) => {
          const distance = (value: XyzValue) => (value.x - semanticWhite.x) ** 2 + (value.y - semanticWhite.y) ** 2 + (value.z - semanticWhite.z) ** 2;
          return distance(left.measured_xyz!) - distance(right.measured_xyz!);
        })[0];
      if (slot) values.set(record.system.channel_id, slot.dmx_from + Math.floor((slot.dmx_to - slot.dmx_from) / 2));
    }
  }
  return values;
}

/** Track automatic defaults as a Color-system draft evolves, without touching custom raw values. */
export function reconcileColorSystemHighlightDefaults(mode: FixtureMode, colorSystems: HeadColorSystem[]): FixtureMode {
  const previous = semanticHighlightDefaultsForMode(mode);
  const next = { ...mode, color_systems: colorSystems };
  const derived = semanticHighlightDefaultsForMode(next);
  return {
    ...next,
    channels: mode.channels.map((channel) => channel.highlight_raw === previous.get(channel.id)
      ? { ...channel, highlight_raw: derived.get(channel.id) ?? channel.highlight_raw }
      : channel),
  };
}

export function channelSplit(mode: FixtureMode, channel: FixtureChannel) {
  return mode.heads.find((head) => head.id === channel.head_id)?.split ?? 0;
}

export function derivePrimarySlots(mode: FixtureMode): { slots: Map<string, number>; errors: string[] } {
  const errors: string[] = [];
  const footprints = new Map(mode.splits.map((split) => [split.number, split.footprint]));
  const reserved = new Map<number, Set<number>>();
  for (const channel of mode.channels) {
    const split = channelSplit(mode, channel);
    const footprint = footprints.get(split) ?? 0;
    if (channel.secondary_slots.length !== resolutionBytes(channel.resolution) - 1) {
      errors.push(`${channel.attribute || "Channel"}: ${channel.resolution.slice(1)}-bit resolution needs ${resolutionBytes(channel.resolution) - 1} component slots`);
    }
    const used = reserved.get(split) ?? new Set<number>();
    for (const slot of channel.secondary_slots) {
      if (!Number.isInteger(slot) || slot < 1 || slot > footprint) errors.push(`${channel.attribute || "Channel"}: component slot ${slot} is outside split ${split}`);
      if (used.has(slot)) errors.push(`Split ${split}: DMX component slot ${slot} is duplicated`);
      used.add(slot);
    }
    reserved.set(split, used);
  }
  const next = new Map<number, number>();
  const primaryUsed = new Map<number, Set<number>>();
  const slots = new Map<string, number>();
  for (const channel of mode.channels) {
    const split = channelSplit(mode, channel);
    const footprint = footprints.get(split) ?? 0;
    const occupied = reserved.get(split) ?? new Set<number>();
    const used = primaryUsed.get(split) ?? new Set<number>();
    let candidate = next.get(split) ?? 1;
    while (occupied.has(candidate) || used.has(candidate)) candidate += 1;
    if (!split || candidate > footprint) errors.push(`${channel.attribute || "Channel"}: split ${split || "?"} exceeds its ${footprint}-slot footprint`);
    slots.set(channel.id, candidate);
    used.add(candidate);
    primaryUsed.set(split, used);
    next.set(split, candidate + 1);
  }
  return { slots, errors: [...new Set(errors)] };
}

export function blankChannel(mode: FixtureMode, split = mode.splits[0]?.number ?? 1): FixtureChannel {
  const head = mode.heads.find((candidate) => candidate.split === split) ?? mode.heads[0];
  const resolution: ChannelResolution = "u8";
  const defaultRaw = 0;
  return {
    id: uuid(),
    head_id: head?.id ?? "",
    attribute: "intensity",
    resolution,
    secondary_slots: [],
    default_raw: defaultRaw,
    highlight_raw: semanticHighlightRaw("intensity", resolution, defaultRaw),
    physical_min: 0,
    physical_max: 100,
    unit: "percent",
    invert: false,
    snap: false,
    reacts_to_virtual_intensity: false,
    reacts_to_sequence_master: true,
    reacts_to_group_master: true,
    reacts_to_grand_master: true,
    behavior: "controlled",
    functions: [],
  };
}

export function blankFunction(channel: FixtureChannel, type: ChannelFunction["behavior"]["type"] = "continuous"): ChannelFunction {
  const range = maxRaw(channel.resolution);
  const behavior: ChannelFunction["behavior"] = type === "continuous"
    ? { type, physical_min: channel.physical_min ?? 0, physical_max: channel.physical_max ?? 1, unit: channel.unit }
    : type === "control"
      ? { type, action_id: "" }
      : { type, semantic_id: "", label: "", raw_value: 0 };
  return {
    id: uuid(),
    name: type === "continuous" ? channel.attribute : "Function",
    dmx_from: 0,
    dmx_to: range,
    attribute: channel.attribute,
    priority: type === "continuous" ? 0 : type === "control" ? 200 : 100,
    behavior,
  };
}

export function reorder<T>(items: T[], from: number, to: number) {
  if (from === to || from < 0 || from >= items.length || to < 0 || to >= items.length) return items;
  const next = [...items];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

export function validateProfile(profile: FixtureProfile) {
  const errors: string[] = [];
  if (!profile.manufacturer.trim()) errors.push("Manufacturer is required");
  if (!profile.name.trim()) errors.push("Fixture name is required");
  if (!profile.modes.length) errors.push("At least one mode is required");
  for (const [key, value] of Object.entries(profile.physical)) {
    if (value != null && (!Number.isFinite(value) || value <= 0)) errors.push(`${key.replaceAll("_", " ")} must be positive`);
  }
  const modeIds = new Set<string>();
  for (const mode of profile.modes) {
    if (!mode.name.trim()) errors.push("Every mode needs a name");
    if (modeIds.has(mode.id)) errors.push("Mode identities must be unique");
    modeIds.add(mode.id);
    const splitNumbers = new Set<number>();
    for (const split of mode.splits) {
      if (!Number.isInteger(split.number) || split.number < 1 || splitNumbers.has(split.number)) errors.push(`${mode.name}: split numbers must be unique positive integers`);
      if (!Number.isInteger(split.footprint) || split.footprint < 1 || split.footprint > 512) errors.push(`${mode.name}: split ${split.number} footprint must be 1–512`);
      splitNumbers.add(split.number);
    }
    if (!mode.splits.length) errors.push(`${mode.name}: at least one split is required`);
    if (!mode.heads.length) errors.push(`${mode.name}: at least one head is required`);
    if (mode.heads.filter((head) => head.master_shared).length > 1) errors.push(`${mode.name}: only one head can be master/shared`);
    const headIds = new Set(mode.heads.map((head) => head.id));
    for (const head of mode.heads) {
      if (!head.name.trim()) errors.push(`${mode.name}: every head needs a name`);
      if (!splitNumbers.has(head.split)) errors.push(`${mode.name}: ${head.name || "head"} references missing split ${head.split}`);
    }
    const channelIds = new Set<string>();
    for (const channel of mode.channels) {
      if (!headIds.has(channel.head_id)) errors.push(`${mode.name}: ${channel.attribute || "channel"} references a missing head`);
      if (channelIds.has(channel.id)) errors.push(`${mode.name}: channel identities must be unique`);
      channelIds.add(channel.id);
      const maximum = maxRaw(channel.resolution);
      if (!channel.attribute.trim()) errors.push(`${mode.name}: every channel needs an attribute`);
      if (!Number.isInteger(channel.default_raw) || channel.default_raw < 0 || channel.default_raw > maximum) errors.push(`${mode.name}: ${channel.attribute} default must be 0–${maximum}`);
      if (!Number.isInteger(channel.highlight_raw) || channel.highlight_raw < 0 || channel.highlight_raw > maximum) errors.push(`${mode.name}: ${channel.attribute} highlight must be 0–${maximum}`);
      const sorted = [...channel.functions].sort((left, right) => left.dmx_from - right.dmx_from);
      sorted.forEach((fn, index) => {
        if (fn.dmx_from < 0 || fn.dmx_to > maximum || fn.dmx_from > fn.dmx_to) errors.push(`${mode.name}: ${fn.name || "function"} has an invalid DMX range`);
        if (index && sorted[index - 1].dmx_to >= fn.dmx_from) errors.push(`${mode.name}: ${channel.attribute} function ranges overlap`);
      });
    }
    errors.push(...derivePrimarySlots(mode).errors.map((error) => `${mode.name}: ${error}`));
  }
  return [...new Set(errors)];
}

export function fixtureProfileFromDefinition(definition: FixtureDefinition): FixtureProfile {
  if (definition.profile_snapshot) return cloneProfile(definition.profile_snapshot);
  const heads: FixtureHead[] = definition.heads.map((head) => ({ id: uuid(), name: head.name, master_shared: head.shared, split: 1 }));
  const indexed = definition.heads.flatMap((head, headIndex) => head.parameters.map((parameter) => ({ headIndex, parameter })))
    .sort((left, right) => (left.parameter.components[0]?.offset ?? 0) - (right.parameter.components[0]?.offset ?? 0));
  const channels: FixtureChannel[] = indexed.map(({ headIndex, parameter }) => {
    const resolution = (`u${Math.max(1, parameter.components.length) * 8}`) as ChannelResolution;
    const maximum = maxRaw(resolution);
    const defaultRaw = Math.round(parameter.default * maximum);
    const invert = parameter.metadata?.invert ?? false;
    const functions: ChannelFunction[] = parameter.capabilities.map((capability) => {
      const dmxFrom = Math.round(Math.max(0, Math.min(255, capability.dmx_from)) * maximum / 255);
      const dmxTo = Math.round(Math.max(0, Math.min(255, capability.dmx_to)) * maximum / 255);
      const label = capability.name.trim() || parameter.attribute;
      const semanticId = label.toLowerCase().replaceAll(/[^a-z0-9]+/g, ".").replace(/^\.+|\.+$/g, "");
      const indexed = capability.preset_family === "color" || capability.preset_family === "gobo" || /color\.wheel|gobo/.test(parameter.attribute);
      return {
        id: uuid(),
        name: label,
        dmx_from: dmxFrom,
        dmx_to: dmxTo,
        attribute: parameter.attribute,
        priority: 100,
        behavior: indexed
          ? { type: "indexed", semantic_id: semanticId, label, raw_value: dmxFrom + Math.floor((dmxTo - dmxFrom) / 2) }
          : { type: "fixed", semantic_id: semanticId, label, raw_value: dmxFrom + Math.floor((dmxTo - dmxFrom) / 2) },
      };
    });
    return {
      id: uuid(),
      head_id: heads[headIndex].id,
      attribute: parameter.attribute,
      resolution,
      secondary_slots: parameter.components.slice(1).map((component) => component.offset + 1),
      default_raw: defaultRaw,
      highlight_raw: semanticHighlightRaw(parameter.attribute, resolution, defaultRaw, invert, parameter.capabilities),
      physical_min: parameter.metadata?.physical_min ?? 0,
      physical_max: parameter.metadata?.physical_max ?? 1,
      unit: parameter.metadata?.unit ?? null,
      invert,
      snap: false,
      reacts_to_virtual_intensity: parameter.virtual_dimmer,
      reacts_to_sequence_master: /intensity/.test(parameter.attribute),
      reacts_to_group_master: /intensity/.test(parameter.attribute),
      reacts_to_grand_master: /intensity/.test(parameter.attribute),
      behavior: "controlled",
      functions,
    };
  });
  const colorSystems: HeadColorSystem[] = definition.color_calibration ? heads.flatMap((head) => {
    const emitters = definition.color_calibration!.emitters.flatMap((emitter) => {
      const name = emitter.name.trim().toLowerCase().replaceAll(/[^a-z0-9]+/g, "_");
      const channel = channels.find((candidate) => candidate.head_id === head.id && [
        `color.emitter.${name}`,
        `color.${name}`,
      ].includes(candidate.attribute));
      return channel ? [{
        channel_id: channel.id,
        name: emitter.name,
        xyz: emitter.xyz,
        maximum_level: emitter.limit,
        response_curve: 1,
        visible: !/^(uv|ir)$|ultraviolet|infrared/i.test(emitter.name),
      }] : [];
    });
    return emitters.length ? [{
      head_id: head.id,
      correction_matrix: definition.color_calibration!.correction_matrix as HeadColorSystem["correction_matrix"],
      system: { type: "additive" as const, emitters },
    }] : [];
  }) : [];
  const mode: FixtureMode = {
    id: definition.mode_id ?? uuid(),
    name: definition.mode || "Default",
    notes: "",
    splits: [{ number: 1, footprint: definition.footprint }],
    heads,
    channels,
    color_systems: colorSystems,
    control_actions: [],
    geometry: blankGeometry(heads.map((head) => head.id)),
  };
  const semanticDefaults = semanticHighlightDefaultsForMode(mode);
  mode.channels = mode.channels.map((channel) => ({ ...channel, highlight_raw: semanticDefaults.get(channel.id) ?? channel.highlight_raw }));
  return {
    ...blankFixtureProfile(),
    id: definition.profile_id ?? definition.id,
    revision: definition.revision,
    manufacturer: definition.manufacturer,
    name: definition.name,
    short_name: definition.model,
    fixture_type: definition.device_type,
    stage_icon_asset: definition.icon_asset ?? null,
    model_asset: definition.model_asset ?? null,
    physical: {
      width_millimetres: definition.physical.width_millimetres ?? null,
      height_millimetres: definition.physical.height_millimetres ?? null,
      depth_millimetres: definition.physical.depth_millimetres ?? null,
      weight_kilograms: definition.physical.weight_kilograms ?? null,
      power_watts: definition.physical.power_watts ?? null,
    },
    hazardous: definition.hazardous,
    direct_control_protocols: definition.direct_control_protocols,
    signal_loss_policy: definition.signal_loss_policy,
    modes: [mode],
  };
}

/** Convert the ordered per-mode result of a legacy/GDTF import into one atomic profile draft. */
export function fixtureProfileFromDefinitions(definitions: FixtureDefinition[]): FixtureProfile {
  if (!definitions.length) return blankFixtureProfile();
  const converted = definitions.map(fixtureProfileFromDefinition);
  return {
    ...converted[0],
    revision: 0,
    modes: converted.flatMap((profile) => profile.modes),
  };
}

/** Resolve every ordered mode into the portable definition shape embedded by a patched show. */
export function fixtureDefinitionsFromProfiles(profiles: FixtureProfile[]): FixtureDefinition[] {
  return profiles.flatMap((profile) => profile.modes.map((mode) => fixtureDefinitionFromProfileMode(profile, mode)));
}

export function fixtureDefinitionFromProfileMode(profile: FixtureProfile, mode: FixtureMode): FixtureDefinition {
  const primary = derivePrimarySlots(mode).slots;
  const footprint = mode.splits.find((split) => split.number === 1)?.footprint ?? mode.splits[0]?.footprint ?? 1;
  const additive = mode.color_systems.find((record) => record.system.type === "additive");
  return {
    schema_version: 2,
    id: profile.id,
    revision: profile.revision,
    manufacturer: profile.manufacturer,
    device_type: profile.fixture_type,
    name: profile.name,
    model: profile.short_name,
    mode: mode.name,
    footprint,
    heads: mode.heads.map((head, index) => ({
      index,
      name: head.name,
      shared: head.master_shared,
      parameters: mode.channels.filter((channel) => channel.head_id === head.id).map((channel) => ({
        attribute: channel.attribute,
        components: [primary.get(channel.id) ?? 1, ...channel.secondary_slots].map((slot) => ({ offset: slot - 1, byte_order: "msb_first" as const })),
        default: channel.default_raw / maxRaw(channel.resolution),
        virtual_dimmer: channel.reacts_to_virtual_intensity,
        metadata: {
          physical_min: channel.physical_min ?? 0,
          physical_max: channel.physical_max ?? 1,
          unit: channel.unit,
          invert: channel.invert,
          wrap: false,
          curve: "linear",
        },
        capabilities: channel.functions.flatMap((fn) => fn.behavior.type === "fixed" || fn.behavior.type === "indexed" ? [{ name: fn.behavior.label, dmx_from: fn.dmx_from, dmx_to: fn.dmx_to, preset_family: fn.behavior.type === "indexed" ? (fn.attribute.includes("gobo") ? "gobo" : "color") : null }] : []),
      })),
    })),
    color_calibration: additive?.system.type === "additive" ? {
      emitters: additive.system.emitters.map((emitter) => ({ name: emitter.name, xyz: emitter.xyz, limit: emitter.maximum_level })),
      correction_matrix: additive.correction_matrix,
    } : null,
    physical: {
      width_millimetres: profile.physical.width_millimetres,
      height_millimetres: profile.physical.height_millimetres,
      depth_millimetres: profile.physical.depth_millimetres,
      weight_kilograms: profile.physical.weight_kilograms,
      power_watts: profile.physical.power_watts,
    },
    model_asset: profile.model_asset,
    icon_asset: profile.stage_icon_asset,
    hazardous: profile.hazardous,
    direct_control_protocols: profile.direct_control_protocols,
    signal_loss_policy: profile.signal_loss_policy,
    safe_values: {},
    profile_id: profile.id,
    mode_id: mode.id,
    // Library definitions are immutable view models. Reuse the profile here so a large
    // multi-mode catalog does not deep-clone the complete profile once per mode. The selected
    // definition is serialized by the patch request (creating its portable value snapshot), and
    // fixtureProfileFromDefinition deep-clones before an editor can mutate the profile.
    profile_snapshot: profile,
  };
}

export function fixtureDefinitionKey(definition: FixtureDefinition) {
  return `${definition.profile_id ?? definition.id}:${definition.revision}:${definition.mode_id ?? definition.id}`;
}

function migratedLegacyContentKey(definition: FixtureDefinition) {
  const normalized = (value: string) => value.trim().toLocaleLowerCase();
  const physical = definition.physical;
  return JSON.stringify([
    normalized(definition.manufacturer),
    normalized(definition.model),
    normalized(definition.name || definition.model),
    normalized(definition.mode),
    normalized(definition.device_type),
    definition.footprint,
    physical.width_millimetres ?? null,
    physical.height_millimetres ?? null,
    physical.depth_millimetres ?? null,
    physical.weight_kilograms ?? null,
    physical.power_watts ?? null,
  ]);
}

/** Prefer profile-backed modes and hide their retained schema-v1 migration sources. */
export function mergeFixtureDefinitions(profiles: FixtureProfile[], legacyDefinitions: FixtureDefinition[]) {
  const profileDefinitions = fixtureDefinitionsFromProfiles(profiles);
  const exactKeys = new Set(profileDefinitions.map(fixtureDefinitionKey));
  const migratedKeys = new Set(profileDefinitions.map(migratedLegacyContentKey));
  return [
    ...profileDefinitions,
    ...legacyDefinitions.filter((definition) =>
      !exactKeys.has(fixtureDefinitionKey(definition))
      && !migratedKeys.has(migratedLegacyContentKey(definition))),
  ];
}
