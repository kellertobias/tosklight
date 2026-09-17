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
      mode: "osc",
      serverPort: 5000,
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
      mode: "native" as const,
      serverPort: 5010,
    };
    saveControllerSettings(memory.storage, settings);
    expect(JSON.parse(memory.value() ?? "null")).toEqual(settings);
  });

  it("migrates settings saved before the mode switch to OSC mode", () => {
    const legacy = memoryStorage(JSON.stringify({
      host: "10.0.0.4",
      port: 9000,
      desk: "main",
      top: true,
    }));
    expect(loadControllerSettings(legacy.storage)).toMatchObject({
      host: "10.0.0.4",
      mode: "osc",
      serverPort: 5000,
    });
  });

  it("restores a saved Native Hardware mode and rejects unknown modes", () => {
    const native = memoryStorage(JSON.stringify({ mode: "native", serverPort: 5010 }));
    expect(loadControllerSettings(native.storage)).toMatchObject({
      mode: "native",
      serverPort: 5010,
    });
    const unknown = memoryStorage(JSON.stringify({ mode: "midi", serverPort: "5010" }));
    expect(loadControllerSettings(unknown.storage)).toMatchObject({
      mode: "osc",
      serverPort: 5000,
    });
  });
});
