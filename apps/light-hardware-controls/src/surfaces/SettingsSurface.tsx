import type { ControllerSettings } from "../controller/types";

interface SettingsSurfaceProps {
  connected: boolean;
  settings: ControllerSettings;
  updateSettings: (changes: Partial<ControllerSettings>) => void;
  connect: () => Promise<void>;
}

export function SettingsSurface({
  connected,
  settings,
  updateSettings,
  connect,
}: SettingsSurfaceProps) {
  return (
    <section className="settings">
      <h2>OSC connection</h2>
      <p>
        The controller connects automatically when it starts. Changes are saved
        for the next launch.
      </p>
      <label>
        Server
        <input
          value={settings.host}
          onChange={(event) => updateSettings({ host: event.target.value })}
        />
      </label>
      <label>
        OSC port
        <input
          type="number"
          value={settings.port}
          onChange={(event) => updateSettings({ port: Number(event.target.value) })}
        />
      </label>
      <label>
        Desk alias
        <input
          value={settings.desk}
          onChange={(event) => updateSettings({ desk: event.target.value })}
        />
      </label>
      <button onClick={() => void connect()}>
        {connected ? "Save and reconnect" : "Connect"}
      </button>
      <small>
        {connected
          ? `Connected to ${settings.desk} on ${settings.host}:${settings.port}`
          : `Connecting to ${settings.host}:${settings.port}…`}
      </small>
    </section>
  );
}
