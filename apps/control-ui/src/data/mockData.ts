import type { DeskModel, FixtureRow, GroupModel, PresetModel } from "../types";

export const groups: GroupModel[] = [
  ["Front Truss", 8], ["Back Truss", 8], ["Floor Package", 12], ["All Movers", 24],
  ["Profiles", 10], ["Washes", 14], ["Audience", 4], ["Stage Left", 12],
  ["Stage Right", 12], ["Odd", 12], ["Even", 12], ["All Fixtures", 36],
].map(([name, fixtures], index) => ({ id: index + 1, name: String(name), fixtures: Number(fixtures) }));

const colors = ["#e24bdb", "#1bd6ec", "#3f8cff", "#f275d9", "#f2c94c", "#44d36b"];

export const fixtures: FixtureRow[] = Array.from({ length: 36 }, (_, index) => {
  const id = index + 1;
  const selected = id <= 8;
  return {
    id,
    name: `${id <= 18 ? "Front" : "Back"} Truss ${id % 2 ? "L" : "R"}${Math.ceil(id / 2)}`,
    type: id % 3 ? "Profile moving head" : "Wash moving head",
    dimmer: selected ? 75 : Math.max(10, 100 - id * 2),
    color: colors[id % colors.length],
    colorLabel: id % 4 === 0 ? "Blue Wash" : colors[id % colors.length],
    pan: 70 + id,
    tilt: 35 + id,
    positionLabel: id % 3 === 0 ? "Fan Out" : undefined,
    beam: id % 4 ? "Open" : "Gobo 3",
    focus: id % 6 ? "Sharp" : "Soft Edge",
    sources: {
      dimmer: selected ? "programmer" : id <= 26 ? "playback" : "default",
      color: selected ? "programmer" : "playback",
      position: selected ? "programmer" : "playback",
      beam: id % 5 === 0 ? "programmer" : "playback",
      focus: "playback",
    },
  };
});

const presetNames = ["Intro", "Blue Wash", "Full Stage", "Mid Stage", "Warm Wash", "Red Wash", "Side Wash L", "Side Wash R", "Strobe", "Audience", "Blackout", "End", "Open White", "Fan Out", "Center", "Gobo Breakup", "Slow Circle", "Rainbow"];
const icons = ["☀", "◐", "▰", "⌖", "◉", "✦"];
export const presets: PresetModel[] = Array.from({ length: 40 }, (_, index) => ({
  id: index + 1,
  name: presetNames[index],
  family: index < 6 ? "Color" : index < 10 ? "Position" : "Mixed",
  color: colors[index % colors.length],
  icon: icons[index % icons.length],
  fixtures: index < presetNames.length ? 24 : undefined,
}));

export const initialDesks: DeskModel[] = [
  {
    id: "programming",
    name: "Programming",
    panes: [
      { id: "presets", kind: "presets", title: "Mixed Presets", x: 1, y: 1, width: 9, height: 18, presetFamily: "Mixed" },
      { id: "fixtures", kind: "fixtures", title: "Fixture Sheet", x: 10, y: 1, width: 15, height: 9 },
      { id: "stage", kind: "stage", title: "Stage · Main floor", x: 10, y: 10, width: 15, height: 9 },
    ],
  },
  { id: "playback", name: "Cuelists", panes: [{ id: "cuelists-main", kind: "cuelists", title: "Cuelists", x: 1, y: 1, width: 24, height: 18 }] },
  { id: "patch", name: "Patch", panes: [{ id: "dmx-main", kind: "dmx", title: "DMX Output", x: 1, y: 1, width: 24, height: 18 }] },
];
