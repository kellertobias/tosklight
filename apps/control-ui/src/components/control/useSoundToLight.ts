import { useCallback, useEffect, useRef, useState } from "react";
import type {
  SoundObservation,
  SoundToLightConfig,
  SpeedGroupActionInput,
  SpeedGroupId,
  SpeedGroupSoundState,
} from "../../api/types";
import { useServer } from "../../api/ServerContext";
import {
  enumerateAudioInputs,
  inactiveCaptureStatus,
  microphonePermission,
  SoundToLightAudioAnalyzer,
  type AudioInputDevice,
  type MicrophonePermission,
  type SoundCaptureStatus,
} from "./soundToLightAnalyzer";

export const speedGroupIds: SpeedGroupId[] = ["A", "B", "C", "D", "E"];

export function soundDeviceStorageKey(deskId: string, group: SpeedGroupId) {
  return `light.sound-to-light.device.${deskId}.${group}`;
}

function browserLocalStorage(): Storage | null {
  const storage = globalThis.localStorage;
  return storage && typeof storage.getItem === "function" && typeof storage.setItem === "function" ? storage : null;
}

type GroupMap<T> = Partial<Record<SpeedGroupId, T>>;

export interface SoundToLightController {
  states: GroupMap<SpeedGroupSoundState>;
  captures: GroupMap<SoundCaptureStatus>;
  devices: AudioInputDevice[];
  deviceIds: GroupMap<string>;
  permission: MicrophonePermission;
  loading: boolean;
  error: string | null;
  setDevice: (group: SpeedGroupId, deviceId: string) => void;
  setPreview: (group: SpeedGroupId, configuration: SoundToLightConfig | null) => void;
  refreshInputs: () => Promise<void>;
  save: (group: SpeedGroupId, configuration: SoundToLightConfig) => Promise<SpeedGroupSoundState>;
  action: (group: SpeedGroupId, input: SpeedGroupActionInput) => Promise<SpeedGroupSoundState>;
}

export function shouldPublishSoundObservation(state: SpeedGroupSoundState | undefined) {
  return state?.configuration.enabled === true;
}

function errorMessage(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}

