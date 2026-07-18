const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const svgNamespace = "http://www.w3.org/2000/svg";

function makeKeyframes(startValue, peakValue) {
  return [
    { label: "A", time: 0, source: "Fixed", value: startValue, preset: 1 },
    { label: "B", time: 50, source: "Preset", value: peakValue, preset: 2 },
    { label: "A′", time: 100, source: "Loop", value: startValue, loop: true },
  ];
}

const state = {
  view: "curves",
  lane: "Intensity",
  unit: "%",
  encoder: 1,
  keyframe: 0,
  keyframeFine: false,
  phaseEditing: false,
  phaseStart: 0,
  phaseEnd: 360,
  phaseOffset: 0,
  phaseBlock: 1,
  phaseRepeats: 1,
  ordering: "Selection",
  wings: false,
  curveBias: 50,
  laneModes: { Intensity: "Keyframes", Pan: "Keyframes", Tilt: "Center + size", Red: "Keyframes", Green: "Keyframes", Blue: "Keyframes" },
  cycleSeconds: 4,
  speedGroup: false,
  speedBpm: 120,
  overallMultiplier: "× 1",
  laneMultipliers: { Intensity: "× 1", Pan: "÷ 2", Tilt: "× 1", Red: "× 2", Green: "× 2", Blue: "× 2" },
  quantize: "Beat",
  startPolicy: "Start now",
  playing: false,
  previewStart: 0,
  animationFrame: null,
  toastTimer: null,
  values: {
    Intensity: { value: 10, time: 0, shape: "Sine" },
    Pan: { value: -90, time: 0, shape: "Sine" },
    Tilt: { value: 35, time: 0, shape: "Cosine" },
    Red: { value: 0, time: 0, shape: "PWM" },
    Green: { value: 0, time: 0, shape: "Sine" },
    Blue: { value: 100, time: 0, shape: "Cosine" },
  },
  keyframes: {
    Intensity: makeKeyframes(10, 100),
    Pan: makeKeyframes(-90, 90),
    Tilt: makeKeyframes(35, -5),
    Red: makeKeyframes(0, 100),
    Green: makeKeyframes(0, 70),
    Blue: makeKeyframes(100, 0),
  },
  knobTurns: [0, 0, 0, 0, 0, 0],
};

const shapes = ["Sine", "Cosine", "Ramp ↑", "Ramp ↓", "PWM"];
const orderingModes = ["Selection", "Linear", "Radial out", "Radial in"];
const multiplierOptions = ["÷ 4", "÷ 3", "÷ 2", "× 1", "× 2", "× 3", "× 4"];
const quantizeOptions = ["Off", "Beat", "Bar", "2 bars"];
const startPolicies = ["Start now", "Join sync", "Next boundary"];
const shapePaths = {
  Sine: "M0 58 C130 58 170 13 300 13 S470 58 600 58",
  Cosine: "M0 13 C130 13 170 58 300 58 S470 13 600 13",
  "Ramp ↑": "M0 58 L300 13 L600 58",
  "Ramp ↓": "M0 13 L300 58 L600 13",
  PWM: "M0 58 L140 58 L140 14 L430 14 L430 58 L600 58",
};
const lanePresentation = {
  Intensity: { color: "amber", unit: "%" },
  Pan: { color: "cyan", unit: "°" },
  Tilt: { color: "cyan", unit: "°" },
  Red: { color: "red", unit: "%" },
  Green: { color: "green", unit: "%" },
  Blue: { color: "blue", unit: "%" },
};

function clamp(value, minimum, maximum) {
  return Math.max(minimum, Math.min(maximum, value));
}

function toast(message) {
  const node = $("#toast");
  node.textContent = message;
  node.classList.add("visible");
  $("#status-message").textContent = message;
  clearTimeout(state.toastTimer);
  state.toastTimer = setTimeout(() => node.classList.remove("visible"), 1600);
}

function selectExclusive(button, selector) {
  $$(selector).forEach((item) => item.classList.toggle("is-active", item === button));
}

