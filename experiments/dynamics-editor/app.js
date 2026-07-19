const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const svgNamespace = "http://www.w3.org/2000/svg";

const attributePresentation = {
  Intensity: { color: "amber", unit: "%", family: "Dimmer", minimum: 0, maximum: 100 },
  Pan: { color: "cyan", unit: "°", family: "Position", minimum: -180, maximum: 180 },
  Tilt: { color: "cyan", unit: "°", family: "Position", minimum: -90, maximum: 90 },
  Red: { color: "red", unit: "%", family: "Color", minimum: 0, maximum: 100 },
  Green: { color: "green", unit: "%", family: "Color", minimum: 0, maximum: 100 },
  Blue: { color: "blue", unit: "%", family: "Color", minimum: 0, maximum: 100 },
};

const colorPresets = [
  ["Red", "#ff0000"], ["Orange", "#ff6500"], ["Yellow", "#ffd500"], ["Lime", "#91ff00"],
  ["Green", "#00d83a"], ["Teal", "#00a681"], ["Cyan", "#00e5ff"], ["Light blue", "#55b7ff"],
  ["Dark blue", "#1749d1"], ["Purple", "#7438d1"], ["Magenta", "#f000d8"], ["Pink", "#ff5da9"],
  ["Warm white", "#ffd9aa"], ["Pure white", "#ffffff"], ["Color off", "#000000"],
].map(([name, hex]) => ({ name, hex, rgb: hexToRgb(hex) }));

const intensityPresets = [{ name: "Off", value: 0 }, { name: "On", value: 100 }];
const positionPresets = {
  Pan: [
    { name: "Back", value: 180 }, { name: "Up", value: 0 }, { name: "Down", value: 0 },
    { name: "Left", value: -90 }, { name: "Right", value: 90 },
  ],
  Tilt: [
    { name: "Back", value: 45 }, { name: "Up", value: 0 }, { name: "Down", value: -90 },
    { name: "Left", value: 0 }, { name: "Right", value: 0 },
  ],
};

const laneModes = ["Keyframes", "Function max/min", "Function amplitude"];
const functionTypes = ["Sinus", "Cosinus", "Linear +", "Linear −", "PWM", "Random gate", "Random timing", "Random gate + timing", "Macro (future)"];
const functionDescriptions = {
  Sinus: "Smooth wave that starts in the middle and completes one full cycle · controls: value range, speed, width",
  Cosinus: "Smooth wave that starts at maximum and completes one full cycle · controls: value range, speed, width",
  "Linear +": "Rises steadily from minimum to maximum · controls: value range, speed, width",
  "Linear −": "Falls steadily from maximum to minimum · controls: value range, speed, width",
  PWM: "Switches between minimum and maximum with shaped edges · controls: attack, on, decay, off, speed, width",
  "Random gate": "Makes seeded minimum/maximum gate decisions at regular opportunities · controls: pulse, grouping, density, source, speed, width",
  "Random timing": "Places maximum pulses at seeded random moments · controls: pulse, grouping, density, source, speed, width",
  "Random gate + timing": "Randomizes both event timing and minimum/maximum gate decisions · controls: pulse, grouping, density, source, speed, width",
  "Macro (future)": "Will run a reusable operator-defined function · macro selection and parameters are not available yet",
};
const keyframeInterpolations = ["Linear", "Ease in", "Ease out", "Ease in + out", "Hold", "Drop"];
const orderingModes = ["Selection", "Random each loop", "Linear", "Radial out", "Radial in", "Axial"];
const multiplierOptions = ["÷ 4", "÷ 3", "÷ 2", "× 1", "× 2", "× 3", "× 4"];
const speedGroups = [60, 85, 95, 105, 120, 150].map((bpm, index) => ({ name: `Speed Group ${index + 1}`, bpm }));
const quantizeOptions = ["Off", "Beat", "Bar", "2 bars"];
const startPolicies = ["Start now", "Join sync", "Next boundary"];
const sourceOptions = ["Preset", "Fixed", "Current"];
const randomSources = [
  { name: "Random 1", seed: 137 }, { name: "Random 2", seed: 419 },
  { name: "Random 3", seed: 887 }, { name: "Random 4", seed: 1597 },
];
const orderingDescriptions = {
  Selection: "Selection follows row-first fixture order: top-left to top-right, then the next row.",
  "Random each loop": "Random each loop reshuffles all 100 fixtures at every new effect iteration.",
  Linear: "Grid linear projects fixtures along the selected angle.",
  "Radial out": "Radial out is a ping expanding from the center.",
  "Radial in": "Radial in collapses from the edges toward the center.",
  Axial: "Axial sweeps clockwise around the center like radar.",
};

function hexToRgb(hex) {
  const value = Number.parseInt(hex.slice(1), 16);
  return { r: value >> 16, g: value >> 8 & 255, b: value & 255 };
}

function frame(time, source, value, preset, options = {}) {
  return { time, source, value, preset, interpolation: "Ease in + out", ...options };
}

function scalarSource(source = "Fixed", value = 0, preset = "") {
  return { source, value, preset };
}

function defaultFunctionConfig(attribute) {
  const info = attributePresentation[attribute];
  const center = (info.minimum + info.maximum) / 2;
  return {
    type: "Sinus",
    center: scalarSource("Fixed", center),
    amplitude: (info.maximum - info.minimum) / 4,
    bottom: scalarSource("Fixed", info.minimum),
    top: scalarSource("Fixed", info.maximum),
    random: { sourceIndex: 0, density: 8, grouping: 35, pulseWidth: 8 },
  };
}

function lane(attribute, shape, keyframes, multiplier = "× 1", pwm = { attack: 8, on: 50, decay: 8, off: 50 }) {
  return { attribute, mode: "Keyframes", shape, keyframes, multiplier, pwm, functionConfig: defaultFunctionConfig(attribute), functionWidth: 100, widthScale: 100, scaleBaseTimes: keyframes.map((item) => item.time) };
}

function functionLane(attribute, type, center, amplitude, multiplier = "× 1") {
  const targetLane = lane(attribute, "Sine", [frame(0, "Current", 0, ""), frame(100, "Current", 0, "", { loop: true })], multiplier);
  targetLane.mode = "Function amplitude";
  targetLane.functionConfig.type = type;
  targetLane.functionConfig.center = center;
  targetLane.functionConfig.amplitude = amplitude;
  return targetLane;
}

function randomStrobeLane() {
  const targetLane = functionLane("Intensity", "Random timing", scalarSource("Fixed", 50), 50);
  targetLane.functionConfig.random = { sourceIndex: 0, density: 8, grouping: 72, pulseWidth: 6 };
  targetLane.functionWidth = 45;
  return targetLane;
}

function colorLane(attribute, points, shape = "Linear") {
  const channel = { Red: "r", Green: "g", Blue: "b" }[attribute];
  return lane(attribute, shape, points.map(([time, presetName], index) => {
    const preset = colorPresets.find((item) => item.name === presetName);
    return frame(time, "Preset", Math.round(preset.rgb[channel] / 255 * 100), presetName, index === points.length - 1 ? { loop: true } : {});
  }));
}

const examples = [
  {
    name: "Circle around current",
    description: "Pan and tilt orbit around the current position while intensity stays on.",
    ordering: "Axial",
    duration: 6,
    lanes: [
      lane("Intensity", "Hold", [frame(0, "Preset", 100, "On"), frame(100, "Preset", 100, "On", { loop: true })]),
      functionLane("Pan", "Sinus", scalarSource("Current", 0, "Up"), 90),
      functionLane("Tilt", "Cosinus", scalarSource("Current", 0, "Up"), 45),
    ],
  },
  {
    name: "Top-down tilt travel",
    description: "A linear grid wave tilts Back to Up and returns; intensity is on only for the outward move.",
    ordering: "Linear",
    duration: 8,
    lanes: [
      lane("Intensity", "Linear", [frame(0, "Preset", 0, "Off"), frame(3, "Preset", 100, "On"), frame(35, "Preset", 100, "On"), frame(42, "Preset", 0, "Off"), frame(100, "Preset", 0, "Off", { loop: true })]),
      lane("Tilt", "Sine", [frame(0, "Preset", 45, "Back"), frame(38, "Preset", 0, "Up"), frame(58, "Preset", 0, "Up"), frame(100, "Preset", 45, "Back", { loop: true })]),
    ],
  },
  {
    name: "Radial red-white wave",
    description: "A radial-out pulse turns on red, passes through white, returns to red, then turns off.",
    ordering: "Radial out",
    duration: 5,
    lanes: [
      lane("Intensity", "Linear", [frame(0, "Preset", 0, "Off"), frame(3, "Preset", 100, "On"), frame(84, "Preset", 100, "On"), frame(91, "Preset", 0, "Off"), frame(100, "Preset", 0, "Off", { loop: true })]),
      colorLane("Red", [[0, "Red"], [22, "Red"], [43, "Pure white"], [57, "Pure white"], [78, "Red"], [100, "Red"]]),
      colorLane("Green", [[0, "Red"], [22, "Red"], [43, "Pure white"], [57, "Pure white"], [78, "Red"], [100, "Red"]]),
      colorLane("Blue", [[0, "Red"], [22, "Red"], [43, "Pure white"], [57, "Pure white"], [78, "Red"], [100, "Red"]]),
    ],
  },
  {
    name: "Random Strobe",
    description: "Intensity-only flashes use seeded random timing, grouped into short bursts, with a new pattern every loop.",
    ordering: "Random each loop",
    duration: 2,
    lanes: [randomStrobeLane()],
  },
];

examples.forEach((example) => example.lanes.forEach(relabelFrames));

const state = {
  view: "curves",
  exampleIndex: 0,
  laneAttribute: "Intensity",
  selectedAttributes: ["Intensity"],
  keyframeIndex: 0,
  functionSourceField: null,
  modeModalTab: "Keyframes",
  encoder: 1,
  ordering: examples[0].ordering,
  phaseStart: 0,
  phaseEnd: 360,
  phaseOffset: 0,
  linearDirection: 90,
  phaseBlock: 1,
  phaseRepeats: 1,
  cycleSeconds: examples[0].duration,
  speedGroupIndex: -1,
  lastSpeedGroupIndex: 4,
  overallMultiplier: "× 1",
  quantize: "Beat",
  startPolicy: "Start now",
  transportOffset: 0,
  playing: true,
  previewStart: 0,
  progress: 0,
  iteration: 0,
  selectedFixture: 0,
  animationFrame: null,
  toastTimer: null,
  knobTurns: [0, 0, 0, 0, 0, 0],
};

