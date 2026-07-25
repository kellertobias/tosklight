export type Blink = "off" | "on" | "slow" | "medium" | "fast";

export interface Lamp {
  color: string;
  state: Blink;
  bpm?: number;
}

export interface HighlightFeedback {
  active: boolean;
  canNext: boolean;
  canPrevious: boolean;
}

export interface FeedbackMessage {
  address: string;
  arguments: unknown[];
}

export interface FeedbackState {
  connected: boolean;
  page: number;
  levels: Record<number, number>;
  lamps: Record<string, Lamp>;
  speedBpms: Record<number, number>;
  highlight: HighlightFeedback;
  updateArmed: boolean;
}

export interface ControllerSettings {
  host: string;
  port: number;
  desk: string;
  top: boolean;
}

export type ControlArgument = boolean | number | string;
export type SendControl = (path: string, arguments_: ControlArgument[]) => void;

export const darkLamp: Lamp = { color: "#25303a", state: "off" };

export const initialFeedbackState: FeedbackState = {
  connected: false,
  page: 1,
  levels: {},
  lamps: {},
  speedBpms: {},
  highlight: { active: false, canNext: false, canPrevious: false },
  updateArmed: false,
};
