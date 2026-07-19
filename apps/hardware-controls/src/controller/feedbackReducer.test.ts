import { describe, expect, it } from "vitest";
import { feedbackReducer, reduceFeedback } from "./feedbackReducer";
import { initialFeedbackState } from "./types";

describe("hardware feedback reducer", () => {
  it("marks the controller connected and projects desk state feedback", () => {
    const page = reduceFeedback(initialFeedbackState, {
      address: "/light/main/feedback/page",
      arguments: [{ Int: 7 }],
    });
    const armed = reduceFeedback(page, {
      address: "/light/main/feedback/update/armed",
      arguments: [{ Bool: true }],
    });

    expect(armed.connected).toBe(true);
    expect(armed.page).toBe(7);
    expect(armed.updateArmed).toBe(true);
  });

  it("retains canonical and legacy paged-playback feedback compatibility", () => {
    const canonical = reduceFeedback(initialFeedbackState, {
      address: "/light/main/feedback/page-playback/4/fader",
      arguments: [{ Float: 0.625 }],
    });
    const legacy = reduceFeedback(canonical, {
      address: "/light/main/feedback/paged-playback/4/button/2",
      arguments: [1, 0.5, 0, "slow"],
    });

    expect(legacy.levels[4]).toBe(0.625);
    expect(legacy.lamps["4/2"]).toEqual({
      color: "rgb(255 128 0)",
      state: "slow",
    });
  });

  it("projects highlight availability and the active lamp independently", () => {
    const active = reduceFeedback(initialFeedbackState, {
      address: "/light/main/feedback/highlight/active",
      arguments: [true],
    });
    const withPrevious = reduceFeedback(active, {
      address: "/light/main/feedback/highlight/can-previous",
      arguments: [true],
    });
    const complete = reduceFeedback(withPrevious, {
      address: "/light/main/feedback/highlight/can-next",
      arguments: [true],
    });

    expect(complete.highlight).toEqual({
      active: true,
      canNext: true,
      canPrevious: true,
    });
    expect(complete.lamps.highlight).toEqual({
      color: "#27c4d8",
      state: "on",
    });
  });

  it("projects speed feedback with the exact BPM and beat lamp", () => {
    const state = reduceFeedback(initialFeedbackState, {
      address: "/light/main/feedback/speed-group/3",
      arguments: [128, 0.2, 0.4, 0.8, "fast"],
    });

    expect(state.speedBpms[3]).toBe(128);
    expect(state.lamps["speed/3"]).toEqual({
      bpm: 128,
      color: "rgb(51 102 204)",
      state: "on",
    });
  });

  it("marks feedback stale while reconnecting without discarding projections", () => {
    const connected = {
      ...initialFeedbackState,
      connected: true,
      page: 9,
      levels: { 2: 0.75 },
    };

    expect(feedbackReducer(connected, { type: "connection-requested" })).toEqual({
      ...connected,
      connected: false,
    });
  });
});