function currentExample() { return examples[state.exampleIndex]; }
function selectedLane() { return currentExample().lanes.find((item) => item.attribute === state.laneAttribute) || currentExample().lanes[0]; }
function selectedLanes() { return currentExample().lanes.filter((item) => state.selectedAttributes.includes(item.attribute)); }
function selectedFrame() { return selectedLane().keyframes[state.keyframeIndex] || selectedLane().keyframes[0]; }
function presentation(attribute = state.laneAttribute) { return attributePresentation[attribute]; }
function clamp(value, minimum, maximum) { return Math.max(minimum, Math.min(maximum, value)); }
function normalizeDegrees(value) { return (value % 360 + 360) % 360; }
function phaseSpan() { return state.phaseEnd - state.phaseStart; }
function isFunctionLane(targetLane = selectedLane()) { return targetLane.mode !== "Keyframes"; }
function usesFunctionBounds(targetLane = selectedLane()) { return targetLane.mode === "Function max/min"; }
function isRandomType(type) { return type === "Random gate" || type === "Random timing" || type === "Random gate + timing"; }
function isRandomLane(targetLane) { return isFunctionLane(targetLane) && isRandomType(targetLane.functionConfig.type); }

function linearDirectionLabel(value = state.linearDirection) {
  const direction = normalizeDegrees(value);
  const labels = { 0: "left → right", 45: "top-left → bottom-right", 90: "top → bottom", 135: "top-right → bottom-left", 180: "right → left", 225: "bottom-right → top-left", 270: "bottom → top", 315: "bottom-left → top-right" };
  return labels[direction] || "custom diagonal";
}

function setPhaseSpan(value, announce = true) {
  const span = clamp(value, 0, 720);
  state.phaseEnd = state.phaseStart + span;
  if (announce) toast(`Phase span ${span}°: ${span > 360 ? "narrower, repeated wave" : span < 360 ? "broader wave" : "one wave across the grid"}.`);
}

function syncScaleBaseTime(targetLane, index) {
  targetLane.scaleBaseTimes[index] = targetLane.widthScale ? targetLane.keyframes[index].time / targetLane.widthScale * 100 : targetLane.keyframes[index].time;
}

function setLaneWidthScale(targetLane, value) {
  const scale = clamp(value, 25, 200);
  const movableCount = targetLane.keyframes.length - 1;
  targetLane.widthScale = scale;
  for (let index = 0; index < movableCount; index += 1) {
    const minimum = index === 0 ? 0 : targetLane.keyframes[index - 1].time + 1;
    const maximum = 100 - (movableCount - index);
    targetLane.keyframes[index].time = clamp(Math.round(targetLane.scaleBaseTimes[index] * scale / 100), minimum, maximum);
  }
  targetLane.keyframes.at(-1).time = 100;
}

function toast() {}

function relabelFrames(targetLane) {
  targetLane.keyframes.forEach((item, index) => {
    item.label = item.loop ? "A′" : String.fromCharCode(65 + index);
  });
}

function normalizedLaneValue(targetLane, value) {
  const info = attributePresentation[targetLane.attribute];
  return clamp((value - info.minimum) / (info.maximum - info.minimum), 0, 1);
}

function sourceValue(targetLane, source) {
  if (source.source === "Fixed" || source.source === "Current") return source.value;
  const preset = presetsForAttribute(targetLane.attribute).find((item) => item.name === source.preset);
  if (!preset) return source.value;
  if (!["Red", "Green", "Blue"].includes(targetLane.attribute)) return preset.value;
  return preset.rgb[{ Red: "r", Green: "g", Blue: "b" }[targetLane.attribute]] / 255 * 100;
}

function seededRandom(seed, iteration, index, stream = 0) {
  let value = seed ^ Math.imul(iteration + 1, 0x9e3779b1) ^ Math.imul(index + 1, 0x85ebca6b) ^ Math.imul(stream + 1, 0xc2b2ae35);
  value = Math.imul(value ^ value >>> 16, 0x7feb352d);
  value = Math.imul(value ^ value >>> 15, 0x846ca68b);
  return ((value ^ value >>> 16) >>> 0) / 4294967296;
}

function randomSourceFor(targetLane) {
  return randomSources[targetLane.functionConfig.random.sourceIndex] || randomSources[0];
}

function randomEventTimes(targetLane, iteration) {
  const config = targetLane.functionConfig.random;
  const seed = randomSourceFor(targetLane).seed;
  const countVariance = .65 + seededRandom(seed, iteration, 0, 10) * .7;
  const count = clamp(Math.round(config.density * countVariance), 1, 32);
  const grouping = config.grouping / 100;
  const pulse = config.pulseWidth / 100;
  const events = [];
  let eventIndex = 0;
  let clusterIndex = 0;
  while (eventIndex < count) {
    const burstSize = clamp(2 + Math.floor(seededRandom(seed, iteration, clusterIndex, 11) * 3), 1, count - eventIndex);
    const center = seededRandom(seed, iteration, clusterIndex, 12);
    for (let position = 0; position < burstSize && eventIndex < count; position += 1) {
      const scattered = seededRandom(seed, iteration, eventIndex, 13);
      const jitter = (seededRandom(seed, iteration, eventIndex, 14) - .5) * pulse * .5;
      const grouped = (center + position * pulse * .8 + jitter + 1) % 1;
      const difference = (grouped - scattered + 1.5) % 1 - .5;
      events.push((scattered + difference * grouping + 1) % 1);
      eventIndex += 1;
    }
    clusterIndex += 1;
  }
  return events.sort((left, right) => left - right);
}

function randomWave(targetLane, cycle, iteration) {
  const config = targetLane.functionConfig.random;
  const source = randomSourceFor(targetLane);
  const type = targetLane.functionConfig.type;
  if (cycle <= 0 || cycle >= 1) return -1;
  if (type === "Random gate") {
    const step = Math.min(config.density - 1, Math.floor(cycle * config.density));
    if (step === 0) return -1;
    const groupSize = 1 + Math.round(config.grouping / 100 * 3);
    const gateIndex = Math.floor(step / groupSize);
    const gateStart = step / config.density;
    if (cycle - gateStart >= config.pulseWidth / 100) return -1;
    return seededRandom(source.seed, iteration, gateIndex, 20) >= .5 ? 1 : -1;
  }
  const pulse = config.pulseWidth / 100;
  const events = randomEventTimes(targetLane, iteration);
  const eventIndex = events.findIndex((start) => cycle >= start && cycle - start < pulse);
  if (eventIndex < 0) return -1;
  if (type === "Random timing") return 1;
  return seededRandom(source.seed, iteration, eventIndex, 21) >= .5 ? 1 : -1;
}

function functionWave(targetLane, cycle, iteration = 0) {
  const type = targetLane.functionConfig.type;
  if (type === "Sinus") return Math.sin(cycle * Math.PI * 2);
  if (type === "Cosinus") return Math.cos(cycle * Math.PI * 2);
  if (type === "Linear +") return cycle * 2 - 1;
  if (type === "Linear −") return 1 - cycle * 2;
  if (isRandomType(type)) return randomWave(targetLane, cycle, iteration);
  if (type === "PWM") {
    const total = targetLane.pwm.on + targetLane.pwm.off || 1;
    const position = cycle * total;
    const attack = Math.min(targetLane.pwm.attack, targetLane.pwm.on);
    const decay = Math.min(targetLane.pwm.decay, targetLane.pwm.off);
    if (position <= attack) return attack ? -1 + position / attack * 2 : 1;
    if (position <= targetLane.pwm.on) return 1;
    if (position <= targetLane.pwm.on + decay) return 1 - (position - targetLane.pwm.on) / Math.max(1, decay) * 2;
    return -1;
  }
  return 0;
}

function functionValueAtLane(targetLane, percent, iteration = 0) {
  const exactCycleEnd = percent > 0 && Math.abs(percent % 100) < .0001;
  const transportCycle = exactCycleEnd ? 1 : ((percent / 100) % 1 + 1) % 1;
  const activeWidth = targetLane.functionWidth / 100;
  const cycle = transportCycle >= activeWidth ? 1 : transportCycle / Math.max(.01, activeWidth);
  const wave = functionWave(targetLane, cycle, iteration);
  const config = targetLane.functionConfig;
  let value;
  if (usesFunctionBounds(targetLane)) {
    const bottom = sourceValue(targetLane, config.bottom);
    const top = sourceValue(targetLane, config.top);
    value = bottom + (wave + 1) / 2 * (top - bottom);
  } else {
    value = sourceValue(targetLane, config.center) + wave * config.amplitude;
  }
  const info = attributePresentation[targetLane.attribute];
  return clamp(value, info.minimum, info.maximum);
}

function valueAtLane(targetLane, percent, iteration = 0) {
  if (isFunctionLane(targetLane)) return functionValueAtLane(targetLane, percent, iteration);
  const frames = targetLane.keyframes;
  if (percent <= frames[0].time) return frames[0].value;
  for (let index = 0; index < frames.length - 1; index += 1) {
    const left = frames[index];
    const right = frames[index + 1];
    if (percent > right.time) continue;
    const span = Math.max(.001, right.time - left.time);
    let local = clamp((percent - left.time) / span, 0, 1);
    const interpolation = left.interpolation || "Ease in + out";
    if (interpolation === "Ease in") local *= local;
    if (interpolation === "Ease out") local = 1 - (1 - local) ** 2;
    if (interpolation === "Ease in + out") local = (1 - Math.cos(local * Math.PI)) / 2;
    if (interpolation === "Hold") local = 0;
    if (interpolation === "Drop") local = local > 0 ? 1 : 0;
    return left.value + (right.value - left.value) * local;
  }
  return frames.at(-1).value;
}

function curvePath(targetLane, iteration = state.iteration) {
  const points = Array.from({ length: 61 }, (_, index) => {
    const percent = index / 60 * 100;
    const x = index * 10;
    const y = 61 - normalizedLaneValue(targetLane, valueAtLane(targetLane, percent, iteration)) * 49;
    return `${index ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`;
  });
  return points.join(" ");
}