function activeLane() {
  return $(`.attribute-lane[data-lane="${state.lane}"]`);
}

function activeEncoder() {
  return $(`.encoder-card[data-encoder="${state.encoder}"]`);
}

function laneKeyframes() {
  return state.keyframes[state.lane];
}

function selectedKeyframe() {
  return laneKeyframes()[state.keyframe];
}

function setSelectedEncoder(number) {
  state.encoder = number;
  $$(".encoder-card").forEach((card) => card.classList.toggle("is-active", Number(card.dataset.encoder) === number));
}

function renderKeyframeMarks() {
  $$(".attribute-lane").forEach((lane) => {
    const path = $(".curve-line", lane);
    const host = $(".keyframe-marks", lane);
    const frames = state.keyframes[lane.dataset.lane];
    const length = path.getTotalLength();
    host.replaceChildren();
    frames.forEach((frame, index) => {
      const point = path.getPointAtLength(frame.time / 100 * length);
      const circle = document.createElementNS(svgNamespace, "circle");
      circle.setAttribute("cx", point.x);
      circle.setAttribute("cy", point.y);
      circle.setAttribute("r", "5");
      if (lane.dataset.lane === state.lane && index === state.keyframe) circle.classList.add("is-selected");
      host.append(circle);
    });
  });
}

function sourceDetail(frame) {
  if (frame.loop) return "Loop · alias of A";
  if (frame.source === "Preset") return `Preset 1.${frame.preset}`;
  if (frame.source === "Current") return `Current · upstream ${state.lane}`;
  return `Fixed · ${frame.value}${state.unit}`;
}

function renderKeyframeEditor() {
  const frames = laneKeyframes();
  state.keyframe = clamp(state.keyframe, 0, frames.length - 1);
  const selected = selectedKeyframe();
  $("#keyframe-lane-name").textContent = `${state.lane} keyframes`;
  const host = $("#keyframe-list");
  host.replaceChildren();
  frames.forEach((frame, index) => {
    const button = document.createElement("button");
    button.className = `keyframe-chip ${index === state.keyframe ? "is-active" : ""} ${frame.loop ? "loop" : ""}`;
    button.dataset.keyframeIndex = String(index);
    button.innerHTML = `<b>${frame.label}</b><span>${frame.time}%</span><small>${sourceDetail(frame)}</small>`;
    button.addEventListener("click", () => {
      state.keyframe = index;
      renderKeyframeEditor();
      renderEncoders();
      renderKeyframeMarks();
      toast(`${state.lane} keyframe ${frame.label} selected.`);
    });
    host.append(button);
  });
  $$("[data-source]").forEach((button) => {
    button.disabled = selected.loop;
    button.setAttribute("aria-pressed", String(!selected.loop && button.dataset.source === selected.source));
  });
  $("#source-detail").textContent = sourceDetail(selected);
  $("#encoder-context").textContent = `${state.lane} lane · keyframe ${selected.label}`;
  renderKeyframeMarks();
}

function insertKeyframe() {
  const frames = laneKeyframes();
  const loopIndex = frames.findIndex((frame) => frame.loop);
  const previous = frames[loopIndex - 1];
  const nextTime = Math.round((previous.time + 100) / 2 * 10) / 10;
  const label = String.fromCharCode(65 + loopIndex);
  frames.splice(loopIndex, 0, { label, time: nextTime, source: "Current", value: previous.value, preset: 1 });
  state.keyframe = loopIndex;
  renderKeyframeEditor();
  renderEncoders();
  toast(`Inserted keyframe ${label} at ${nextTime}% using Current.`);
}

function setKeyframeSource(source) {
  const frame = selectedKeyframe();
  if (frame.loop) return;
  frame.source = source;
  renderKeyframeEditor();
  renderEncoders();
  toast(`${state.lane} keyframe ${frame.label} now uses ${source}.`);
}

