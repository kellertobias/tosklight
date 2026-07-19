import { describe, expect, it } from "vitest";
import {
  defaultControllerSettings,
  hardwareSettingsKey,
  loadControllerSettings,
  saveControllerSettings,
  type SettingsStorage,
} from "./settings";

function memoryStorage(initial: string | null = null) {
  let value = initial;
  const storage: SettingsStorage = {
    getItem: (key) => key === hardwareSettingsKey ? value : null,
    setItem: (key, next) => {
      if (key === hardwareSettingsKey) value = next;
    },
  };
  return { storage, value: () => value };
}

describe("hardware controller settings", () => {
  it("keeps the established connection defaults", () => {
    const { storage } = memoryStorage();
    expect(loadControllerSettings(storage)).toEqual(defaultControllerSettings);
  });

  it("merges validated saved fields and ignores malformed storage", () => {
    const saved = memoryStorage(JSON.stringify({
      host: "10.0.0.4",
      port: 9010,
      desk: "wing",
      top: false,
      ignored: "value",
    }));
    expect(loadControllerSettings(saved.storage)).toEqual({
      host: "10.0.0.4",
      port: 9010,
      desk: "wing",
      top: false,
    });

    const malformed = memoryStorage("{not-json");
    expect(loadControllerSettings(malformed.storage)).toEqual(
      defaultControllerSettings,
    );
  });

  it("persists the complete reconnect and top-row configuration", () => {
    const memory = memoryStorage();
    const settings = {
      host: "light.local",
      port: 9001,
      desk: "main",
      top: false,
    };
    saveControllerSettings(memory.storage, settings);
    expect(JSON.parse(memory.value() ?? "null")).toEqual(settings);
  });
});