function frameDisplay(targetLane, targetFrame) {
  if (targetFrame.source === "Preset") return targetFrame.preset;
  if (targetFrame.source === "Current") return "Current";
  return `${Math.round(targetFrame.value)}${attributePresentation[targetLane.attribute].unit}`;
}

function laneSummary(targetLane) {
  if (isFunctionLane(targetLane)) {
    const config = targetLane.functionConfig;
    if (usesFunctionBounds(targetLane)) return `${sourceDisplay(targetLane, config.bottom)} ↕ ${sourceDisplay(targetLane, config.top)}`;
    return `${sourceDisplay(targetLane, config.center)} ± ${Math.round(config.amplitude)}${attributePresentation[targetLane.attribute].unit}`;
  }
  const values = targetLane.keyframes.filter((item) => !item.loop).map((item) => frameDisplay(targetLane, item));
  const compact = values.length > 4 ? [values[0], values[1], "…", values.at(-1)] : values;
  return compact.join(" → ");
}

function randomConfigSummary(targetLane) {
  const config = targetLane.functionConfig.random;
  const source = randomSourceFor(targetLane);
  return `${targetLane.functionConfig.type} · ${config.density}/loop · group ${config.grouping}% · pulse ${config.pulseWidth}% · ${source.name} #${source.seed}`;
}

function sourceDisplay(targetLane, source) {
  if (source.source === "Preset") return source.preset;
  if (source.source === "Current") return "Current";
  return `${Math.round(source.value)}${attributePresentation[targetLane.attribute].unit}`;
}

function renderHeader() {
  const example = currentExample();
  $("#header-name").textContent = `${state.exampleIndex + 1} · ${example.name}`;
  $("#header-detail").textContent = `${example.lanes.length} lanes · ${state.ordering.toLowerCase()} fixture order`;
  const menu = $("#example-menu");
  menu.replaceChildren();
  examples.forEach((item, index) => {
    const button = document.createElement("button");
    button.classList.toggle("is-active", index === state.exampleIndex);
    button.innerHTML = `<i>${index + 1}</i><b>${item.name}</b><small>${item.description}</small>`;
    button.addEventListener("click", () => switchExample(index));
    menu.append(button);
  });
}

function renderLanes() {
  const host = $("#lane-scroller");
  host.replaceChildren();
  const selectionCount = selectedLanes().length;
  $("#lane-selection-summary").textContent = `${selectionCount} lane${selectionCount === 1 ? "" : "s"} selected · click selects one · Shift-click adds or removes`;
  const selectedModes = [...new Set(selectedLanes().map((item) => item.mode))];
  $$("[data-lane-mode]").forEach((button) => button.classList.toggle("is-active", selectedModes.length === 1 && button.dataset.laneMode === selectedModes[0]));
  currentExample().lanes.forEach((targetLane) => {
    const info = attributePresentation[targetLane.attribute];
    const path = curvePath(targetLane);
    const button = document.createElement("button");
    const isSelected = state.selectedAttributes.includes(targetLane.attribute);
    button.className = `attribute-lane${isSelected ? " is-active" : ""}${targetLane.attribute === state.laneAttribute ? " is-primary" : ""}`;
    button.setAttribute("aria-pressed", String(isSelected));
    button.dataset.lane = targetLane.attribute;
    button.dataset.color = info.color;
    const circles = targetLane.mode === "Keyframes" ? targetLane.keyframes.map((item, index) => {
      const x = item.time * 6;
      const y = 61 - normalizedLaneValue(targetLane, item.value) * 49;
      return `<circle cx="${x}" cy="${y}" r="5" class="${targetLane.attribute === state.laneAttribute && index === state.keyframeIndex ? "is-selected" : ""}"></circle>`;
    }).join("") : "";
    button.innerHTML = `
      <span class="lane-identity"><i class="attribute-mark"></i><b>${targetLane.attribute}</b><small>${targetLane.mode}${isFunctionLane(targetLane) ? ` · ${targetLane.functionConfig.type}` : ""}${isSelected ? " · selected" : ""}</small></span>
      <span class="lane-curve" aria-label="${targetLane.attribute} scalar curve">
        <svg viewBox="0 0 600 72" preserveAspectRatio="none" aria-hidden="true">
          <path class="curve-fill" d="${path} L600 72 L0 72 Z"></path><path class="curve-line" d="${path}"></path>
          <g class="keyframe-marks">${circles}</g><g class="phase-marks"></g><line class="lane-playhead" x1="0" x2="0" y1="0" y2="72"></line>
        </svg><span class="axis-start">0%</span><span class="axis-middle">50%</span><span class="axis-end">100%</span>
      </span>
      <span class="lane-summary"><b>${laneSummary(targetLane)}</b><small>${isFunctionLane(targetLane) ? isRandomType(targetLane.functionConfig.type) ? randomConfigSummary(targetLane) : `${targetLane.functionConfig.type} · width ${targetLane.functionWidth}%${targetLane.functionConfig.type === "PWM" ? ` · A${targetLane.pwm.attack}/O${targetLane.pwm.on}/D${targetLane.pwm.decay}/F${targetLane.pwm.off}` : ""}` : `${targetLane.keyframes[state.keyframeIndex]?.interpolation || "Ease in + out"} · width ${targetLane.widthScale}%`}</small><small class="lane-speed-readout">Speed ${targetLane.multiplier}</small></span>`;
    button.addEventListener("click", (event) => selectLane(targetLane.attribute, event.shiftKey));
    host.append(button);
  });
  renderPhaseMarksOnCurves();
}

function renderKeyframes() {
  const targetLane = selectedLane();
  const isKeyframed = targetLane.mode === "Keyframes";
  $("#keyframe-editor").hidden = !isKeyframed;
  $("#function-editor").hidden = isKeyframed;
  if (!isKeyframed) return renderFunctionEditor();
  $("#keyframe-lane-name").textContent = `${targetLane.attribute} keyframes${selectedLanes().length > 1 ? ` · ${selectedLanes().length} lanes selected` : ""}`;
  const host = $("#keyframe-list");
  host.replaceChildren();
  targetLane.keyframes.forEach((item, index) => {
    const button = document.createElement("button");
    button.className = `keyframe-chip${index === state.keyframeIndex ? " is-active" : ""}${item.loop ? " loop" : ""}`;
    button.innerHTML = `<b>${item.label}</b><span>${Math.round(item.time)}%</span><small>${item.loop ? "Loop · alias A" : `${item.source} · ${frameDisplay(targetLane, item)}`}</small>`;
    button.addEventListener("click", () => {
      state.keyframeIndex = index;
      renderAll(false);
      toast(`${targetLane.attribute} keyframe ${item.label} selected.`);
    });
    host.append(button);
  });
  const selected = selectedFrame();
  $("#source-detail").textContent = selected.loop ? "Loop · alias of A" : `${selected.source} · ${frameDisplay(targetLane, selected)}`;
}

function renderFunctionEditor() {
  const targetLane = selectedLane();
  const config = targetLane.functionConfig;
  const bounds = usesFunctionBounds(targetLane);
  const isRandom = isRandomType(config.type);
  const random = config.random;
  const controls = [
    ["Turn selection · P Mode · Push menu", config.type, 1],
    [bounds ? "Top" : "Middle", sourceDisplay(targetLane, bounds ? config.top : config.center), 2],
    [bounds ? "Bottom" : "Amplitude", bounds ? sourceDisplay(targetLane, config.bottom) : `${Math.round(config.amplitude)}${attributePresentation[targetLane.attribute].unit}`, 3],
    [config.type === "PWM" ? "Attack · P On" : isRandom ? "Pulse · P Grouping" : "Parameter", config.type === "PWM" ? `${targetLane.pwm.attack}% · ${targetLane.pwm.on}%` : isRandom ? `${random.pulseWidth}% · ${random.grouping}%` : "—", 4],
    [config.type === "PWM" ? "Decay · P Off" : isRandom ? "Density · P Source" : "Parameter", config.type === "PWM" ? `${targetLane.pwm.decay}% · ${targetLane.pwm.off}%` : isRandom ? `${random.density}/loop · ${randomSourceFor(targetLane).name}` : "—", 5],
    ["Speed · P Width", `${targetLane.multiplier} · ${targetLane.functionWidth}%`, 6],
  ];
  const host = $("#function-editor");
  host.innerHTML = controls.map(([label, value, encoder]) => `<button class="function-chip" data-function-encoder="${encoder}"><small>${label}</small><b>${value}</b></button>`).join("");
  if (config.type !== "PWM" && !isRandom) {
    $("[data-function-encoder=\"4\"]", host).disabled = true;
    $("[data-function-encoder=\"5\"]", host).disabled = true;
  }
  $$("[data-function-encoder]", host).forEach((button) => button.addEventListener("click", () => {
    const encoder = Number(button.dataset.functionEncoder);
    state.encoder = encoder;
    if (encoder === 1) openModeModal();
    else if (encoder === 2) openFunctionSourceModal(bounds ? "top" : "center");
    else if (encoder === 3 && bounds) openFunctionSourceModal("bottom");
    else if (encoder === 3) { selectedLanes().forEach((item) => { item.functionConfig.amplitude = clamp(item.functionConfig.amplitude + 1, 0, attributePresentation[item.attribute].maximum - attributePresentation[item.attribute].minimum); }); renderAll(false); }
    else if (encoder === 4 && config.type === "PWM") { adjustPwm("attack", 1, false); renderAll(false); }
    else if (encoder === 5 && config.type === "PWM") { adjustPwm("decay", 1, false); renderAll(false); }
    else if (encoder === 4 && isRandom) { adjustRandom("pulseWidth", 1, false); renderAll(false); }
    else if (encoder === 5 && isRandom) { adjustRandom("density", 1, false); renderAll(false); }
    else if (encoder === 6) { const multiplier = cycleOption(multiplierOptions, targetLane.multiplier, 1); selectedLanes().forEach((item) => { item.multiplier = multiplier; }); renderAll(false); }
  }));
}

