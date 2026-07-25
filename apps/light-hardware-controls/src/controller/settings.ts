import type { ControllerSettings } from "./types";

export const hardwareSettingsKey = "tosklight.hardware";

export const defaultControllerSettings: ControllerSettings = {
  host: "127.0.0.1",
  port: 9000,
  desk: "main",
  top: true,
};

export interface SettingsStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export function loadControllerSettings(
  storage: SettingsStorage,
): ControllerSettings {
  const saved = parseSavedSettings(storage.getItem(hardwareSettingsKey));
  return { ...defaultControllerSettings, ...saved };
}

function parseSavedSettings(value: string | null): Partial<ControllerSettings> {
  if (value === null) return {};
  try {
    const parsed: unknown = JSON.parse(value);
    if (typeof parsed !== "object" || parsed === null) return {};
    const candidate = parsed as Partial<ControllerSettings>;
    return {
      ...(typeof candidate.host === "string" ? { host: candidate.host } : {}),
      ...(typeof candidate.port === "number" ? { port: candidate.port } : {}),
      ...(typeof candidate.desk === "string" ? { desk: candidate.desk } : {}),
      ...(typeof candidate.top === "boolean" ? { top: candidate.top } : {}),
    };
  } catch {
    return {};
  }
}

export function saveControllerSettings(
  storage: SettingsStorage,
  settings: ControllerSettings,
): void {
  storage.setItem(hardwareSettingsKey, JSON.stringify(settings));
}