export function useSoundToLight(): SoundToLightController {
  const server = useServer();
  const serverRef = useRef(server);
  serverRef.current = server;
  const deskId = server.session?.desk.id ?? null;
  const sessionId = server.session?.session_id ?? null;
  const [states, setStates] = useState<GroupMap<SpeedGroupSoundState>>({});
  const statesRef = useRef(states);
  statesRef.current = states;
  const [captures, setCaptures] = useState<GroupMap<SoundCaptureStatus>>({});
  const [devices, setDevices] = useState<AudioInputDevice[]>([]);
  const [deviceIds, setDeviceIds] = useState<GroupMap<string>>({});
  const [previews, setPreviews] = useState<GroupMap<SoundToLightConfig>>({});
  const [permission, setPermission] = useState<MicrophonePermission>("unknown");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const analyzers = useRef(new Map<SpeedGroupId, { deviceId: string; analyzer: SoundToLightAudioAnalyzer }>());
  const latestObservations = useRef<GroupMap<SoundObservation>>({});
  const posting = useRef(new Set<SpeedGroupId>());
  const retryAfter = useRef<GroupMap<number>>({});
  const mounted = useRef(true);

  useEffect(() => () => {
    mounted.current = false;
    analyzers.current.forEach(({ analyzer }) => analyzer.stop());
    analyzers.current.clear();
  }, []);

  const refreshInputs = useCallback(async () => {
    const [nextPermission, nextDevices] = await Promise.all([
      microphonePermission(),
      enumerateAudioInputs().catch(() => []),
    ]);
    if (!mounted.current) return;
    setPermission(nextPermission);
    setDevices(nextDevices);
  }, []);

  useEffect(() => {
    void refreshInputs();
    const changed = () => void refreshInputs();
    navigator.mediaDevices?.addEventListener?.("devicechange", changed);
    return () => navigator.mediaDevices?.removeEventListener?.("devicechange", changed);
  }, [refreshInputs]);

  useEffect(() => {
    if (!deskId) {
      setDeviceIds({});
      return;
    }
    const mappings: GroupMap<string> = {};
    const storage = browserLocalStorage();
    for (const group of speedGroupIds) {
      const selected = storage?.getItem(soundDeviceStorageKey(deskId, group));
      if (selected) mappings[group] = selected;
    }
    setDeviceIds(mappings);
  }, [deskId]);

  useEffect(() => {
    if (!sessionId) {
      setStates({});
      return;
    }
    let cancelled = false;
    setLoading(true);
    void Promise.all(speedGroupIds.map((group) => serverRef.current.speedGroup(group)))
      .then((loaded) => {
        if (cancelled) return;
        setStates(Object.fromEntries(loaded.map((state) => [state.group, state])) as GroupMap<SpeedGroupSoundState>);
        setError(null);
      })
      .catch((reason) => {
        if (!cancelled) setError(`Unable to load Speed Groups: ${errorMessage(reason)}`);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => { cancelled = true; };
  }, [sessionId]);

  const acceptState = useCallback((state: SpeedGroupSoundState) => {
    if (!mounted.current) return state;
    setStates((current) => ({ ...current, [state.group]: state }));
    return state;
  }, []);

  const postObservation = useCallback((group: SpeedGroupId, observation: SoundObservation) => {
    latestObservations.current[group] = observation;
    if (posting.current.has(group) || Date.now() < (retryAfter.current[group] ?? 0)) return;
    posting.current.add(group);
    void (async () => {
      try {
        while (latestObservations.current[group] && mounted.current) {
          const next = latestObservations.current[group]!;
          delete latestObservations.current[group];
          acceptState(await serverRef.current.observeSpeedGroup(group, next));
        }
        setError(null);
      } catch (reason) {
        delete latestObservations.current[group];
        retryAfter.current[group] = Date.now() + 1_000;
        if (mounted.current) setError(`Speed Group ${group} audio feedback failed: ${errorMessage(reason)}`);
      } finally {
        posting.current.delete(group);
      }
    })();
  }, [acceptState]);

  useEffect(() => {
    for (const group of speedGroupIds) {
      const saved = states[group]?.configuration;
      const preview = previews[group];
      const configuration = preview ?? saved;
      const deviceId = deviceIds[group] ?? "";
      const shouldCapture = Boolean(configuration && deviceId && (saved?.enabled || preview));
      const running = analyzers.current.get(group);
      if (!shouldCapture || !configuration) {
        if (running) {
          running.analyzer.stop();
          analyzers.current.delete(group);
        }
        setCaptures((current) => ({
          ...current,
          [group]: deviceId
            ? inactiveCaptureStatus
            : {
                ...inactiveCaptureStatus,
                message: saved?.enabled
                  ? "Sound-to-Light is enabled, but this browser has no desk-local input assignment."
                  : inactiveCaptureStatus.message,
              },
        }));
        continue;
      }
      if (running?.deviceId === deviceId) {
        running.analyzer.updateConfiguration(configuration);
        continue;
      }
      running?.analyzer.stop();
      const analyzer = new SoundToLightAudioAnalyzer(
        configuration,
        (observation) => {
          // Preview meters may run before Apply, but only authoritative enabled show state may
          // publish observations to the desk controller.
          if (shouldPublishSoundObservation(statesRef.current[group])) postObservation(group, observation);
        },
        (status) => {
          if (!mounted.current) return;
          setCaptures((current) => ({ ...current, [group]: status }));
          if (status.phase === "capturing") {
            setPermission("granted");
            if (!status.observation) void refreshInputs();
          }
          if (status.phase === "permission_denied") setPermission("denied");
        },
      );
      analyzers.current.set(group, { deviceId, analyzer });
      void analyzer.start(deviceId);
    }
  }, [deviceIds, postObservation, previews, refreshInputs, states]);

  const setDevice = useCallback((group: SpeedGroupId, deviceId: string) => {
    if (!deskId) return;
    const key = soundDeviceStorageKey(deskId, group);
    const storage = browserLocalStorage();
    if (deviceId) storage?.setItem(key, deviceId);
    else storage?.removeItem(key);
    setDeviceIds((current) => {
      const next = { ...current };
      if (deviceId) next[group] = deviceId;
      else delete next[group];
      return next;
    });
  }, [deskId]);

  const setPreview = useCallback((group: SpeedGroupId, configuration: SoundToLightConfig | null) => {
    setPreviews((current) => {
      const next = { ...current };
      if (configuration) next[group] = configuration;
      else delete next[group];
      return next;
    });
  }, []);

  const save = useCallback(async (group: SpeedGroupId, configuration: SoundToLightConfig) => {
    try {
      const state = acceptState(await serverRef.current.updateSpeedGroup(group, configuration));
      setError(null);
      return state;
    } catch (reason) {
      setError(`Unable to save Speed Group ${group}: ${errorMessage(reason)}`);
      throw reason;
    }
  }, [acceptState]);

  const action = useCallback(async (group: SpeedGroupId, input: SpeedGroupActionInput) => {
    try {
      const state = acceptState(await serverRef.current.speedGroupAction(group, input));
      setError(null);
      return state;
    } catch (reason) {
      setError(`Speed Group ${group} action failed: ${errorMessage(reason)}`);
      throw reason;
    }
  }, [acceptState]);

  return {
    states,
    captures,
    devices,
    deviceIds,
    permission,
    loading,
    error,
    setDevice,
    setPreview,
    refreshInputs,
    save,
    action,
  };
}