function renderPhaseMarksOnCurves() {
  $$(".attribute-lane").forEach((laneNode) => {
    const path = $(".curve-line", laneNode);
    const marks = $(".phase-marks", laneNode);
    if (!path || !marks) return;
    const length = path.getTotalLength();
    marks.replaceChildren();
    for (let index = 0; index < 100; index += 1) {
      const point = path.getPointAtLength(fixturePhase(index, state.ordering) * length);
      const circle = document.createElementNS(svgNamespace, "circle");
      circle.setAttribute("cx", point.x); circle.setAttribute("cy", point.y); circle.setAttribute("r", index % 10 === 0 ? "3" : "1.5");
      marks.append(circle);
    }
  });
}

function renderPhaseView() {
  const host = $("#phase-all-lanes");
  host.replaceChildren();
  currentExample().lanes.forEach((targetLane) => {
    const info = attributePresentation[targetLane.attribute];
    const pathData = curvePath(targetLane);
    const article = document.createElement("article");
    article.className = "phase-overview-lane";
    article.dataset.color = info.color;
    article.innerHTML = `<span class="phase-overview-identity"><i class="attribute-mark"></i><b>${targetLane.attribute}</b><small>${isFunctionLane(targetLane) ? targetLane.functionConfig.type : targetLane.keyframes[0].interpolation}</small></span><span class="phase-overview-curve"><svg viewBox="0 0 600 72" preserveAspectRatio="none" role="img" aria-label="${targetLane.attribute} curve with 100 shared fixture phases"><path class="phase-overview-fill" d="${pathData} L600 72 L0 72 Z"></path><path class="phase-overview-line" d="${pathData}"></path><g class="shared-phase-marks"></g><line class="lane-playhead" x1="0" x2="0" y1="0" y2="72"></line></svg><span class="axis-start">0°</span><span class="axis-middle">180°</span><span class="axis-end">360°</span></span><span class="phase-overview-summary"><b>Shared phase</b><small>100 fixtures</small></span>`;
    host.append(article);
    const path = $(".phase-overview-line", article);
    const marks = $(".shared-phase-marks", article);
    const length = path.getTotalLength();
    for (let index = 0; index < 100; index += 1) {
      const fixturePosition = state.phaseStart / 360 + fixturePhase(index, state.ordering) * phaseSpan() / 360 + state.phaseOffset / 360;
      const point = path.getPointAtLength(((fixturePosition % 1 + 1) % 1) * length);
      const circle = document.createElementNS(svgNamespace, "circle");
      circle.setAttribute("cx", point.x); circle.setAttribute("cy", point.y); circle.setAttribute("r", index % 10 === 0 ? "3" : "1.35");
      marks.append(circle);
    }
  });
  $("#phase-summary-title").textContent = `Origin ${state.phaseStart}° · span ${phaseSpan()}°`;
  $("#phase-summary-detail").textContent = `100 fixtures · ${state.ordering.toLowerCase()}${state.ordering === "Linear" ? ` · ${state.linearDirection}°` : ""}`;
  $("#ordering-explanation").textContent = state.ordering === "Linear" ? `Grid linear runs ${linearDirectionLabel()} at ${state.linearDirection}°.` : orderingDescriptions[state.ordering];
  $("#linear-direction-control").hidden = state.ordering !== "Linear";
  $("#linear-direction-value").textContent = `${state.linearDirection}° · ${linearDirectionLabel()}`;
  $$("[data-ordering]").forEach((button) => button.classList.toggle("is-active", button.dataset.ordering === state.ordering));
  $$("[data-phase-span]").forEach((button) => button.classList.toggle("is-active", Number(button.dataset.phaseSpan) === phaseSpan()));
}

function renderSpeedView() {
  const group = activeSpeedGroup();
  $("#speed-source-label").textContent = group ? group.name : "Fixed duration";
  $("#speed-primary-value").textContent = group ? `${group.bpm} BPM` : `${state.cycleSeconds.toFixed(1)} s`;
  $("#speed-secondary-value").textContent = group ? `four beats · ${currentCycleSeconds(false).toFixed(2)} s per cycle` : "per complete cycle";
  $("#overall-speed-value").textContent = state.overallMultiplier;
  $("#quantize-value").textContent = state.quantize;
  $("#start-policy-value").textContent = state.startPolicy;
  $("#fixed-speed-choice").classList.toggle("is-active", !group);
  $("#fixed-speed-choice-value").textContent = `${state.cycleSeconds.toFixed(1)} s`;
  const host = $("#speed-group-list");
  host.replaceChildren();
  speedGroups.forEach((item, index) => {
    const button = document.createElement("button");
    button.className = `speed-group-choice${state.speedGroupIndex === index ? " is-active" : ""}`;
    button.innerHTML = `<b>${item.name}</b><span>${item.bpm} BPM</span><small>4 beats · ${(240 / item.bpm).toFixed(2)} s</small>`;
    button.addEventListener("click", () => selectSpeedGroup(index));
    host.append(button);
  });
}

function renderGridStructure() {
  const host = $("#fixture-grid");
  host.replaceChildren();
  for (let index = 0; index < 100; index += 1) {
    const button = document.createElement("button");
    button.className = "fixture-cell";
    button.innerHTML = `<span class="fixture-number">${index + 1}</span><i class="position-dot" aria-hidden="true"></i>`;
    button.setAttribute("aria-label", `Fixture ${index + 1}, row ${Math.floor(index / 10) + 1}, column ${index % 10 + 1}`);
    button.addEventListener("click", () => { state.selectedFixture = index; updateGrid(state.progress); });
    host.append(button);
  }
}

let randomFixtureCache = { key: "", ranks: [] };

function randomFixtureRank(index, iteration) {
  const seed = randomSources[0].seed;
  const key = `${seed}:${iteration}`;
  if (randomFixtureCache.key !== key) {
    const order = Array.from({ length: 100 }, (_, fixture) => fixture).sort((left, right) => seededRandom(seed, iteration, left, 90) - seededRandom(seed, iteration, right, 90));
    const ranks = Array(100);
    order.forEach((fixture, rank) => { ranks[fixture] = rank / 100; });
    randomFixtureCache = { key, ranks };
  }
  return randomFixtureCache.ranks[index];
}

function fixturePhase(index, ordering, iteration = state.iteration) {
  const row = Math.floor(index / 10);
  const column = index % 10;
  const dx = column - 4.5;
  const dy = row - 4.5;
  let base;
  if (ordering === "Selection") base = index / 100;
  else if (ordering === "Random each loop") base = randomFixtureRank(index, iteration);
  else if (ordering === "Linear") {
    const radians = state.linearDirection * Math.PI / 180;
    const x = column / 9 - .5;
    const y = row / 9 - .5;
    const projectionRange = Math.abs(Math.cos(radians)) + Math.abs(Math.sin(radians));
    base = (x * Math.cos(radians) + y * Math.sin(radians)) / projectionRange + .5;
  }
  const radial = Math.hypot(dx, dy) / Math.hypot(4.5, 4.5);
  if (ordering === "Radial out") base = radial;
  if (ordering === "Radial in") base = 1 - radial;
  if (ordering === "Axial") base = (Math.atan2(dx, -dy) + Math.PI * 2) % (Math.PI * 2) / (Math.PI * 2);
  const blockCount = Math.max(1, Math.ceil(100 / state.phaseBlock));
  const blocked = Math.floor(clamp(base ?? 0, 0, .9999) * blockCount) / blockCount;
  return blocked * state.phaseRepeats % 1;
}

function multiplierFactor(multiplier) {
  const amount = Number(multiplier.slice(2));
  return multiplier.startsWith("×") ? amount : 1 / amount;
}

function activeSpeedGroup() { return state.speedGroupIndex >= 0 ? speedGroups[state.speedGroupIndex] : null; }

function currentCycleSeconds(includeOverall = true) {
  const group = activeSpeedGroup();
  const base = group ? 240 / group.bpm : state.cycleSeconds;
  return includeOverall ? base / multiplierFactor(state.overallMultiplier) : base;
}

function attributeValue(attribute, absoluteCycle, fallback) {
  const targetLane = currentExample().lanes.find((item) => item.attribute === attribute);
  const scaledCycle = absoluteCycle * multiplierFactor(targetLane?.multiplier || "× 1");
  const iteration = Math.floor(scaledCycle);
  const localPercent = ((scaledCycle % 1 + 1) % 1) * 100;
  return targetLane ? valueAtLane(targetLane, localPercent, iteration) : fallback;
}

function nearestColorName(r, g, b) {
  let best = colorPresets[0];
  let distance = Infinity;
  colorPresets.forEach((preset) => {
    const current = (preset.rgb.r - r) ** 2 + (preset.rgb.g - g) ** 2 + (preset.rgb.b - b) ** 2;
    if (current < distance) { distance = current; best = preset; }
  });
  return best.name.toLowerCase();
}

function fixtureLook(index, progress) {
  const spread = (state.phaseEnd - state.phaseStart) / 360;
  const phase = state.phaseStart / 360 + fixturePhase(index, state.ordering, state.iteration) * spread + state.phaseOffset / 360;
  const absoluteLocal = state.iteration + progress - phase;
  const local = ((absoluteLocal % 1) + 1) % 1;
  const intensity = clamp(attributeValue("Intensity", absoluteLocal, 100), 0, 100);
  const r = clamp(attributeValue("Red", absoluteLocal, 100), 0, 100) / 100 * 255;
  const g = clamp(attributeValue("Green", absoluteLocal, 100), 0, 100) / 100 * 255;
  const b = clamp(attributeValue("Blue", absoluteLocal, 100), 0, 100) / 100 * 255;
  const pan = clamp(attributeValue("Pan", absoluteLocal, 0), -180, 180);
  const tilt = clamp(attributeValue("Tilt", absoluteLocal, 0), -90, 90);
  const hex = `#${[r, g, b].map((value) => Math.round(value).toString(16).padStart(2, "0")).join("")}`;
  return { intensity, r, g, b, pan, tilt, hex, local: local * 100 };
}

