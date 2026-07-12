import { describe, expect, it } from "vitest";
import { applyMarqueeSelection, applyStageSelection } from "./stageSelection";

const ordered = ["1", "2", "3", "4", "5"];
describe("stage selection gestures", () => {
  it("replaces selection on an unmodified click", () => expect(applyStageSelection({ fixtureId: "3", orderedFixtureIds: ordered, selectedFixtureIds: ["1"], anchorFixtureId: "1", additive: false, range: false })).toEqual(["3"]));
  it("toggles individual fixtures with Ctrl or Meta", () => expect(applyStageSelection({ fixtureId: "3", orderedFixtureIds: ordered, selectedFixtureIds: ["1", "3"], anchorFixtureId: "1", additive: true, range: false })).toEqual(["1"]));
  it("uses contiguous stage order for Shift", () => expect(applyStageSelection({ fixtureId: "4", orderedFixtureIds: ordered, selectedFixtureIds: ["1"], anchorFixtureId: "2", additive: false, range: true })).toEqual(["2", "3", "4"]));
  it("preserves selection for additive marquee", () => {
    expect(applyMarqueeSelection(["1", "4"], ["2", "4", "5"], true)).toEqual(["1", "4", "2", "5"]);
    expect(applyMarqueeSelection(["1"], ["2", "3"], false)).toEqual(["2", "3"]);
  });
});