function phaseFractions() {
  const fixtureCount = 12;
  const patternSize = Math.ceil(fixtureCount / state.phaseRepeats);
  const blockCount = Math.ceil(patternSize / state.phaseBlock);
  return Array.from({ length: fixtureCount }, (_, fixtureIndex) => {
    const patternIndex = fixtureIndex % patternSize;
    let blockIndex = Math.floor(patternIndex / state.phaseBlock);
    if (state.wings) {
      const center = (blockCount - 1) / 2;
      blockIndex = Math.round(Math.abs(blockIndex - center));
    }
    const degrees = state.phaseStart + state.phaseOffset + (state.phaseEnd - state.phaseStart) * blockIndex / blockCount;
    return ((degrees % 360) + 360) % 360 / 360;
  });
}

function renderPhasePoints() {
  const fractions = phaseFractions();
  $$(".attribute-lane").forEach((lane) => {
    const path = $(".curve-line", lane);
    const host = $(".phase-marks", lane);
    host.replaceChildren();
    const length = path.getTotalLength();
    fractions.forEach((fraction, index) => {
      const point = path.getPointAtLength(fraction * length);
      const circle = document.createElementNS(svgNamespace, "circle");
      circle.setAttribute("cx", point.x);
      circle.setAttribute("cy", point.y);
      circle.setAttribute("r", "6");
      const title = document.createElementNS(svgNamespace, "title");
      title.textContent = `Fixture ${index + 1} · ${Math.round(fraction * 360)}° phase`;
      circle.append(title);
      host.append(circle);
      const label = document.createElementNS(svgNamespace, "text");
      label.setAttribute("x", point.x);
      label.setAttribute("y", point.y + 0.25);
      label.textContent = String(index + 1);
      host.append(label);
    });
  });
  $("#target-summary").textContent = `12 targets · block ${state.phaseBlock} · ${state.phaseRepeats} repeat${state.phaseRepeats === 1 ? "" : "s"}`;
  renderPhaseView();
}

function renderPhaseView() {
  const laneHost = $("#phase-all-lanes");
  if (!laneHost) return;
  const fractions = phaseFractions();
  laneHost.replaceChildren();
  Object.entries(state.values).forEach(([laneName, laneState]) => {
    const presentation = lanePresentation[laneName];
    const pathData = shapePaths[laneState.shape];
    const lane = document.createElement("article");
    lane.className = "phase-overview-lane";
    lane.dataset.color = presentation.color;
    lane.innerHTML = `
      <span class="phase-overview-identity"><i class="attribute-mark"></i><b>${laneName}</b><small>${laneState.shape} · ${presentation.unit}</small></span>
      <span class="phase-overview-curve">
        <svg viewBox="0 0 600 72" preserveAspectRatio="none" role="img" aria-label="${laneName} curve with the shared fixture phase positions">
          <path class="phase-overview-fill" d="${pathData} L600 72 L0 72 Z"></path>
          <path class="phase-overview-line" d="${pathData}"></path>
          <g class="shared-phase-marks"></g>
          <line class="lane-playhead" x1="0" x2="0" y1="0" y2="72"></line>
        </svg>
        <span class="axis-start">0°</span><span class="axis-middle">180°</span><span class="axis-end">360°</span>
      </span>
      <span class="phase-overview-summary"><b>Shared phase</b><small>12 fixtures</small></span>`;
    laneHost.append(lane);
    const path = $(".phase-overview-line", lane);
    const marks = $(".shared-phase-marks", lane);
    const length = path.getTotalLength();
    fractions.forEach((fraction, index) => {
      const point = path.getPointAtLength(fraction * length);
      const circle = document.createElementNS(svgNamespace, "circle");
      circle.setAttribute("cx", point.x);
      circle.setAttribute("cy", point.y);
      circle.setAttribute("r", "6");
      const title = document.createElementNS(svgNamespace, "title");
      title.textContent = `Fixture ${index + 1} · ${Math.round(fraction * 360)}° phase on ${laneName}`;
      circle.append(title);
      marks.append(circle);
      const label = document.createElementNS(svgNamespace, "text");
      label.setAttribute("x", point.x);
      label.setAttribute("y", point.y + 0.25);
      label.textContent = String(index + 1);
      marks.append(label);
    });
  });
  $("#phase-summary-title").textContent = `${state.phaseStart} THRU ${state.phaseEnd}°`;
  $("#phase-summary-detail").textContent = `12 fixtures · ${state.phaseRepeats} repeat${state.phaseRepeats === 1 ? "" : "s"} · ${state.ordering.toLowerCase()}`;
  const orderHost = $("#fixture-order");
  orderHost.replaceChildren();
  fractions.forEach((fraction, index) => {
    const button = document.createElement("button");
    button.innerHTML = `<b>${index + 1}</b><small>${Math.round(fraction * 360)}°</small>`;
    button.addEventListener("click", () => toast(`Fixture ${index + 1} is positioned at ${Math.round(fraction * 360)}°.`));
    orderHost.append(button);
  });
  $$("[data-ordering]").forEach((button) => button.classList.toggle("is-active", button.dataset.ordering === state.ordering));
}

