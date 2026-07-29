import type { ApiDriver } from "../bench/core/api";

const FAMILY_GROUPS = [
  ["Show Profile", "13", "A"], ["Aux Show Profile", "14", "A"],
  ["Show Wash", "15", "B"], ["Aux Show Wash", "16", "B"],
  ["Show LED", "17", "C"], ["Aux Show LED", "18", "C"],
] as const;
const MOVING_GROUPS = [
  ["Show Profile", "13"], ["Aux Show Profile", "14"],
  ["Show Wash", "15"], ["Aux Show Wash", "16"],
] as const;

export function plannedDemoDynamicDefinitions() {
  const definitions: any[] = [];
  for (const [group, groupId, speed] of FAMILY_GROUPS) {
    definitions.push(
      definition(definitions.length + 1, `${group} PWM`, groupId, speed, [
        lane("intensity", "max_min", "pwm"),
      ]),
      definition(definitions.length + 2, `${group} Random`, groupId, speed, [
        lane("intensity", "random", "sinus", "random"),
      ], [randomGroup("random")]),
      definition(definitions.length + 3, `${group} Sinus`, groupId, speed, [
        lane("intensity", "max_min", "sinus"),
      ]),
    );
  }
  for (const [group, groupId] of MOVING_GROUPS) {
    definitions.push(
      definition(definitions.length + 1, `${group} Circle`, groupId, "E", [
        lane("pan", "middle_amplitude", "sinus"),
        lane("tilt", "middle_amplitude", "cosinus"),
      ]),
      definition(definitions.length + 2, `${group} Waterfall`, groupId, "E", [
        keyframeLane("tilt", [[0, 0.15], [0.5, 0.85], [1, 0.15]]),
        keyframeLane("intensity", [[0, 0], [0.45, 1], [0.7, 1], [1, 0]]),
      ]),
    );
  }
  definitions.push(
    definition(27, "Wash Row Waterfall", "5", "E", [
      keyframeLane("tilt", [[0, 0.15], [0.5, 0.85], [1, 0.15]]),
      keyframeLane("intensity", [[0, 0], [0.45, 1], [0.7, 1], [1, 0]]),
    ], [], { type: "grid_linear", angle_degrees: 90 }),
    definition(28, "Sunstrip Random Color", "40", "C", [
      lane("color.red", "random", "sinus", "color"),
      lane("color.green", "random", "sinus", "color"),
      lane("color.blue", "random", "sinus", "color"),
    ], [randomGroup("color")]),
    definition(29, "Sunstrip Rain", "40", "C", [
      keyframeLane("intensity", [[0, 0], [0.5, 1], [1, 0]]),
      keyframeLane("color.blue", [[0, 0], [0.35, 1], [0.75, 1], [1, 0]]),
      keyframeLane("color.white", [[0, 0], [0.65, 0], [0.8, 1], [1, 0]]),
    ], [], { type: "grid_linear", angle_degrees: 90 }),
    definition(30, "Show LED Random Strobe", "17", "C", [
      lane("strobe", "random", "sinus", "strobe"),
    ], [randomGroup("strobe")]),
  );
  return definitions;
}

export async function installPlannedDemoDynamics(api: ApiDriver, showId: string) {
  const definitions = plannedDemoDynamicDefinitions();
  for (const dynamicDefinition of definitions)
    await api.request(
      "POST",
      "/api/v2/dynamics/create",
      { request_id: crypto.randomUUID(), definition: dynamicDefinition },
      true,
      undefined,
      { showId },
    );
  const [page] = await api.showObjects<any>(showId, "playback_page");
  if (!page) throw new Error("Plan 76 Busking Playback page is missing");
  const virtual_playbacks = Object.fromEntries(definitions.map((dynamicDefinition, index) => {
    const number = 1001 + index;
    return [String(number), dynamicPlayback(number, dynamicDefinition)];
  }));
  await api.seedShowObject(showId, "playback_page", page.id, {
    ...page.body,
    virtual_playbacks,
  }, page.revision);
  return definitions;
}