function updateGrid(progress) {
  const cells = $$(".fixture-cell");
  cells.forEach((cell, index) => {
    const look = fixtureLook(index, progress);
    cell.style.setProperty("--fixture-color", look.hex);
    cell.style.setProperty("--fixture-level", (look.intensity / 100).toFixed(2));
    cell.style.setProperty("--pan-x", `${10 + (look.pan + 180) / 360 * 80}%`);
    cell.style.setProperty("--tilt-y", `${10 + (1 - (look.tilt + 90) / 180) * 80}%`);
    const level = look.intensity / 100;
    cell.style.backgroundColor = `rgb(${Math.round(look.r * level * .9 + 7)} ${Math.round(look.g * level * .9 + 9)} ${Math.round(look.b * level * .9 + 12)})`;
    cell.dataset.level = `${Math.round(look.intensity)}%`;
    cell.classList.toggle("is-selected", index === state.selectedFixture);
    cell.setAttribute("aria-label", `Fixture ${index + 1}: intensity ${Math.round(look.intensity)} percent, color ${nearestColorName(look.r, look.g, look.b)}, pan ${Math.round(look.pan)} degrees, tilt ${Math.round(look.tilt)} degrees`);
  });
  const selected = fixtureLook(state.selectedFixture, progress);
  $("#fixture-detail").innerHTML = `<b>Fixture ${state.selectedFixture + 1}</b><span>Intensity ${Math.round(selected.intensity)}%</span><span>Color ${nearestColorName(selected.r, selected.g, selected.b)} · ${selected.hex}</span><span>Pan ${Math.round(selected.pan)}° · Tilt ${Math.round(selected.tilt)}°</span>`;
  $("#grid-time").textContent = `L${state.iteration + 1} · ${Math.round(progress * 100)}%`;
  $("#grid-order-label").textContent = `${state.ordering === "Axial" ? "Axial radar" : state.ordering} · 10×10`;
  $("#target-summary").textContent = `100 fixtures · ${state.ordering.toLowerCase()}`;
}

function bindRepeatingTurn(button, action) {
  let delayTimer;
  let repeatTimer;
  let repeated = false;
  const stop = () => {
    clearTimeout(delayTimer);
    clearInterval(repeatTimer);
  };
  button.addEventListener("pointerdown", (event) => {
    if (event.button !== undefined && event.button !== 0) return;
    repeated = false;
    delayTimer = setTimeout(() => {
      repeated = true;
      action();
      repeatTimer = setInterval(action, 85);
    }, 380);
    const release = () => {
      stop();
      document.removeEventListener("pointerup", release);
      document.removeEventListener("pointercancel", release);
    };
    document.addEventListener("pointerup", release);
    document.addEventListener("pointercancel", release);
  });
  button.addEventListener("click", (event) => {
    stop();
    if (repeated) {
      repeated = false;
      event.preventDefault();
      return;
    }
    action();
  });
}

function renderEncoderCards() {
  const host = $("#encoder-bank");
  host.replaceChildren();
  const configs = encoderConfigs();
  configs.forEach((config, index) => {
    const number = index + 1;
    const card = document.createElement("article");
    card.className = `encoder-card${number === state.encoder ? " is-active" : ""}${config.unassigned ? " is-unassigned" : ""}`;
    card.dataset.encoder = String(number);
    card.innerHTML = `<span class="encoder-number">${number}</span><span class="encoder-label">${config.label}</span><strong class="encoder-value">${config.value}</strong><div class="encoder-control"><button class="pushed-turn" data-pushed-turn="-1" aria-label="Pushed turn encoder ${number} left">−P</button><button data-turn="-1" aria-label="Turn encoder ${number} left">−</button><button class="encoder-push" data-push aria-label="Push encoder ${number}">Push</button><button data-turn="1" aria-label="Turn encoder ${number} right">+</button><button class="pushed-turn" data-pushed-turn="1" aria-label="Pushed turn encoder ${number} right">P+</button></div>`;
    $(".encoder-push", card).style.setProperty("--knob-turn", `${-35 + state.knobTurns[index] * 11}deg`);
    $$('button', card).forEach((button) => { button.disabled = Boolean(config.unassigned); });
    host.append(card);
  });
  $$('[data-turn], [data-pushed-turn]').forEach((button) => {
    const card = button.closest(".encoder-card");
    const pushed = button.hasAttribute("data-pushed-turn");
    const direction = Number(pushed ? button.dataset.pushedTurn : button.dataset.turn);
    bindRepeatingTurn(button, () => handleEncoderTurn(Number(card.dataset.encoder), direction, pushed));
  });
  $$('[data-push]').forEach((button) => button.addEventListener("click", () => handleEncoderPush(Number(button.closest(".encoder-card").dataset.encoder), button)));
}

function encoderConfigs() {
  if (state.view === "phase") {
    $("#encoder-context").textContent = `Shared spread · ${state.ordering}`;
    return [
      { label: "Phase origin", value: `${state.phaseStart}°` }, { label: "Phase span", value: `${phaseSpan()}°` },
      { label: "Block size", value: String(state.phaseBlock) }, { label: "Repeats", value: String(state.phaseRepeats) },
      { label: "Fixture order", value: state.ordering }, state.ordering === "Linear" ? { label: "Direction", value: `${state.linearDirection}° · ${linearDirectionLabel()}` } : { label: "Phase offset", value: `${state.phaseOffset}°` },
    ];
  }
  if (state.view === "speed") {
    const group = activeSpeedGroup();
    $("#encoder-context").textContent = "Shared cycle speed and transport";
    return [
      { label: "Speed source", value: group ? group.name : "Fixed" },
      { label: group ? "Group tempo" : "Duration", value: group ? `${group.bpm} BPM` : `${state.cycleSeconds.toFixed(1)} s` },
      { label: "Overall", value: state.overallMultiplier }, { label: "Quantize", value: state.quantize },
      { label: "Start policy", value: state.startPolicy }, { label: "Cycle offset", value: `${state.transportOffset}°` },
    ];
  }
  const targetLane = selectedLane();
  const targets = selectedLanes();
  if (isFunctionLane(targetLane)) {
    const config = targetLane.functionConfig;
    const bounds = usesFunctionBounds(targetLane);
    const isRandom = isRandomType(config.type);
    const random = config.random;
    $("#encoder-context").textContent = `${targets.length} lane${targets.length === 1 ? "" : "s"} selected · primary ${targetLane.attribute} · ${targetLane.mode}`;
    return [
      { label: "Selection · P Mode", value: `${config.type} · ${targetLane.mode}` },
      { label: bounds ? "Top" : "Middle", value: `${(bounds ? config.top : config.center).source} · ${sourceDisplay(targetLane, bounds ? config.top : config.center)}` },
      { label: bounds ? "Bottom" : "Amplitude", value: bounds ? `${config.bottom.source} · ${sourceDisplay(targetLane, config.bottom)}` : `${Math.round(config.amplitude)}${presentation().unit}` },
      config.type === "PWM" ? { label: "Attack · P On", value: `${targetLane.pwm.attack}% · ${targetLane.pwm.on}%` } : isRandom ? { label: "Pulse · P Grouping", value: `${random.pulseWidth}% · ${random.grouping}%` } : { label: "Unassigned", value: "—", unassigned: true },
      config.type === "PWM" ? { label: "Decay · P Off", value: `${targetLane.pwm.decay}% · ${targetLane.pwm.off}%` } : isRandom ? { label: "Density · P Source", value: `${random.density}/loop · ${randomSourceFor(targetLane).name}` } : { label: "Unassigned", value: "—", unassigned: true },
      { label: "Speed · P Width", value: `${targetLane.multiplier} · ${targetLane.functionWidth}%` },
    ];
  }
  const targetFrame = selectedFrame();
  $("#encoder-context").textContent = `${targets.length} lane${targets.length === 1 ? "" : "s"} selected · primary ${targetLane.attribute} · keyframe ${targetFrame.label}`;
  const value = targetFrame.source === "Preset" ? targetFrame.preset : targetFrame.source === "Current" ? "Current" : `${Math.round(targetFrame.value)}${presentation().unit}`;
  return [
    { label: "Keyframe · P Mode", value: `${targetFrame.label} · ${state.keyframeIndex + 1}/${targetLane.keyframes.length}` },
    { label: `${targetFrame.source} value`, value }, { label: "Keyframe time", value: `${Math.round(targetFrame.time)}%` },
    { label: "Interpolation", value: targetFrame.interpolation || "Ease in + out" },
    { label: "Unassigned", value: "—", unassigned: true },
    { label: "Speed · P Width", value: `${targetLane.multiplier} · ${targetLane.widthScale}%` },
  ];
}

function cycleOption(options, current, direction) {
  const index = Math.max(0, options.indexOf(current));
  return options[(index + direction + options.length) % options.length];
}

function setSelectedLaneMode(mode, announce = true) {
  selectedLanes().forEach((targetLane) => { targetLane.mode = mode; });
  state.functionSourceField = null;
  if (announce) toast(`${selectedLanes().length} lane${selectedLanes().length === 1 ? "" : "s"} set to ${mode.toLowerCase()} mode; the other configuration is preserved.`);
  renderAll(false);
}

function cycleFunctionType(direction) {
  const type = cycleOption(functionTypes, selectedLane().functionConfig.type, direction);
  selectedLanes().forEach((targetLane) => { targetLane.functionConfig.type = type; });
}

function adjustFunctionSource(field, direction, pushed) {
  const targetLane = selectedLane();
  const source = targetLane.functionConfig[field];
  if (pushed) {
    source.source = cycleOption(sourceOptions, source.source, direction);
    if (source.source === "Preset") {
      const preset = presetsForAttribute(targetLane.attribute)[0];
      source.preset = preset.name;
      source.value = sourceValue(targetLane, source);
    }
    return;
  }
  if (source.source === "Preset") {
    const presets = presetsForAttribute(targetLane.attribute);
    const current = Math.max(0, presets.findIndex((item) => item.name === source.preset));
    const preset = presets[(current + direction + presets.length) % presets.length];
    source.preset = preset.name;
    source.value = sourceValue(targetLane, source);
  } else if (source.source === "Fixed") {
    const info = attributePresentation[targetLane.attribute];
    source.value = clamp(source.value + direction * (["Pan", "Tilt"].includes(targetLane.attribute) ? 5 : 1), info.minimum, info.maximum);
  } else toast("Current follows the upstream programmer value; pushed-turn changes the source.");
}

