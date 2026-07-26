import type {
  DmxSnapshot,
  OutputHealth,
  PatchedFixture,
} from "../../../light-desktop/src/api/types";

function slots(values: Record<number, number>): number[] {
  return Array.from({ length: 512 }, (_, index) => values[index + 1] ?? 0);
}

export const dmxSnapshot: DmxSnapshot = {
  revision: 17,
  universes: [
    { universe: 1, slots: slots({ 2: 36, 7: 112, 13: 224, 14: 154, 97: 255 }) },
    { universe: 2, slots: slots({ 1: 18, 64: 84, 128: 176, 512: 232 }) },
  ],
  overrides: [
    { universe: 1, address: 13, value: 224 },
    { universe: 2, address: 128, value: 176 },
  ],
};

export const dmxSnapshotWithoutOverrides: DmxSnapshot = {
  ...dmxSnapshot,
  overrides: [],
};

export const dmxPatchedFixtures = [{
  fixture_id: "stage-hazer",
  fixture_number: 99,
  name: "Stage Hazer",
  universe: 1,
  address: 13,
  definition: {
    name: "Stage Hazer",
    device_type: "hazer",
    footprint: 2,
    heads: [{
      parameters: [
        { attribute: "fog", components: [{ offset: 0 }] },
        { attribute: "fan", components: [{ offset: 1 }] },
      ],
    }],
  },
  multipatch: [],
}] as unknown as PatchedFixture[];

export const dmxOutputHealth: OutputHealth = {
  frames_sent: 85_412,
  packets_sent: 170_824,
  send_errors: 0,
  deadline_misses: 0,
  maximum_lateness_micros: 140,
  frame_hz: 44,
  last_tick_micros: 510,
  maximum_tick_micros: 820,
  scheduler_utilization: 0.08,
};