function definition(
  poolNumber: number,
  name: string,
  groupId: string,
  speedGroup: string,
  lanes: any[],
  randomGroups: any[] = [],
  ordering: any = { type: "selection" },
) {
  const boundRandomGroups = randomGroups.map((item, index) => ({
    ...item,
    id: stableUuid(7, poolNumber * 100 + index + 1),
  }));
  return {
    id: stableUuid(5, poolNumber),
    pool_number: poolNumber,
    revision: 0,
    name,
    color: "#4edcff",
    icon: "∿",
    target_binding: { type: "live_group", group_id: groupId },
    lanes: lanes.map((item, index) => ({
      ...item,
      id: stableUuid(6, poolNumber * 100 + index + 1),
      random_group_id: item.random_group_id ? boundRandomGroups[0]?.id ?? null : null,
    })),
    random_groups: boundRandomGroups,
    phase_mode: "uniform",
    phase: {
      ordering,
      offset_degrees: 0,
      span_degrees: 360,
      block_size: 1,
      repeats: 1,
      wings: false,
      anchors_degrees: [],
    },
    speed: {
      type: "speed_group",
      group: speedGroup,
      beats_per_cycle: { numerator: 4, denominator: 1 },
    },
    overall_speed_multiplier: { numerator: 1, denominator: 1 },
    run_mode: "loop",
    default_activation: "start_now",
    activation_boundary: "beat",
  };
}

function lane(attribute: string, mode: string, periodic: string, randomGroupId?: string) {
  return {
    attribute,
    mode,
    keyframes: keyframes([[0, 0], [0.5, 1], [1, 0]]),
    max_min: {
      minimum: value(0),
      maximum: value(1),
      function: periodic,
      size: 1,
      pwm: {
        attack: 0,
        on: 0.25,
        decay: 0,
        off: 0.75,
        attack_interpolation: "linear",
        decay_interpolation: "linear",
      },
    },
    middle_amplitude: {
      middle: { type: "current" },
      amplitude: 0.35,
      function: periodic,
      size: 1,
      pwm: {
        attack: 0,
        on: 0.5,
        decay: 0,
        off: 0.5,
        attack_interpolation: "linear",
        decay_interpolation: "linear",
      },
    },
    speed_multiplier: { numerator: 1, denominator: 1 },
    width: 1,
    random_group_id: randomGroupId ?? null,
    phase: null,
  };
}

function keyframeLane(attribute: string, points: Array<[number, number]>) {
  return { ...lane(attribute, "keyframes", "sinus"), keyframes: keyframes(points) };
}

function keyframes(points: Array<[number, number]>) {
  return {
    points: points.map(([position, scalar]) => ({
      position: Math.min(position, 0.999),
      source: value(scalar),
      interpolation: "ease_in_out",
    })),
    size: 1,
  };
}

function randomGroup(key: string) {
  return {
    seed: randomSeed(key),
    low: value(0),
    high: value(1),
    decision_interval_millis: 250,
    start_probability: 0.25,
    mean_duration_millis: 500,
    duration_spread_millis: 100,
    attack_ratio: 0.1,
    decay_ratio: 0.1,
  };
}

function randomSeed(key: string) {
  return [...key].reduce((seed, character) => seed * 31 + character.charCodeAt(0), 17);
}

function value(scalar: number) {
  return { type: "value", value: scalar };
}

function dynamicPlayback(number: number, dynamicDefinition: any) {
  return {
    number,
    name: dynamicDefinition.name,
    target: {
      type: "dynamic",
      assignment: {
        dynamic: {
          dynamic_id: dynamicDefinition.id,
          last_known_pool_number: dynamicDefinition.pool_number,
          embedded_fallback: { definition: dynamicDefinition },
        },
        revision: 1,
        target_scope: null,
        fader_mode: "size_and_master",
        priority: 0,
        activation_override: null,
        resume_policy: "follow_dynamic",
        local_speed_multiplier: { numerator: 1, denominator: 1 },
        learned_duration_millis: null,
        crossfade_non_intensity: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        auto_off_full_control: true,
      },
    },
    buttons: ["off", "pause", "flash"],
    button_count: 3,
    fader: "master",
    has_fader: true,
    go_activates: true,
    auto_off: true,
    xfade_millis: 0,
    color: "#4edcff",
    flash_release: "release_all",
    protect_from_swap: false,
  };
}

function stableUuid(namespace: number, value: number) {
  return `00000000-0000-4${namespace.toString(16).padStart(3, "0")}-8300-${value.toString(16).padStart(12, "0")}`;
}