function cycleSpeedSource(direction) {
  const sources = [-1, ...speedGroups.map((_, index) => index)];
  const current = Math.max(0, sources.indexOf(state.speedGroupIndex));
  const next = sources[(current + direction + sources.length) % sources.length];
  selectSpeedGroup(next, false);
}

function selectSpeedGroup(index, announce = true) {
  state.speedGroupIndex = index;
  if (index >= 0) state.lastSpeedGroupIndex = index;
  state.progress = 0;
  state.iteration = 0;
  state.previewStart = 0;
  renderAll();
  if (announce) toast(index >= 0 ? `${speedGroups[index].name}: ${speedGroups[index].bpm} BPM.` : `Fixed duration: ${state.cycleSeconds.toFixed(1)} seconds.`);
}

function presetsForAttribute(attribute) {
  if (attribute === "Intensity") return intensityPresets;
  if (attribute === "Pan" || attribute === "Tilt") return positionPresets[attribute];
  return colorPresets;
}

function applyPreset(targetLane, targetFrame, preset) {
  targetFrame.source = "Preset";
  targetFrame.preset = preset.name;
  if (["Red", "Green", "Blue"].includes(targetLane.attribute)) {
    const channelMap = { Red: "r", Green: "g", Blue: "b" };
    ["Red", "Green", "Blue"].forEach((attribute) => {
      const colorLaneTarget = currentExample().lanes.find((item) => item.attribute === attribute);
      if (!colorLaneTarget) return;
      const matching = colorLaneTarget.keyframes.find((item) => item.time === targetFrame.time) || colorLaneTarget.keyframes[state.keyframeIndex];
      if (!matching || matching.loop) return;
      matching.source = "Preset"; matching.preset = preset.name; matching.value = Math.round(preset.rgb[channelMap[attribute]] / 255 * 100);
    });
  } else {
    targetFrame.value = preset.value;
  }
}

function cycleFramePreset(direction) {
  const targetLane = selectedLane();
  const targetFrame = selectedFrame();
  const presets = presetsForAttribute(targetLane.attribute);
  const current = Math.max(0, presets.findIndex((item) => item.name === targetFrame.preset));
  applyPreset(targetLane, targetFrame, presets[(current + direction + presets.length) % presets.length]);
}

function cycleFrameSource(direction) {
  const targetFrame = selectedFrame();
  if (targetFrame.loop) return;
  targetFrame.source = cycleOption(sourceOptions, targetFrame.source, direction);
  if (targetFrame.source === "Preset") {
    const first = presetsForAttribute(selectedLane().attribute)[0];
    applyPreset(selectedLane(), targetFrame, first);
  }
}

function snapKeyframe(direction) {
  const targetLane = selectedLane();
  const targetFrame = selectedFrame();
  const neighbor = targetLane.keyframes[state.keyframeIndex + direction];
  if (targetFrame.loop || !neighbor) return toast(`No ${direction > 0 ? "next" : "previous"} keyframe to snap to.`);
  targetFrame.time = neighbor.time;
  syncScaleBaseTime(targetLane, state.keyframeIndex);
  toast(`${targetFrame.label} snapped to ${neighbor.label} at ${Math.round(targetFrame.time)}%.`);
}

function centerKeyframe() {
  const targetLane = selectedLane();
  const targetFrame = selectedFrame();
  const previous = targetLane.keyframes[state.keyframeIndex - 1];
  const next = targetLane.keyframes[state.keyframeIndex + 1];
  if (targetFrame.loop || !previous || !next) return toast("This keyframe needs neighbors on both sides to be centered.");
  targetFrame.time = Math.round((previous.time + next.time) / 2);
  syncScaleBaseTime(targetLane, state.keyframeIndex);
  toast(`${targetFrame.label} centered at ${targetFrame.time}%.`);
}

function insertKeyframe() {
  const targetLane = selectedLane();
  const frames = targetLane.keyframes;
  const loopIndex = frames.findIndex((item) => item.loop);
  const left = frames[Math.max(0, loopIndex - 1)];
  const time = Math.round((left.time + frames[loopIndex].time) / 2);
  frames.splice(loopIndex, 0, frame(time, "Current", left.value, left.preset));
  targetLane.scaleBaseTimes.splice(loopIndex, 0, time / targetLane.widthScale * 100);
  relabelFrames(targetLane);
  state.keyframeIndex = loopIndex;
  renderAll(false);
  toast(`Inserted keyframe ${selectedFrame().label} at ${time}% using Current.`);
}

function adjustRandom(field, direction) {
  const primary = selectedLane();
  selectedLanes().filter(isRandomLane).forEach((item) => {
    const random = item.functionConfig.random;
    if (field === "density") random.density = clamp(random.density + direction, 1, 32);
    if (field === "grouping") random.grouping = clamp(random.grouping + direction * 5, 0, 100);
    if (field === "pulseWidth") random.pulseWidth = clamp(random.pulseWidth + direction, 1, 50);
    if (field === "source") random.sourceIndex = (random.sourceIndex + direction + randomSources.length) % randomSources.length;
  });
  toast(`Random ${field}: ${randomConfigSummary(primary)}.`);
}

function reseedRandomSource(targetLane) {
  const source = randomSourceFor(targetLane);
  source.seed = 100 + Math.floor(Math.random() * 9900);
  toast(`${source.name} reseeded to #${source.seed}; every linked lane keeps using the same random stream.`);
}

function adjustPwm(field, direction, pushed) {
  const targetLane = selectedLane();
  selectedLanes().filter((item) => isFunctionLane(item) && item.functionConfig.type === "PWM").forEach((item) => {
    const step = direction * (pushed ? 5 : 1);
    if (field === "attack") item.pwm.attack = clamp(item.pwm.attack + step, 0, item.pwm.on);
    if (field === "decay") item.pwm.decay = clamp(item.pwm.decay + step, 0, item.pwm.off);
    if (field === "on") {
      item.pwm.on = clamp(item.pwm.on + step, item.pwm.attack, 100 - item.pwm.decay);
      item.pwm.off = 100 - item.pwm.on;
    }
    if (field === "off") {
      item.pwm.off = clamp(item.pwm.off + step, item.pwm.decay, 100 - item.pwm.attack);
      item.pwm.on = 100 - item.pwm.off;
    }
  });
  toast(`PWM ${field}: ${targetLane.pwm[field]}%.`);
}

function handleEncoderTurn(number, direction, pushed) {
  state.encoder = number;
  state.knobTurns[number - 1] += direction;
  if (state.view === "phase") {
    const step = pushed ? 45 : 15;
    if (number === 1) {
      const span = phaseSpan();
      state.phaseStart = clamp(state.phaseStart + direction * step, -360, 360);
      state.phaseEnd = state.phaseStart + span;
    }
    if (number === 2) setPhaseSpan(phaseSpan() + direction * step, false);
    if (number === 3) state.phaseBlock = clamp(state.phaseBlock + direction * (pushed ? 5 : 1), 1, 100);
    if (number === 4) state.phaseRepeats = clamp(state.phaseRepeats + direction, 1, 10);
    if (number === 5) setOrdering(cycleOption(orderingModes, state.ordering, direction), false);
    if (number === 6) {
      if (state.ordering === "Linear") state.linearDirection = normalizeDegrees(state.linearDirection + direction * (pushed ? 45 : 5));
      else state.phaseOffset = clamp(state.phaseOffset + direction * step, -360, 360);
    }
    renderAll(false); return;
  }
  if (state.view === "speed") {
    if (number === 1) cycleSpeedSource(direction);
    if (number === 2) {
      const group = activeSpeedGroup();
      if (group) group.bpm = clamp(group.bpm + direction * (pushed ? 5 : 1), 20, 300);
      else state.cycleSeconds = Math.round(clamp(state.cycleSeconds + direction * (pushed ? 1 : .1), .1, 30) * 10) / 10;
      state.previewStart = 0;
    }
    if (number === 3) state.overallMultiplier = cycleOption(multiplierOptions, state.overallMultiplier, direction);
    if (number === 4) state.quantize = cycleOption(quantizeOptions, state.quantize, direction);
    if (number === 5) state.startPolicy = cycleOption(startPolicies, state.startPolicy, direction);
    if (number === 6) state.transportOffset = clamp(state.transportOffset + direction * (pushed ? 45 : 15), -360, 360);
    renderAll(false); return;
  }
  const targetLane = selectedLane();
  if (number === 1 && pushed) {
    const mode = cycleOption(laneModes, targetLane.mode, direction);
    selectedLanes().forEach((item) => { item.mode = mode; });
    state.functionSourceField = null;
    renderAll(false); return;
  }
  if (isFunctionLane(targetLane)) {
    const config = targetLane.functionConfig;
    if (isRandomType(config.type)) {
      if (number === 1) cycleFunctionType(direction);
      if (number === 2) adjustFunctionSource(usesFunctionBounds(targetLane) ? "top" : "center", direction, pushed);
      if (number === 3) {
        if (usesFunctionBounds(targetLane)) adjustFunctionSource("bottom", direction, pushed);
        else selectedLanes().forEach((item) => {
          const info = attributePresentation[item.attribute];
          item.functionConfig.amplitude = clamp(item.functionConfig.amplitude + direction * (pushed ? 5 : 1), 0, info.maximum - info.minimum);
        });
      }
      if (number === 4) adjustRandom(pushed ? "grouping" : "pulseWidth", direction);
      if (number === 5) adjustRandom(pushed ? "source" : "density", direction);
      if (number === 6) {
        if (pushed) selectedLanes().filter(isFunctionLane).forEach((item) => { item.functionWidth = clamp(item.functionWidth + direction * 5, 5, 100); });
        else {
          const multiplier = cycleOption(multiplierOptions, targetLane.multiplier, direction);
          selectedLanes().forEach((item) => { item.multiplier = multiplier; });
        }
      }
      renderAll(false); return;
    }
    if (number === 1) cycleFunctionType(direction);
    if (number === 2) adjustFunctionSource(usesFunctionBounds(targetLane) ? "top" : "center", direction, pushed);
    if (number === 3) {
      if (usesFunctionBounds(targetLane)) adjustFunctionSource("bottom", direction, pushed);
      else selectedLanes().forEach((item) => {
        const info = attributePresentation[item.attribute];
        item.functionConfig.amplitude = clamp(item.functionConfig.amplitude + direction * (pushed ? 5 : 1), 0, info.maximum - info.minimum);
      });
    }
    if (config.type === "PWM" && number === 4) adjustPwm(pushed ? "on" : "attack", direction, false);
    if (config.type === "PWM" && number === 5) adjustPwm(pushed ? "off" : "decay", direction, false);
    if (number === 6) {
      if (pushed) selectedLanes().filter(isFunctionLane).forEach((item) => { item.functionWidth = clamp(item.functionWidth + direction * 5, 5, 100); });
      else {
        const multiplier = cycleOption(multiplierOptions, targetLane.multiplier, direction);
        selectedLanes().forEach((item) => { item.multiplier = multiplier; });
      }
    }
    renderAll(false); return;
  }
  const targetFrame = selectedFrame();
  if (number === 1) {
    state.keyframeIndex = (state.keyframeIndex + direction + targetLane.keyframes.length) % targetLane.keyframes.length;
  } else if (number === 2) {
    if (pushed) cycleFrameSource(direction);
    else if (targetFrame.source === "Preset") cycleFramePreset(direction);
    else if (targetFrame.source === "Fixed") targetFrame.value = clamp(targetFrame.value + direction * (["Pan", "Tilt"].includes(targetLane.attribute) ? 5 : 1), presentation().minimum, presentation().maximum);
    else toast("Current follows the upstream programmer value; pushed-turn changes the source.");
  } else if (number === 3) {
    if (pushed) snapKeyframe(direction);
    else if (!targetFrame.loop) { targetFrame.time = clamp(targetFrame.time + direction, 0, 100); syncScaleBaseTime(targetLane, state.keyframeIndex); }
  } else if (number === 4) targetFrame.interpolation = cycleOption(keyframeInterpolations, targetFrame.interpolation || "Ease in + out", direction);
  else if (number === 6) {
    if (pushed) selectedLanes().filter((item) => item.mode === "Keyframes").forEach((item) => setLaneWidthScale(item, item.widthScale + direction * 5));
    else {
      const multiplier = cycleOption(multiplierOptions, targetLane.multiplier, direction);
      selectedLanes().forEach((item) => { item.multiplier = multiplier; });
    }
  }
  renderAll(false);
}