function multiplierDuration(multiplier) {
  const operator = multiplier[0];
  const amount = Number(multiplier.slice(2));
  if (operator === "×") return state.cycleSeconds / amount;
  return state.cycleSeconds * amount;
}

function renderSpeedView() {
  if (!$("#speed-source-label")) return;
  $("#speed-source-label").textContent = state.speedGroup ? "Speed Group 1" : "Fixed duration";
  $("#speed-primary-value").textContent = state.speedGroup ? `${state.speedBpm} BPM` : `${state.cycleSeconds.toFixed(1)} s`;
  $("#speed-secondary-value").textContent = state.speedGroup ? "shared transport" : "per complete cycle";
  $("#overall-speed-value").textContent = state.overallMultiplier;
  $("#quantize-value").textContent = state.quantize;
  $("#start-policy-value").textContent = state.startPolicy;
  $$(".lane-speed").forEach((button) => {
    const lane = button.dataset.speedLane;
    const multiplier = state.laneMultipliers[lane];
    button.classList.toggle("is-active", lane === state.lane);
    $("span", button).textContent = multiplier;
    $("small", button).textContent = state.speedGroup ? "transport linked" : `${multiplierDuration(multiplier).toFixed(1)} s`;
  });
}

function setView(view) {
  state.view = view;
  $$("[data-view]").forEach((button) => button.setAttribute("aria-selected", String(button.dataset.view === view)));
  $$("[data-view-panel]").forEach((panel) => {
    const active = panel.dataset.viewPanel === view;
    panel.hidden = !active;
    panel.classList.toggle("is-active", active);
  });
  state.phaseEditing = view === "phase";
  document.body.classList.toggle("phase-editing", state.phaseEditing);
  if (view === "phase") renderPhasePoints();
  if (view === "speed") renderSpeedView();
  renderEncoders();
  const labels = { curves: "Curves", phase: "Phase spread", speed: "Speed" };
  toast(`${labels[view]} view · the six encoders are remapped for this task.`);
}

function setLane(button) {
  selectExclusive(button, ".attribute-lane");
  state.lane = button.dataset.lane;
  state.unit = button.dataset.unit;
  state.keyframe = 0;
  renderKeyframeEditor();
  renderEncoders();
  toast(`${state.lane} lane selected. All six encoders now address this scalar lane.`);
}

function setLaneShape(shape) {
  const lane = activeLane();
  const line = $(".curve-line", lane);
  const fill = $(".curve-fill", lane);
  const path = shapePaths[shape];
  line.setAttribute("d", path);
  fill.setAttribute("d", `${path} L600 72 L0 72 Z`);
  state.values[state.lane].shape = shape;
  $(".lane-summary small", lane).textContent = `${shape} · independent`;
  renderKeyframeMarks();
  renderPhasePoints();
}

