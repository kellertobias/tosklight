import { feedbackPagePlaybackOffset } from "../oscPaths";
import type { Blink, FeedbackMessage, FeedbackState, Lamp } from "./types";
import { initialFeedbackState } from "./types";

export type FeedbackAction =
  | { type: "connection-requested" }
  | { type: "feedback-received"; feedback: FeedbackMessage };

function unwrapArgument(value: unknown): unknown {
  if (typeof value === "object" && value !== null) {
    return Object.values(value as Record<string, unknown>)[0];
  }
  return value;
}

function feedbackColor(red: unknown, green: unknown, blue: unknown): string {
  const channel = (value: unknown) => Math.round(Number(value) * 255);
  return `rgb(${channel(red)} ${channel(green)} ${channel(blue)})`;
}

function withLamp(
  lamps: Record<string, Lamp>,
  key: string,
  lamp: Lamp,
): Record<string, Lamp> {
  return { ...lamps, [key]: lamp };
}

export function reduceFeedback(
  state: FeedbackState,
  feedback: FeedbackMessage,
): FeedbackState {
  const parts = feedback.address.split("/");
  const arguments_ = feedback.arguments.map(unwrapArgument);
  let next = state.connected ? state : { ...state, connected: true };

  if (feedback.address.endsWith("/feedback/page")) {
    const page = Number(arguments_[0]);
    if (page !== next.page) next = { ...next, page };
  }
  if (feedback.address.endsWith("/feedback/update/armed")) {
    const updateArmed = Boolean(arguments_[0]);
    if (updateArmed !== next.updateArmed) next = { ...next, updateArmed };
  }

  const highlightOffset = parts.indexOf("highlight");
  if (highlightOffset >= 0 && parts[highlightOffset - 1] === "feedback") {
    next = reduceHighlight(next, parts[highlightOffset + 1], arguments_[0]);
  }

  const speedOffset = parts.indexOf("speed-group");
  if (speedOffset >= 0) {
    next = reduceSpeedGroup(next, Number(parts[speedOffset + 1]), arguments_);
  }

  const playbackOffset = feedbackPagePlaybackOffset(parts);
  return playbackOffset < 0
    ? next
    : reducePlayback(next, parts, playbackOffset, arguments_);
}

function reduceHighlight(
  state: FeedbackState,
  field: string | undefined,
  value: unknown,
): FeedbackState {
  if (field === "active") {
    const active = Boolean(value);
    if (active === state.highlight.active) return state;
    return {
      ...state,
      highlight: { ...state.highlight, active },
      lamps: withLamp(state.lamps, "highlight", {
        color: active ? "#27c4d8" : "#25303a",
        state: active ? "on" : "off",
      }),
    };
  }
  if (field === "can-next") {
    if (Boolean(value) === state.highlight.canNext) return state;
    return {
      ...state,
      highlight: { ...state.highlight, canNext: Boolean(value) },
    };
  }
  if (field === "can-previous") {
    if (Boolean(value) === state.highlight.canPrevious) return state;
    return {
      ...state,
      highlight: { ...state.highlight, canPrevious: Boolean(value) },
    };
  }
  return state;
}

function reduceSpeedGroup(
  state: FeedbackState,
  number: number,
  arguments_: unknown[],
): FeedbackState {
  const bpm = Number(arguments_[0]);
  const stateName: Blink = arguments_[4] === "off" ? "off" : "on";
  const lamp = {
    color: feedbackColor(arguments_[1], arguments_[2], arguments_[3]),
    state: stateName,
    bpm,
  };
  const currentLamp = state.lamps[`speed/${number}`];
  if (
    state.speedBpms[number] === bpm
    && currentLamp?.color === lamp.color
    && currentLamp.state === lamp.state
    && currentLamp.bpm === lamp.bpm
  ) {
    return state;
  }
  return {
    ...state,
    speedBpms: { ...state.speedBpms, [number]: bpm },
    lamps: withLamp(state.lamps, `speed/${number}`, lamp),
  };
}

function reducePlayback(
  state: FeedbackState,
  parts: string[],
  offset: number,
  arguments_: unknown[],
): FeedbackState {
  const slot = Number(parts[offset + 1]);
  if (parts[offset + 2] === "fader") {
    const level = Number(arguments_[0]);
    if (state.levels[slot] === level) return state;
    return {
      ...state,
      levels: { ...state.levels, [slot]: level },
    };
  }
  if (parts[offset + 2] === "button") {
    const key = `${slot}/${parts[offset + 3]}`;
    const lamp: Lamp = {
      color: feedbackColor(arguments_[0], arguments_[1], arguments_[2]),
      state: String(arguments_[3]) as Blink,
    };
    const current = state.lamps[key];
    if (current?.color === lamp.color && current.state === lamp.state) {
      return state;
    }
    return {
      ...state,
      lamps: withLamp(state.lamps, key, lamp),
    };
  }
  return state;
}

export function feedbackReducer(
  state: FeedbackState = initialFeedbackState,
  action: FeedbackAction,
): FeedbackState {
  if (action.type === "connection-requested") {
    return state.connected ? { ...state, connected: false } : state;
  }
  return reduceFeedback(state, action.feedback);
}
