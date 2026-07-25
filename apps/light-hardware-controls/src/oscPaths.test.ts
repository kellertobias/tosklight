import { describe, expect, it } from "vitest";
import { feedbackPagePlaybackOffset, oscPaths } from "./oscPaths";

describe("hardware OSC contract", () => {
  it("keeps exact programmer, playback, encoder, and highlight paths", () => {
    expect(oscPaths.programmer("record")).toBe("programmer/record");
    expect(oscPaths.pagePlayback(21)).toBe("page-playback/21");
    expect(oscPaths.encoder(4)).toBe("encode/4");
    expect(oscPaths.navigation).toBe("nav");
    expect(oscPaths.highlight("previous")).toBe("highlight/previous");
    expect(oscPaths.speedGroupButton(2)).toBe("speed-group/2/button");
    expect(oscPaths.speedGroupEncoder(2)).toBe("speed-group/2/encoder");
  });

  it("accepts canonical and historic playback feedback segments", () => {
    expect(feedbackPagePlaybackOffset(["feedback", "page-playback", "2"])).toBe(1);
    expect(feedbackPagePlaybackOffset(["feedback", "paged-playback", "2"])).toBe(1);
    expect(feedbackPagePlaybackOffset(["feedback", "page"])).toBe(-1);
  });
});