function handleEncoderPush(number, button) {
  state.encoder = number;
  button.classList.add("is-pushed"); setTimeout(() => button.classList.remove("is-pushed"), 150);
  if (state.view === "phase") {
    if (number === 1) { const span = phaseSpan(); state.phaseStart = 0; state.phaseEnd = span; } if (number === 2) setPhaseSpan(360, false); if (number === 3) state.phaseBlock = 1;
    if (number === 4) state.phaseRepeats = 1; if (number === 5) setOrdering(cycleOption(orderingModes, state.ordering, 1), false); if (number === 6) { if (state.ordering === "Linear") state.linearDirection = 90; else state.phaseOffset = 0; }
    renderAll(false); return;
  }
  if (state.view === "speed") {
    if (number === 1) selectSpeedGroup(state.speedGroupIndex >= 0 ? -1 : state.lastSpeedGroupIndex, false); if (number === 2) toast("Tap tempo would learn repeated pushes."); if (number === 3) state.overallMultiplier = "× 1";
    if (number === 4) state.quantize = cycleOption(quantizeOptions, state.quantize, 1); if (number === 5) state.startPolicy = cycleOption(startPolicies, state.startPolicy, 1); if (number === 6) state.transportOffset = 0;
    renderAll(false); return;
  }
  const targetLane = selectedLane();
  if (number === 1) {
    openModeModal(); return;
  }
  if (isFunctionLane(targetLane)) {
    const config = targetLane.functionConfig;
    if (number === 2) return openFunctionSourceModal(usesFunctionBounds(targetLane) ? "top" : "center");
    if (number === 3 && usesFunctionBounds(targetLane)) return openFunctionSourceModal("bottom");
    if (number === 3) selectedLanes().forEach((item) => { item.functionConfig.amplitude = (attributePresentation[item.attribute].maximum - attributePresentation[item.attribute].minimum) / 4; });
    if (isRandomType(config.type) && number === 4) selectedLanes().filter(isRandomLane).forEach((item) => { item.functionConfig.random.grouping = 50; item.functionConfig.random.pulseWidth = 8; });
    if (isRandomType(config.type) && number === 5) selectedLanes().filter(isRandomLane).forEach((item) => { item.functionConfig.random.density = 8; });
    if (config.type === "PWM" && (number === 4 || number === 5)) {
      selectedLanes().filter((item) => isFunctionLane(item) && item.functionConfig.type === "PWM").forEach((item) => {
        if (number === 4) item.pwm.attack = 8;
        if (number === 5) item.pwm.decay = 8;
        item.pwm.on = 50; item.pwm.off = 50;
      });
    }
    if (number === 6) selectedLanes().forEach((item) => { item.multiplier = "× 1"; item.functionWidth = 100; });
    renderAll(false); return;
  }
  if (number === 2) return openValueModal();
  if (number === 3) centerKeyframe();
  if (number === 4) selectedFrame().interpolation = cycleOption(keyframeInterpolations, selectedFrame().interpolation || "Ease in + out", 1);
  if (number === 6) selectedLanes().forEach((item) => { item.multiplier = "× 1"; setLaneWidthScale(item, 100); });
  renderAll(false);
}

function openModeModal() {
  state.modeModalTab = selectedLane().mode;
  $("#mode-modal").hidden = false;
  renderModeModal();
}

function renderModeModal() {
  const targetLane = selectedLane();
  $("#mode-modal-title").textContent = `${targetLane.attribute} · encoder 1 selection`;
  $$('[data-mode-modal-tab]').forEach((button) => {
    const active = button.dataset.modeModalTab === state.modeModalTab;
    button.classList.toggle("is-active", active);
    button.setAttribute("aria-selected", String(active));
  });
  const host = $("#mode-modal-list");
  host.replaceChildren();
  if (state.modeModalTab === "Keyframes") {
    targetLane.keyframes.forEach((item, index) => {
      const button = document.createElement("button");
      button.className = `mode-choice${targetLane.mode === "Keyframes" && index === state.keyframeIndex ? " is-active" : ""}`;
      button.innerHTML = `<b>Keyframe ${item.label}</b><small>${Math.round(item.time)}% · ${frameDisplay(targetLane, item)}${item.loop ? " · loop close" : ""}</small>`;
      button.addEventListener("click", () => {
        selectedLanes().forEach((itemLane) => { itemLane.mode = "Keyframes"; });
        state.keyframeIndex = Math.min(index, targetLane.keyframes.length - 1);
        $("#mode-modal").hidden = true;
        renderAll(false);
      });
      host.append(button);
    });
    return;
  }
  functionTypes.forEach((type) => {
    const button = document.createElement("button");
    button.className = `mode-choice${targetLane.mode === state.modeModalTab && targetLane.functionConfig.type === type ? " is-active" : ""}`;
    button.innerHTML = `<b>${type}</b><small>${functionDescriptions[type]}</small>`;
    button.addEventListener("click", () => {
      selectedLanes().forEach((itemLane) => { itemLane.mode = state.modeModalTab; itemLane.functionConfig.type = type; });
      $("#mode-modal").hidden = true;
      renderAll(false);
    });
    host.append(button);
  });
}

function renderEncoderHelp() {
  const targetLane = selectedLane();
  const host = $("#encoder-help-content");
  $("#encoder-help-title").textContent = `${targetLane.attribute} · encoder help`;
  if (!isRandomLane(targetLane)) {
    host.innerHTML = `<p>Encoder 1 always controls what is selected in the lane. Its pushed turn changes the lane generation mode; its Push opens the complete selection menu.</p><div class="encoder-help-grid"><div class="encoder-help-item"><b>Encoder 1 turn</b><span>${isFunctionLane(targetLane) ? "Selects the current built-in function." : "Selects the current keyframe."}</span></div><div class="encoder-help-item"><b>Encoder 1 pushed turn</b><span>Cycles Keyframes, Max / min, and Amplitude.</span></div><div class="encoder-help-item"><b>Encoder 1 Push</b><span>Opens three tabs for choosing the mode and its function or keyframe.</span></div><div class="encoder-help-item"><b>Random</b><span>Select Random gate, Random timing, or Random gate + timing to see its timing, grouping, width, and seed controls here.</span></div></div>`;
    return;
  }
  const random = targetLane.functionConfig.random;
  const source = randomSourceFor(targetLane);
  host.innerHTML = `<p>${targetLane.functionConfig.type} is repeatable within a loop and creates a new seeded pattern on the next loop. Random gate chooses minimum or maximum at regular opportunities. Random timing places maximum pulses at random moments. Random gate + timing randomizes both the moments and their minimum/maximum gate decisions.</p><div class="encoder-help-grid"><div class="encoder-help-item"><b>Encoder 4 turn · Pulse</b><span>Sets how long each gate or random-time event stays active.</span></div><div class="encoder-help-item"><b>Encoder 4 pushed · Grouping</b><span>0% separates decisions. Higher values pull gates or event times into related bursts of roughly two to four.</span></div><div class="encoder-help-item"><b>Encoder 5 turn · Density</b><span>Sets the target number of gate opportunities or random events per loop. The actual timed-event count varies each round.</span></div><div class="encoder-help-item"><b>Encoder 5 pushed · Source</b><span>Selects Random 1–4. Lanes using the same source share one underlying seed and pattern.</span></div><div class="encoder-help-item"><b>Encoder 6 pushed · Width</b><span>Compresses the complete random pattern into the start of the cycle. The remaining tail holds the minimum value until restart.</span></div><div class="encoder-help-item"><b>Start and final value</b><span>Every Random function starts and ends at the lane minimum. A gate-based Random never begins with a maximum gate.</span></div></div><div class="random-source-help"><span><b>Shared random source</b><small>${source.name} · seed #${source.seed}</small></span><div class="random-source-buttons"></div><button class="reseed-button">Generate new seed for ${source.name}</button></div>`;
  const sourceHost = $(".random-source-buttons", host);
  randomSources.forEach((item, index) => {
    const button = document.createElement("button");
    button.classList.toggle("is-active", index === random.sourceIndex);
    button.textContent = `${item.name} · #${item.seed}`;
    button.addEventListener("click", () => {
      selectedLanes().filter(isRandomLane).forEach((laneItem) => { laneItem.functionConfig.random.sourceIndex = index; });
      renderAll(false); renderEncoderHelp();
    });
    sourceHost.append(button);
  });
  $(".reseed-button", host).addEventListener("click", () => { reseedRandomSource(targetLane); renderAll(false); renderEncoderHelp(); });
}

