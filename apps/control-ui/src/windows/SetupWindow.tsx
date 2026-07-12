import { useEffect, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DeskConfiguration } from "../api/types";
import { configuredServerUrl } from "../api/LightApiClient";
import { FixtureLibrarySetup } from "../components/setup/FixtureLibrarySetup";
import { useApp } from "../state/AppContext";
import { ScreensSetup } from "../components/setup/ScreensSetup";

const sections = [
  "Shows & recovery",
  "Users & sessions",
  "Inputs",
  "Outputs",
  "Timecode",
  "Network & API",
  "Safety",
  "Diagnostics",
  "Fixture library",
  "Playback layout",
  "Screens",
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
  const [deskButtons, setDeskButtons] = useState(3);
  const [deskName, setDeskName] = useState("");
  const [deskAlias, setDeskAlias] = useState("");
  useEffect(() => setDraft(server.configuration), [server.configuration]);
  useEffect(() => { if (server.session?.desk) { setDeskButtons(server.session.desk.buttons); setDeskName(server.session.desk.name); setDeskAlias(server.session.desk.osc_alias); dispatch({ type: "SET_PLAYBACK_LAYOUT", columns: server.session.desk.columns, rows: server.session.desk.rows }); } }, [server.session?.desk, dispatch]);

  const save = async () => {
    if (!draft) return;
    setRestartRequired(await server.saveConfiguration(draft));
  };

  return (
    <div className="setup-window">
      <header className="window-toolbar">
        <h1>Desk Setup</h1>
        <span className="spacer" />
        {restartRequired && <small className="warning">Restart required</small>}
        <span id="setup-section-actions" className="setup-section-actions" />
        <button disabled={!draft} onClick={() => void save()}>
          Save changes
        </button>
      </header>
      <div>
        <nav>
          {sections.map((name, index) => (
            <button
              onClick={() => setSection(index)}
              className={index === section ? "active" : ""}
              key={name}
            >
              {name}
            </button>
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
                      <button onClick={() => server.switchUser(user.name)}>
                        Use this operator
                      </button>
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
              </div>
            </>
          )}
          {section === 3 && draft && (
            <>
              <h2>Output engine</h2>
              <div className="configuration-form">
                <label>
                  Frame rate
                  <input
                    type="number"
                    min="40"
                    max="44"
                    value={draft.frame_rate_hz}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        frame_rate_hz: Number(event.target.value),
                      })
                    }
                  />
                  <small>40–44 Hz</small>
                </label>
                <label>
                  Output bind address
                  <input
                    value={draft.output_bind_ip}
                    onChange={(event) =>
                      setDraft({ ...draft, output_bind_ip: event.target.value })
                    }
                  />
                </label>
                <label>
                  Backup retention
                  <input
                    type="number"
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
                </label>
              </div>
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
              <div className="configuration-form">
                <label>
                  Light server URL
                  <input
                    value={serverUrl}
                    onChange={(event) => setServerUrl(event.target.value)}
                  />
                  <small>
                    Tauri can use this desk or a remote Light server.
                  </small>
                </label>
                <button onClick={() => server.setServerUrl(serverUrl)}>
                  Connect to server
                </button>
              </div>
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
          {section === 6 && (
            <>
              <h2>Safety</h2>
              <p>
                Fixture safety values and hazardous-device signal-loss policies
                are loaded from the active show and fixture definitions.
              </p>
            </>
          )}
          {section === 7 && (
            <>
              <h2>Diagnostics</h2>
              <div className="setup-cards">
                <section>
                  <b>
                    {server.bootstrap?.output_health.frame_hz.toFixed(1) ?? "—"}{" "}
                    Hz
                  </b>
                  <small>Current frame rate</small>
                </section>
                <section>
                  <b>
                    {server.bootstrap?.output_health.deadline_misses ?? 0}{" "}
                    misses
                  </b>
                  <small>Scheduler deadlines</small>
                </section>
                <section>
                  <b>
                    {server.bootstrap?.output_health.send_errors ?? 0} errors
                  </b>
                  <small>Network output</small>
                </section>
              </div>
            </>
          )}
          {section === 9 && <><h2>Playback desk</h2><p>Each client remembers a physical desk identity. Clients and OSC components joined to the same desk share its active page.</p><div className="setup-list">{server.bootstrap?.desks.map((desk) => <article key={desk.id}><b>{desk.name}</b><span>/{desk.osc_alias}/ · {desk.columns}×{desk.rows} · {desk.buttons} buttons</span><small>{desk.id === server.session?.desk.id ? "Current desk" : desk.id}</small>{desk.id !== server.session?.desk.id && <button onClick={() => server.selectControlDesk(desk.id)}>Join this desk</button>}</article>)}</div><div className="configuration-form"><label>Name<input value={deskName} onChange={(event) => setDeskName(event.target.value)}/></label><label>OSC alias<input value={deskAlias} onChange={(event) => setDeskAlias(event.target.value)}/></label><label>Playbacks per row<input type="number" min="1" max="32" value={state.playbackColumns} onChange={(event) => dispatch({ type: "SET_PLAYBACK_LAYOUT", columns: Number(event.target.value), rows: state.playbackRows })}/></label><label>Rows<input type="number" min="1" max="3" value={state.playbackRows} onChange={(event) => dispatch({ type: "SET_PLAYBACK_LAYOUT", columns: state.playbackColumns, rows: Number(event.target.value) })}/></label><label>Visible buttons<input type="number" min="0" max="3" value={deskButtons} onChange={(event) => setDeskButtons(Number(event.target.value))}/></label><button onClick={() => server.session?.desk && void server.updateControlDesk({ ...server.session.desk, name: deskName, osc_alias: deskAlias, columns: state.playbackColumns, rows: state.playbackRows, buttons: deskButtons })}>Save desk layout</button><small>{state.playbackColumns * state.playbackRows} playback slots total · OSC prefix /light/{server.session?.desk.osc_alias ?? "desk"}/</small></div></>}
          {section === 10 && <ScreensSetup/>}
          {section === 8 && <FixtureLibrarySetup />}
          {server.error && <p className="modal-error">{server.error}</p>}
        </main>
      </div>
    </div>
  );
}
