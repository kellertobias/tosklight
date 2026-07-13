import { useEffect, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DeskConfiguration } from "../api/types";
import { configuredServerUrl } from "../api/LightApiClient";
import { FixtureLibrarySetup } from "../components/setup/FixtureLibrarySetup";
import { ScreensSetup } from "../components/setup/ScreensSetup";
import { Button, FormLayout, FormField, NumberField, SwitchField, TextField } from "../components/common";
import { WindowHeader } from "../components/window-kit";
import { useApp } from "../state/AppContext";

const sections = [
  "Shows & recovery",
  "Users & sessions",
  "Inputs",
  "Outputs",
  "Timecode",
  "Network & API",
  "Fixture library",
  "Screens & playback",
];

export function SetupWindow(_: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [section, setSection] = useState(0);
  const [draft, setDraft] = useState<DeskConfiguration | null>(
    server.configuration,
  );
  const [restartRequired, setRestartRequired] = useState(false);
  const [serverUrl, setServerUrl] = useState(configuredServerUrl());
  useEffect(() => setDraft(server.configuration), [server.configuration]);

  const save = async () => {
    if (!draft) return;
    setRestartRequired(await server.saveConfiguration(draft));
  };

  return (
    <div className="setup-window">
      <WindowHeader title={section === 6 ? "Fixture library" : "Desk Setup"} info={{ primary: section === 6 ? `${server.fixtureLibrary.length} modes` : sections[section], secondary: restartRequired ? "Restart required" : undefined }} actions={[[{ id: "save", label: "Save changes", disabled: !draft, onClick: () => void save() }]]} />
      <div>
        <nav>
          {sections.map((name, index) => (
            <Button
              onClick={() => setSection(index)}
              className={index === section ? "active" : ""}
              key={name}
            >
              {name}
            </Button>
          ))}
        </nav>
        <main>
          {section === 0 && (
            <>
              <h2>Shows & recovery</h2>
              <div className="setup-cards">
                <section>
                  <b>
                    {server.bootstrap?.active_show?.name ?? "No show loaded"}
                  </b>
                  <small>
                    {server.bootstrap?.active_show?.updated_at ??
                      "Choose a show from the library"}
                  </small>
                </section>
                <section>
                  <b>{server.shows.length} library shows</b>
                  <small>Portable SQLite files</small>
                </section>
                <section>
                  <b>{server.status}</b>
                  <small>
                    Revision {server.bootstrap?.active_show?.revision ?? "—"}
                  </small>
                </section>
              </div>
            </>
          )}
          {section === 1 && (
            <>
              <h2>Users & sessions</h2>
              <div className="setup-list">
                {server.bootstrap?.users.map((user) => (
                  <article key={user.id}>
                    <b>{user.name}</b>
                    <span>{user.enabled ? "Enabled" : "Disabled"}</span>
                    <small>
                      {user.id === server.session?.user.id
                        ? "Current operator"
                        : user.id}
                    </small>
                    {user.enabled && user.id !== server.session?.user.id && (
                      <Button onClick={() => server.switchUser(user.name)}>
                        Use this operator
                      </Button>
                    )}
                  </article>
                ))}
              </div>
            </>
          )}
          {section === 2 && (
            <>
              <h2>Inputs</h2>
              <div className="setup-list">
                <article>
                  <b>MIDI inputs</b>
                  <span>
                    {draft?.midi_inputs.length
                      ? draft.midi_inputs.join(", ")
                      : "No MIDI inputs selected"}
                  </span>
                </article>
                <article>
                  <b>OSC</b>
                  <span>{draft?.osc_bind ?? "Disabled"}</span>
                </article>
                <article>
                  <b>RTP-MIDI</b>
                  <span>{draft?.rtp_midi_bind ?? "Disabled"}</span>
                </article>
                <article>
                  <b>Software keypad shortcuts</b>
                  <SwitchField label="Use the regular 0–9 keys as keypad numbers" checked={state.regularNumberShortcuts} onChange={(event) => dispatch({ type: "SET_REGULAR_NUMBER_SHORTCUTS", value: event.target.checked })}/>
                  <small>Numpad digits and the non-number software shortcuts remain available.</small>
                </article>
              </div>
            </>
          )}
          {section === 3 && draft && (
            <>
              <h2>Output engine</h2>
              <FormLayout className="configuration-form" columns={3} minColumnWidth={190}>
                  <NumberField
                    label="Frame rate"
                    min="40"
                    max="44"
                    value={draft.frame_rate_hz}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        frame_rate_hz: Number(event.target.value),
                      })
                    }
                    description="40–44 Hz"
                  />
                  <TextField
                    label="Output bind address"
                    value={draft.output_bind_ip}
                    onChange={(event) =>
                      setDraft({ ...draft, output_bind_ip: event.target.value })
                    }
                  />
                  <NumberField
                    label="Backup retention"
                    min="1"
                    max="1000"
                    value={draft.backup_retention}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        backup_retention: Number(event.target.value),
                      })
                    }
                  />
              </FormLayout>
            </>
          )}
          {section === 4 && (
            <>
              <h2>Timecode</h2>
              <div className="setup-list">
                {draft?.timecode_sources.map((source) => (
                  <article key={source.source_prefix}>
                    <b>{source.source_prefix}</b>
                    <span>Priority {source.priority}</span>
                    <small>
                      {source.fallback
                        ? "Fallback allowed"
                        : "Explicit source only"}
                    </small>
                  </article>
                ))}
              </div>
            </>
          )}
          {section === 5 && (
            <>
              <h2>Network & API</h2>
              <FormLayout className="configuration-form" labelPlacement="side">
                  <TextField
                    label="Light server URL"
                    value={serverUrl}
                    onChange={(event) => setServerUrl(event.target.value)}
                    description="Tauri can use this desk or a remote Light server."
                  />
                <FormField label=""><Button onClick={() => server.setServerUrl(serverUrl)}>Connect to server</Button></FormField>
              </FormLayout>
              <div className="setup-cards">
                <section>
                  <b>{configuredServerUrl()}</b>
                  <small>Active REST and WebSocket server</small>
                </section>
                <section>
                  <b>REST /api/v1</b>
                  <small>Initial and coarse-grained state</small>
                </section>
                <section>
                  <b>WebSocket connected</b>
                  <small>Live events and control</small>
                </section>
              </div>
            </>
          )}
          {section === 6 && <FixtureLibrarySetup />}
          {section === 7 && <ScreensSetup/>}
          {server.error && <p className="modal-error">{server.error}</p>}
        </main>
      </div>
    </div>
  );
}
