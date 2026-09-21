import { invoke } from "@tauri-apps/api/core";
import type { ControlArgument, ControllerSettings } from "../controller/types";

export type NativeSimulatorControl =
  | { kind: "button"; control_id: string; pressed: boolean }
  | { kind: "absolute"; control_id: string; value: number }
  | { kind: "relative"; control_id: string; delta: number };

export interface NativeSimulatorBridge {
  open(
    target: Pick<ControllerSettings, "host" | "simulatorPort">,
  ): Promise<void>;
  send(path: string, arguments_: ControlArgument[]): Promise<void>;
  close(): Promise<void>;
}

export const tauriNativeSimulatorBridge: NativeSimulatorBridge = {
  async open({ host, simulatorPort }) {
    await invoke("connect_native_simulator", {
      host,
      port: Number(simulatorPort),
    });
  },
  async send(path, arguments_) {
    await invoke("send_native_simulator_control", {
      control: simulatorControl(path, arguments_),
    });
  },
  async close() {
    await invoke("disconnect_native_simulator");
  },
};

const token = (value: ControlArgument | undefined) =>
  String(value ?? "").toLowerCase();

/** Translate the simulator surface into declared, typed extension controls. No OSC address crosses the relay. */
export function simulatorControl(
  path: string,
  arguments_: ControlArgument[],
): NativeSimulatorControl {
  const normalized = path.replace(/^\/+|\/+$/gu, "");
  const encoder = /^encode\/(\d+)$/u.exec(normalized);
  if (encoder) {
    const action = token(arguments_[0]);
    if (action === "press")
      return {
        kind: "button",
        control_id: `encoder-${encoder[1]}-press`,
        pressed: true,
      };
    if (["up", "right"].includes(action))
      return {
        kind: "relative",
        control_id: `encoder-${encoder[1]}-turn`,
        delta: 1,
      };
    if (["down", "left"].includes(action))
      return {
        kind: "relative",
        control_id: `encoder-${encoder[1]}-turn`,
        delta: -1,
      };
    throw new Error(`unsupported encoder action: ${action}`);
  }
  if (normalized === "nav") {
    const action = token(arguments_[0]);
    if (!["up", "down", "left", "right", "press"].includes(action))
      throw new Error(`unsupported navigation action: ${action}`);
    return {
      kind: "button",
      control_id: `navigation-${action}`,
      pressed: true,
    };
  }
  if (
    /\/fader$/u.test(normalized) ||
    /programmer\/(?:prog|cue|release)-fade$/u.test(normalized)
  ) {
    const value = Number(arguments_[0]);
    if (!Number.isFinite(value) || value < 0 || value > 1)
      throw new Error(
        "absolute simulator controls require a value from 0 through 1",
      );
    return { kind: "absolute", control_id: controlId(normalized), value };
  }
  if (/^speed-group\/\d+\/encoder$/u.test(normalized)) {
    const bpm = Number(arguments_[0]);
    if (!Number.isFinite(bpm) || bpm < 1 || bpm > 999)
      throw new Error(
        "native simulator speed controls require a BPM value from 1 through 999",
      );
    return {
      kind: "absolute",
      control_id: controlId(normalized),
      value: (bpm - 1) / 998,
    };
  }
  if (normalized === "page")
    throw new Error("native simulator page changes require a page key gesture");
  const pressed = arguments_[0];
  if (typeof pressed !== "boolean")
    throw new Error(
      `native simulator button ${normalized} requires a boolean edge`,
    );
  return { kind: "button", control_id: controlId(normalized), pressed };
}

function controlId(path: string): string {
  return path.replaceAll("/", "-").replaceAll("_", "-");
}