function renderEncoders() {
  const laneState = state.values[state.lane];
  const frame = selectedKeyframe();
  const cards = $$(".encoder-card");
  let configs;
  if (state.view === "phase") {
    configs = [
      ["Phase start", `${state.phaseStart}°`, "Push: Zero"],
      ["Phase end", `${state.phaseEnd}°`, "Push: Full cycle"],
      ["Block size", String(state.phaseBlock), `Push: Wings ${state.wings ? "on" : "off"}`],
      ["Repeats", String(state.phaseRepeats), "Push: One"],
      ["Ordering", state.ordering, "Push: Next"],
      ["Phase offset", `${state.phaseOffset}°`, "Push: Zero"],
    ];
    $("#encoder-context").textContent = "Shared spread · 12 fixtures across all 6 attributes";
  } else if (state.view === "speed") {
    configs = [
      ["Speed source", state.speedGroup ? "Speed Group 1" : "Fixed", state.speedGroup ? "Push: Fixed" : "Push: Speed Group"],
      [state.speedGroup ? "Tempo" : "Duration", state.speedGroup ? `${state.speedBpm} BPM` : `${state.cycleSeconds.toFixed(1)} s`, "Push: Tap"],
      ["Overall", state.overallMultiplier, "Push: Normal"],
      [`${state.lane} lane`, state.laneMultipliers[state.lane], "Push: Normal"],
      ["Quantize", state.quantize, "Push: Next"],
      ["Start policy", state.startPolicy, "Push: Next"],
    ];
    $("#encoder-context").textContent = `${state.lane} lane · cycle speed and transport`;
  } else {
    const sourceValue = frame.source === "Preset" ? `Preset 1.${frame.preset}` : frame.source === "Current" ? "Current" : `${frame.value}${state.unit}`;
    configs = [
      [frame.source === "Preset" ? "Preset source" : frame.source === "Current" ? "Current source" : "Fixed value", sourceValue, `Push: ${frame.label}`],
      ["Keyframe time", `${frame.time}%`, state.keyframeFine ? "Push: Coarse" : "Push: Fine"],
      ["Segment", laneState.shape, "Push: Select"],
      ["Curve bias", `${state.curveBias}%`, "Push: Center"],
      ["Keyframes", `${laneKeyframes().length} points`, "Push: Insert"],
      ["Lane mode", state.laneModes[state.lane], "Push: Toggle"],
    ];
    $("#encoder-context").textContent = `${state.lane} lane · keyframe ${frame.label}`;
  }
  cards.forEach((card, index) => {
    $(".encoder-label", card).textContent = configs[index][0];
    $(".encoder-value", card).textContent = configs[index][1];
    $(".encoder-knob small", card).textContent = configs[index][2];
  });
  cards.forEach((card, index) => $(".encoder-knob", card).style.setProperty("--knob-turn", `${-35 + state.knobTurns[index] * 11}deg`));
}

function cycleOption(options, current, direction) {
  const index = Math.max(0, options.indexOf(current));
  return options[(index + direction + options.length) % options.length];
}