function openEncoderHelp() {
  renderEncoderHelp();
  $("#encoder-help-modal").hidden = false;
}

function openValueModal() {
  state.functionSourceField = null;
  if (selectedFrame().loop) return toast("The loop-close keyframe aliases A.");
  $("#value-modal").hidden = false;
  renderValueModal();
}

function openFunctionSourceModal(field) {
  state.functionSourceField = field;
  $("#value-modal").hidden = false;
  renderValueModal();
}

function activeValueTarget() {
  return state.functionSourceField ? selectedLane().functionConfig[state.functionSourceField] : selectedFrame();
}

function renderValueModal() {
  const targetLane = selectedLane();
  const target = activeValueTarget();
  const fieldLabel = state.functionSourceField ? ({ center: "Middle", bottom: "Bottom", top: "Top" }[state.functionSourceField]) : `keyframe ${target.label}`;
  $("#value-modal-title").textContent = `${targetLane.attribute} · ${fieldLabel}`;
  $$("[data-modal-source]").forEach((button) => button.classList.toggle("is-active", button.dataset.modalSource === target.source));
  $("#preset-picker").hidden = target.source !== "Preset";
  $("#fixed-value-editor").hidden = target.source !== "Fixed";
  $("#current-value-copy").hidden = target.source !== "Current";
  const host = $("#preset-picker"); host.replaceChildren();
  presetsForAttribute(targetLane.attribute).forEach((preset) => {
    const button = document.createElement("button");
    button.className = `preset-choice${preset.name === target.preset ? " is-active" : ""}`;
    const color = preset.hex || (preset.value ? "#ffffff" : "#090d11");
    button.style.setProperty("--preset-color", color);
    button.innerHTML = `<i></i><span><b>${preset.name}</b><small>${preset.value !== undefined ? `${preset.value}${presentation().unit}` : preset.hex}</small></span>`;
    button.addEventListener("click", () => {
      if (state.functionSourceField) { target.source = "Preset"; target.preset = preset.name; target.value = sourceValue(targetLane, target); }
      else applyPreset(targetLane, target, preset);
      renderAll(false); renderValueModal();
    });
    host.append(button);
  });
  const input = $("#fixed-value-input");
  input.min = String(presentation().minimum); input.max = String(presentation().maximum); input.value = String(Math.round(target.value));
}

function openLaneModal() {
  const host = $("#lane-picker"); host.replaceChildren();
  Object.keys(attributePresentation).forEach((attribute) => {
    const button = document.createElement("button");
    button.textContent = attribute;
    button.disabled = currentExample().lanes.some((item) => item.attribute === attribute);
    button.addEventListener("click", () => addLane(attribute));
    host.append(button);
  });
  $("#lane-modal").hidden = false;
}

function addLane(attribute) {
  const preset = presetsForAttribute(attribute)[0];
  const value = preset.value ?? 0;
  currentExample().lanes.push(lane(attribute, "Linear", [frame(0, "Preset", value, preset.name), frame(100, "Preset", value, preset.name, { loop: true })]));
  relabelFrames(currentExample().lanes.at(-1));
  state.laneAttribute = attribute; state.selectedAttributes = [attribute]; state.keyframeIndex = 0;
  $("#lane-modal").hidden = true;
  renderAll(false); toast(`${attribute} lane added.`);
}

function selectLane(attribute, extendSelection = false) {
  const isSelected = state.selectedAttributes.includes(attribute);
  if (!extendSelection) {
    state.selectedAttributes = [attribute];
    state.laneAttribute = attribute;
  } else if (isSelected && state.selectedAttributes.length > 1) {
    state.selectedAttributes = state.selectedAttributes.filter((item) => item !== attribute);
    if (state.laneAttribute === attribute) state.laneAttribute = state.selectedAttributes[0];
  } else if (!isSelected) {
    state.selectedAttributes = [...state.selectedAttributes, attribute];
    state.laneAttribute = attribute;
  }
  state.keyframeIndex = 0;
  renderAll(false); toast(`${state.selectedAttributes.length} lane${state.selectedAttributes.length === 1 ? "" : "s"} selected · ${state.laneAttribute} is primary${extendSelection ? " · Shift selection" : ""}.`);
}

function switchExample(index) {
  state.exampleIndex = index;
  state.laneAttribute = examples[index].lanes[0].attribute;
  state.selectedAttributes = [state.laneAttribute];
  state.keyframeIndex = 0;
  state.ordering = examples[index].ordering;
  state.cycleSeconds = examples[index].duration;
  state.progress = 0; state.iteration = 0; state.previewStart = 0;
  $("#example-menu").hidden = true; $("#example-trigger").setAttribute("aria-expanded", "false");
  renderAll(); toast(`Loaded example ${index + 1}: ${examples[index].name}.`);
}

function setOrdering(ordering, announce = true) {
  state.ordering = ordering; currentExample().ordering = ordering;
  if (announce) toast(`Fixture order: ${ordering}.`);
}

function setView(view) {
  state.view = view;
  $$("[data-view]").forEach((button) => button.setAttribute("aria-selected", String(button.dataset.view === view)));
  $$("[data-view-panel]").forEach((panel) => { panel.hidden = panel.dataset.viewPanel !== view; panel.classList.toggle("is-active", panel.dataset.viewPanel === view); });
  document.body.classList.toggle("phase-editing", view === "phase");
  renderAll(false);
}

function renderAll() {
  renderHeader(); renderLanes(); renderKeyframes(); renderPhaseView(); renderSpeedView(); renderEncoderCards();
  updateGrid(state.progress);
}

function animate(time) {
  if (!state.playing) return;
  const duration = currentCycleSeconds();
  if (!state.previewStart) state.previewStart = time - state.progress * duration * 1000;
  const absoluteProgress = (time - state.previewStart) / (duration * 1000);
  const nextIteration = Math.floor(absoluteProgress);
  const iterationChanged = nextIteration !== state.iteration;
  state.iteration = nextIteration;
  state.progress = (absoluteProgress % 1 + 1) % 1;
  if (iterationChanged && (state.ordering === "Random each loop" || currentExample().lanes.some(isRandomLane))) {
    renderLanes();
    if (state.view === "phase") renderPhaseView();
  }
  updateGrid(state.progress);
  $$(".lane-playhead").forEach((line) => { const x = state.progress * 600; line.setAttribute("x1", x); line.setAttribute("x2", x); });
  state.animationFrame = requestAnimationFrame(animate);
}

function toggleTransport(button) {
  state.playing = !state.playing;
  document.body.classList.toggle("playing", state.playing);
  button.setAttribute("aria-pressed", String(state.playing)); button.textContent = state.playing ? "■ Stop" : "▶ Preview";
  if (state.playing) { state.previewStart = 0; state.animationFrame = requestAnimationFrame(animate); toast("Local fixture preview running."); }
  else { cancelAnimationFrame(state.animationFrame); toast("Local fixture preview stopped."); }
}

$("#example-trigger").addEventListener("click", () => {
  const menu = $("#example-menu"); menu.hidden = !menu.hidden; $("#example-trigger").setAttribute("aria-expanded", String(!menu.hidden));
});
$$("[data-view]").forEach((button) => button.addEventListener("click", () => setView(button.dataset.view)));
$$("[data-ordering]").forEach((button) => button.addEventListener("click", () => { setOrdering(button.dataset.ordering); renderAll(false); updateGrid(state.progress); }));
$$("[data-direction-step]").forEach((button) => button.addEventListener("click", () => { state.linearDirection = normalizeDegrees(state.linearDirection + Number(button.dataset.directionStep)); renderAll(false); updateGrid(state.progress); toast(`Grid linear: ${state.linearDirection}° · ${linearDirectionLabel()}.`); }));
$$("[data-phase-span]").forEach((button) => button.addEventListener("click", () => { setPhaseSpan(Number(button.dataset.phaseSpan)); renderAll(false); updateGrid(state.progress); }));
$$("[data-lane-mode]").forEach((button) => button.addEventListener("click", () => setSelectedLaneMode(button.dataset.laneMode)));
$$("[data-modal-source]").forEach((button) => button.addEventListener("click", () => {
  const target = activeValueTarget();
  target.source = button.dataset.modalSource;
  if (target.source === "Preset") {
    const preset = presetsForAttribute(selectedLane().attribute)[0];
    if (state.functionSourceField) { target.preset = preset.name; target.value = sourceValue(selectedLane(), target); }
    else applyPreset(selectedLane(), target, preset);
  }
  renderAll(false); renderValueModal();
}));
$$("[data-close-modal]").forEach((button) => button.addEventListener("click", () => { $(`#${button.dataset.closeModal}`).hidden = true; state.functionSourceField = null; }));
$("#fixed-value-input").addEventListener("input", (event) => { activeValueTarget().value = clamp(Number(event.target.value), presentation().minimum, presentation().maximum); renderAll(false); });
$("#fixed-speed-choice").addEventListener("click", () => selectSpeedGroup(-1));
$$('[data-action]').forEach((button) => button.addEventListener("click", () => { if (button.dataset.action === "transport") toggleTransport(button); if (button.dataset.action === "add-lane") openLaneModal(); if (button.dataset.action === "insert-keyframe") insertKeyframe(); }));
$$('[data-mode-modal-tab]').forEach((button) => button.addEventListener("click", () => {
  state.modeModalTab = button.dataset.modeModalTab;
  selectedLanes().forEach((item) => { item.mode = state.modeModalTab; });
  renderAll(false); renderModeModal();
}));
$("#encoder-help-button").addEventListener("click", openEncoderHelp);
$$('.modal-backdrop').forEach((backdrop) => backdrop.addEventListener("click", (event) => { if (event.target === backdrop) { backdrop.hidden = true; state.functionSourceField = null; } }));

renderGridStructure();
renderAll();
document.body.classList.add("playing");
state.animationFrame = requestAnimationFrame(animate);
