import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import type { OscBridge } from "../transport/oscBridge";
import { tauriOscBridge } from "../transport/oscBridge";
import { feedbackReducer } from "./feedbackReducer";
import {
  loadControllerSettings,
  saveControllerSettings,
  type SettingsStorage,
} from "./settings";
import type { ControllerSettings, FeedbackState, SendControl } from "./types";
import { initialFeedbackState } from "./types";

export interface HardwareController {
  feedback: FeedbackState;
  settings: ControllerSettings;
  updateSettings: (changes: Partial<ControllerSettings>) => void;
  setTopRowVisible: (visible: boolean) => void;
  connect: () => Promise<void>;
  send: SendControl;
}

interface ControllerDependencies {
  bridge?: OscBridge;
  storage?: SettingsStorage;
}

export function useHardwareController(
  dependencies: ControllerDependencies = {},
): HardwareController {
  const bridge = dependencies.bridge ?? tauriOscBridge;
  const storage = dependencies.storage ?? window.localStorage;
  const [feedback, dispatch] = useReducer(feedbackReducer, initialFeedbackState);
  const [settings, setSettings] = useState(() =>
    loadControllerSettings(storage),
  );

  useEffect(() => {
    let disposed = false;
    let disposeListener: (() => void) | undefined;
    void bridge.listenFeedback((message) => {
      dispatch({ type: "feedback-received", feedback: message });
    }).then((dispose) => {
      if (disposed) dispose();
      else disposeListener = dispose;
    });
    return () => {
      disposed = true;
      disposeListener?.();
    };
  }, [bridge]);

  const connect = useCallback(async () => {
    dispatch({ type: "connection-requested" });
    await bridge.connect(settings);
    saveControllerSettings(storage, settings);
  }, [bridge, settings, storage]);

  const initialConnect = useRef(connect);
  useEffect(() => {
    void initialConnect.current();
  }, []);

  const setTopRowVisible = useCallback((visible: boolean) => {
    setSettings((current) => {
      const next = { ...current, top: visible };
      saveControllerSettings(storage, next);
      return next;
    });
  }, [storage]);

  const updateSettings = useCallback((changes: Partial<ControllerSettings>) => {
    setSettings((current) => ({ ...current, ...changes }));
  }, []);

  const send = useCallback<SendControl>((path, arguments_) => {
    void bridge.send(path, arguments_);
  }, [bridge]);

  return {
    feedback,
    settings,
    updateSettings,
    setTopRowVisible,
    connect,
    send,
  };
}
