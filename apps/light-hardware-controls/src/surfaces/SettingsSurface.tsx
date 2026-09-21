import type { ControllerSettings, HardwareMode } from "../controller/types";

interface SettingsSurfaceProps {
  connected: boolean;
  activeMode: HardwareMode;
  settings: ControllerSettings;
  updateSettings: (changes: Partial<ControllerSettings>) => void;
  connect: () => Promise<void>;
}

export function SettingsSurface({
  connected,
  activeMode,
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
          onChange={(event) =>
            updateSettings({ port: Number(event.target.value) })
          }
        />
      </label>
      <label>
        Desk alias
        <input
          value={settings.desk}
          onChange={(event) => updateSettings({ desk: event.target.value })}
        />
      </label>
      <h2>Native Hardware</h2>
      <p>
        In Native Hardware mode the attached device is operated by the desk's
        native extension. Its inputs go straight to the desk, so the on-screen
        controls only mirror feedback. Device health is read from the desk
        server on this port.
      </p>
      <label>
        Desk HTTP port
        <input
          type="number"
          value={settings.serverPort}
          onChange={(event) =>
            updateSettings({ serverPort: Number(event.target.value) })
          }
        />
      </label>
      <h2>Native Simulator</h2>
      <p>
        The supervised simulator extension exposes a loopback typed-control
        relay. This mode sends native extension inputs and never opens an OSC
        subscription.
      </p>
      <label>
        Simulator relay port
        <input
          type="number"
          min="1"
          max="65535"
          value={settings.simulatorPort}
          onChange={(event) =>
            updateSettings({ simulatorPort: Number(event.target.value) })
          }
        />
      </label>
      <button type="button" onClick={() => void connect()}>
        {connected ? "Save and reconnect" : "Connect"}
      </button>
      <small>
        {activeMode === "native"
          ? "Native Hardware mode · "
          : activeMode === "native-simulator"
            ? "Native Simulator mode · "
            : "OSC mode · "}
        {connected
          ? `Connected to ${settings.desk} on ${settings.host}:${settings.port}`
          : `Connecting to ${settings.host}:${settings.port}…`}
      </small>
    </section>
  );
}
