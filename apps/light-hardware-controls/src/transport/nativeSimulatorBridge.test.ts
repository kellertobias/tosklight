import { describe, expect, it } from "vitest";
import { simulatorControl } from "./nativeSimulatorBridge";

describe("native simulator protocol mapping", () => {
  it("emits typed controls rather than OSC addresses", () => {
    expect(simulatorControl("programmer/record", [true, "request"])).toEqual({
      kind: "button",
      control_id: "programmer-record",
      pressed: true,
    });
    expect(simulatorControl("page-playback/3/fader", [0.625])).toEqual({
      kind: "absolute",
      control_id: "page-playback-3-fader",
      value: 0.625,
    });
    expect(simulatorControl("encode/2", ["right"])).toEqual({
      kind: "relative",
      control_id: "encoder-2-turn",
      delta: 1,
    });
    expect(simulatorControl("nav", ["press"])).toEqual({
      kind: "button",
      control_id: "navigation-press",
      pressed: true,
    });
    expect(simulatorControl("speed-group/3/encoder", [500])).toEqual({
      kind: "absolute",
      control_id: "speed-group-3-encoder",
      value: 0.5,
    });
  });
});