function turnEncoder(number, direction) {
  setSelectedEncoder(number);
  state.knobTurns[number - 1] += direction;
  if (state.view === "phase") {
    if (number === 1) state.phaseStart = clamp(state.phaseStart + direction * 15, -360, 360);
    if (number === 2) state.phaseEnd = clamp(state.phaseEnd + direction * 15, 0, 720);
    if (number === 3) state.phaseBlock = clamp(state.phaseBlock + direction, 1, 12);
    if (number === 4) state.phaseRepeats = clamp(state.phaseRepeats + direction, 1, 6);
    if (number === 5) state.ordering = cycleOption(orderingModes, state.ordering, direction);
    if (number === 6) state.phaseOffset = clamp(state.phaseOffset + direction * 15, -360, 360);
    renderPhasePoints();
    renderEncoders();
    toast(`Phase encoder ${number}: ${$(".encoder-value", activeEncoder()).textContent}`);
    return;
  }
  if (state.view === "speed") {
    if (number === 1) state.speedGroup = direction > 0;
    if (number === 2) {
      if (state.speedGroup) state.speedBpm = clamp(state.speedBpm + direction, 20, 300);
      else state.cycleSeconds = Math.round(clamp(state.cycleSeconds + direction * .1, .05, 20) * 10) / 10;
    }
    if (number === 3) state.overallMultiplier = cycleOption(multiplierOptions, state.overallMultiplier, direction);
    if (number === 4) state.laneMultipliers[state.lane] = cycleOption(multiplierOptions, state.laneMultipliers[state.lane], direction);
    if (number === 5) state.quantize = cycleOption(quantizeOptions, state.quantize, direction);
    if (number === 6) state.startPolicy = cycleOption(startPolicies, state.startPolicy, direction);
    renderSpeedView();
    renderEncoders();
    toast(`Speed encoder ${number}: ${$(".encoder-value", activeEncoder()).textContent}`);
    return;
  }
  const laneState = state.values[state.lane];
  const frame = selectedKeyframe();
  if (number === 1) {
    if (frame.loop) {
      toast("The loop-closing keyframe aliases A and cannot be edited independently.");
    } else if (frame.source === "Preset") {
      frame.preset = clamp((frame.preset || 1) + direction, 1, 9);
      toast(`${state.lane} keyframe ${frame.label}: Preset 1.${frame.preset}`);
    } else if (frame.source === "Current") {
      toast("Current follows the upstream value; choose Fixed to turn a literal value.");
    } else {
      const step = state.unit === "°" ? 5 : 1;
      frame.value = clamp(frame.value + direction * step, state.unit === "°" ? -270 : 0, state.unit === "°" ? 270 : 100);
      laneState.value = frame.value;
      toast(`${state.lane} keyframe ${frame.label} fixed value: ${frame.value}${state.unit}`);
    }
  } else if (number === 2) {
    if (frame.loop) {
      toast("The loop close remains fixed at 100%.");
    } else {
      const step = state.keyframeFine ? 0.1 : 1;
      frame.time = Math.round(clamp(frame.time + direction * step, 0, 99.9) * 10) / 10;
      laneState.time = frame.time;
      toast(`${state.lane} keyframe ${frame.label} time: ${frame.time}%`);
    }
  } else if (number === 3) {
    const current = shapes.indexOf(laneState.shape);
    const next = (current + direction + shapes.length) % shapes.length;
    setLaneShape(shapes[next]);
    toast(`${state.lane} segment shape: ${shapes[next]}`);
  } else if (number === 4) {
    state.curveBias = clamp(state.curveBias + direction * 5, 0, 100);
    toast(`Curve bias: ${state.curveBias}%`);
  } else if (number === 5) {
    state.keyframe = (state.keyframe + direction + laneKeyframes().length) % laneKeyframes().length;
    toast(`Selected keyframe ${selectedKeyframe().label}.`);
  } else if (number === 6) {
    state.laneModes[state.lane] = state.laneModes[state.lane] === "Keyframes" ? "Center + size" : "Keyframes";
    toast(`${state.lane} lane mode: ${state.laneModes[state.lane]}`);
  }
  renderKeyframeEditor();
  renderEncoders();
}

