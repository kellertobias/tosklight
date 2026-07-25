import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ControlArgument,
  ControllerSettings,
  FeedbackMessage,
} from "../controller/types";
import {
	injectedOscBridge,
	type ControllableOscWindow,
} from "./controllableOscBridge";

export type DisposeFeedbackListener = () => void;

export interface OscBridge {
  connect(settings: Pick<ControllerSettings, "host" | "port" | "desk">): Promise<void>;
  send(path: string, arguments_: ControlArgument[]): Promise<void>;
  listenFeedback(
    listener: (feedback: FeedbackMessage) => void,
  ): Promise<DisposeFeedbackListener>;
}

export const tauriOscBridge: OscBridge = {
  async connect({ host, port, desk }) {
    await invoke("connect_osc", { host, port: Number(port), desk });
  },

  async send(path, arguments_) {
    await invoke("send_control", { path, args: arguments_ });
  },

  async listenFeedback(listener) {
    return listen<FeedbackMessage>("osc-feedback", ({ payload }) => {
      listener(payload);
    });
  },
};

export function createOscBridge(
	runtime: ControllableOscWindow | undefined = browserWindow(),
): OscBridge {
	if (runtime) {
		const injected = injectedOscBridge(runtime);
		if (injected) return injected;
	}
	return tauriOscBridge;
}

function browserWindow(): ControllableOscWindow | undefined {
	return typeof window === "undefined"
		? undefined
		: (window as ControllableOscWindow);
}
