import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import {
  createHttpNativeHardwareBridge,
  type NativeHardwareBridge,
} from "../transport/nativeBridge";
import type { OscBridge } from "../transport/oscBridge";
import { tauriOscBridge } from "../transport/oscBridge";
import { feedbackReducer } from "./feedbackReducer";
import { nativeStatusIntervalMs, openLink } from "./linkLifecycle";
import {
  loadControllerSettings,
  saveControllerSettings,
  type SettingsStorage,
} from "./settings";
import type {
  ControllerSettings,
  DeviceStatus,
  FeedbackState,
  HardwareMode,
  LastInput,
  SendControl,
} from "./types";
import { initialFeedbackState } from "./types";

export { nativeStatusIntervalMs };

export interface HardwareController {
  feedback: FeedbackState;
  settings: ControllerSettings;
  /** The mode whose transport is currently running. */
  activeMode: HardwareMode;
  device: DeviceStatus;
  lastInput: LastInput | null;
  /** Why the desk link could not be opened, until the next connect. */
  linkError: string | null;
  updateSettings: (changes: Partial<ControllerSettings>) => void;
  setTopRowVisible: (visible: boolean) => void;
  setMode: (mode: HardwareMode) => Promise<void>;
  connect: () => Promise<void>;
  send: SendControl;
}

interface ControllerDependencies {
  bridge?: OscBridge;
  nativeBridge?: NativeHardwareBridge;
  storage?: SettingsStorage;
}

const inactiveDevice: DeviceStatus = { state: "stopped" };
const defaultNativeBridge = createHttpNativeHardwareBridge();

export function useHardwareController(
  dependencies: ControllerDependencies = {},
): HardwareController {
  const bridge = dependencies.bridge ?? tauriOscBridge;
  const nativeBridge = dependencies.nativeBridge ?? defaultNativeBridge;
  const storage = dependencies.storage ?? window.localStorage;
  const [feedback, dispatch] = useReducer(
    feedbackReducer,
    initialFeedbackState,
  );
  const [settings, setSettings] = useState(() =>
    loadControllerSettings(storage),
  );
  const [activeMode, setActiveMode] = useState<HardwareMode>(settings.mode);
  const [device, setDevice] = useState<DeviceStatus>(inactiveDevice);
  const [lastInput, setLastInput] = useState<LastInput | null>(null);
  const [linkError, setLinkError] = useState<string | null>(null);
  // Each connect starts a new generation; work of a superseded one is dropped.
  const generation = useRef(0);
  const activeModeRef = useRef<HardwareMode>(settings.mode);
  const statusTimer = useRef<ReturnType<typeof setInterval> | undefined>(
    undefined,
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

  const stopStatusPolling = useCallback(() => {
    clearInterval(statusTimer.current);
    statusTimer.current = undefined;
  }, []);

  const connectWith = useCallback(
    async (target: ControllerSettings) => {
      const current = ++generation.current;
      const isCurrent = () => current === generation.current;
      activeModeRef.current = target.mode;
      setActiveMode(target.mode);
      setLastInput(null);
      setLinkError(null);
      setDevice(target.mode === "native" ? { state: "starting" } : inactiveDevice);
      dispatch({ type: "connection-requested" });
      await openLink(target, { bridge, nativeBridge, storage }, {
        isCurrent,
        setLinkError,
        setDevice,
        stopPolling: stopStatusPolling,
        startPolling: (refresh) => {
          statusTimer.current = setInterval(() => {
            void refresh();
          }, nativeStatusIntervalMs);
        },
      });
    },
    [bridge, nativeBridge, storage, stopStatusPolling],
  );

  const connect = useCallback(
    () => connectWith(settings),
    [connectWith, settings],
  );

  const setMode = useCallback(
    async (mode: HardwareMode) => {
      if (mode === activeModeRef.current) return;
      const next = { ...settings, mode };
      setSettings(next);
      // The chosen mode is kept for the next launch even if its link fails now.
      saveControllerSettings(storage, next);
      await connectWith(next);
    },
    [connectWith, settings, storage],
  );

  const initialConnect = useRef(connect);
  useEffect(() => {
    void initialConnect.current();
    return () => {
      generation.current += 1;
      stopStatusPolling();
      void nativeBridge.close();
    };
  }, [nativeBridge, stopStatusPolling]);

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
    // In Native Hardware mode the attached device is the only input path; an
    // on-screen press would be a second, competing source.
    if (activeModeRef.current !== "osc") return;
    setLastInput((shown) => (shown?.path === path ? shown : { path }));
    void bridge.send(path, arguments_);
  }, [bridge]);

  return {
    feedback,
    settings,
    activeMode,
    device,
    lastInput,
    linkError,
    updateSettings,
    setTopRowVisible,
    setMode,
    connect,
    send,
  };
}