function pushEncoder(number, button) {
  setSelectedEncoder(number);
  button.classList.add("is-pushed");
  setTimeout(() => button.classList.remove("is-pushed"), 160);
  if (state.view === "phase") {
    if (number === 1) state.phaseStart = 0;
    if (number === 2) state.phaseEnd = 360;
    if (number === 3) state.wings = !state.wings;
    if (number === 4) state.phaseRepeats = 1;
    if (number === 5) state.ordering = cycleOption(orderingModes, state.ordering, 1);
    if (number === 6) state.phaseOffset = 0;
    renderPhasePoints();
    renderEncoders();
    toast(`Phase encoder ${number} push applied.`);
    return;
  }
  if (state.view === "speed") {
    if (number === 1) state.speedGroup = !state.speedGroup;
    if (number === 2) toast("Tap tempo would learn the next encoder pushes.");
    if (number === 3) state.overallMultiplier = "× 1";
    if (number === 4) state.laneMultipliers[state.lane] = "× 1";
    if (number === 5) state.quantize = cycleOption(quantizeOptions, state.quantize, 1);
    if (number === 6) state.startPolicy = cycleOption(startPolicies, state.startPolicy, 1);
    renderSpeedView();
    renderEncoders();
    if (number !== 2) toast(`Speed encoder ${number} push applied.`);
    return;
  }
  if (number === 1) {
    state.keyframe = (state.keyframe + 1) % laneKeyframes().length;
    renderKeyframeEditor();
    toast(`Encoder 1 now edits keyframe ${selectedKeyframe().label}.`);
  } else if (number === 2) {
    state.keyframeFine = !state.keyframeFine;
    toast(`Keyframe time uses ${state.keyframeFine ? "fine 0.1%" : "coarse 1%"} steps.`);
  } else if (number === 3) {
    toast(`Shape picker would open for the ${state.lane} lane.`);
  } else if (number === 4) {
    state.curveBias = 50;
    toast("Curve bias centered at 50%.");
  } else if (number === 5) {
    insertKeyframe();
  } else if (number === 6) {
    state.laneModes[state.lane] = state.laneModes[state.lane] === "Keyframes" ? "Center + size" : "Keyframes";
    toast(`${state.lane} lane mode: ${state.laneModes[state.lane]}`);
  }
  renderKeyframeEditor();
  renderEncoders();
}

function animate(time) {
  if (!state.playing) return;
  if (!state.previewStart) state.previewStart = time;
  const fraction = ((time - state.previewStart) % (state.cycleSeconds * 1000)) / (state.cycleSeconds * 1000);
  $$(".lane-playhead").forEach((line) => {
    const x = fraction * 600;
    line.setAttribute("x1", x);
    line.setAttribute("x2", x);
  });
  state.animationFrame = requestAnimationFrame(animate);
}

function toggleTransport(button) {
  state.playing = !state.playing;
  document.body.classList.toggle("playing", state.playing);
  button.setAttribute("aria-pressed", String(state.playing));
  button.textContent = state.playing ? "■ Stop" : "▶ Preview";
  if (state.playing) {
    state.previewStart = 0;
    state.animationFrame = requestAnimationFrame(animate);
    toast("Local lane preview running.");
  } else {
    cancelAnimationFrame(state.animationFrame);
    toast("Local lane preview stopped.");
  }
}

$$('.attribute-lane').forEach((button) => button.addEventListener("click", () => setLane(button)));
$$('[data-source]').forEach((button) => button.addEventListener("click", () => setKeyframeSource(button.dataset.source)));
$$('[data-view]').forEach((button) => button.addEventListener("click", () => setView(button.dataset.view)));
$$('[data-ordering]').forEach((button) => button.addEventListener("click", () => {
  state.ordering = button.dataset.ordering;
  renderPhasePoints();
  renderEncoders();
  toast(`Phase ordering: ${state.ordering}.`);
}));
$$('[data-speed-lane]').forEach((button) => button.addEventListener("click", () => {
  const lane = $(`.attribute-lane[data-lane="${button.dataset.speedLane}"]`);
  setLane(lane);
  renderSpeedView();
}));

$$('[data-encoder-turn]').forEach((button) => button.addEventListener("click", () => {
  const card = button.closest(".encoder-card");
  turnEncoder(Number(card.dataset.encoder), Number(button.dataset.encoderTurn));
}));
$$('[data-encoder-push]').forEach((button) => button.addEventListener("click", () => {
  const card = button.closest(".encoder-card");
  pushEncoder(Number(card.dataset.encoder), button);
}));

$$('[data-action]').forEach((button) => button.addEventListener("click", () => {
  const action = button.dataset.action;
  if (action === "transport") return toggleTransport(button);
  if (action === "insert-keyframe") return insertKeyframe();
  const messages = {
    undo: "Dummy undo: the last encoder gesture would be reverted.",
    redo: "Dummy redo: the encoder gesture would be restored.",
    "add-lane": "The independent attribute lane picker would open.",
  };
  toast(messages[action] || `${action} clicked.`);
}));

renderKeyframeEditor();
renderPhasePoints();
renderSpeedView();
renderEncoders();
