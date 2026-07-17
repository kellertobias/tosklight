import { useEffect, useRef, useState } from "react";
import type { WindowProps } from "./windowTypes";
import { useServer } from "../api/ServerContext";
import type { DeskConfiguration } from "../api/types";
import { configuredServerUrl } from "../api/LightApiClient";
import { FixtureLibrarySetup } from "../components/setup/FixtureLibrarySetup";
import { FileManagerRootsSetup, fileManagerRootsValidationError } from "../components/setup/FileManagerRootsSetup";
import { OutputRoutesSetup } from "../components/setup/OutputRoutesSetup";
import { ShowRecoveryFileManager } from "../components/setup/ShowRecoveryFileManager";
import { RootConfinedFilePickerButton } from "../components/files/RootConfinedFilePickerButton";
import { ScreensSetup } from "../components/setup/ScreensSetup";
import { Button, FormLayout, FormField, NumberField, SelectField, SwitchField, TextAreaField, TextField } from "../components/common";
import { WindowHeader } from "../components/window-kit";
import { useApp } from "../state/AppContext";

const sections = ["Shows & recovery", "Users & sessions", "Inputs", "Outputs", "Timecode", "Network & API", "Fixture library", "Screens & playback", "File Manager", "Desk Lock"];

export function SetupWindow(_: WindowProps) {
  const server = useServer();
  const { state, dispatch } = useApp();
  const [section, setSection] = useState(0);
  const [draft, setDraft] = useState<DeskConfiguration | null>(server.configuration);
  const [restartRequired, setRestartRequired] = useState(false);
  const [serverUrl, setServerUrl] = useState(configuredServerUrl());
  const [lockMessage, setLockMessage] = useState(server.deskLock?.message ?? "Desk locked");
  const [lockWallpaper, setLockWallpaper] = useState<string | null>(server.deskLock?.wallpaper ?? null);
  const [unlockMode, setUnlockMode] = useState<"button" | "pin">(server.deskLock?.unlock_mode ?? "button");
  const [lockPin, setLockPin] = useState("");
  const draftRevision = useRef(0);
  const draftDirty = useRef(false);
  const pendingConfigurationSave = useRef<{ revision: number; configuration: DeskConfiguration } | null>(null);
  const fileManagerRootError = draft ? fileManagerRootsValidationError(draft.file_manager_roots) : null;
  useEffect(() => {
    const pending = pendingConfigurationSave.current;
    if (pending && JSON.stringify(pending.configuration) === JSON.stringify(server.configuration)) {
      pendingConfigurationSave.current = null;
      if (draftRevision.current === pending.revision) {
        draftDirty.current = false;
        setDraft(server.configuration);
      }
      return;
    }
    if (!draftDirty.current) setDraft(server.configuration);
  }, [server.configuration]);
  useEffect(() => {
    if (server.deskLock) {
      setLockMessage(server.deskLock.message);
      setLockWallpaper(server.deskLock.wallpaper);
      setUnlockMode(server.deskLock.unlock_mode);
    }
  }, [server.deskLock]);

  const editDraft = (next: DeskConfiguration) => {
    draftRevision.current += 1;
    draftDirty.current = true;
    setDraft(next);
  };

  const save = async () => {
    if (!draft) return;
    pendingConfigurationSave.current = { revision: draftRevision.current, configuration: draft };
    setRestartRequired(await server.saveConfiguration(draft));
  };

  return (
    <div className="setup-window">
      <WindowHeader
        title={section === 6 ? "Fixture library" : "Desk Setup"}
        info={{
          primary: section === 6 ? `${server.fixtureLibrary.length} modes` : sections[section],
          secondary: restartRequired ? "Restart required" : undefined,
        }}
        toolbar={section === 6 ? <div id="setup-section-actions" className="setup-section-actions" /> : undefined}
        actions={[
          [
            {
              id: "save",
              label: "Save changes",
              disabled: !draft || Boolean(fileManagerRootError),
              onClick: () => void save(),
            },
          ],
        ]}
      />
      <div>
        <nav>
          {sections.map((name, index) => (
            <Button onClick={() => setSection(index)} className={index === section ? "active" : ""} key={name}>
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
                  <b>{server.bootstrap?.active_show?.name ?? "No show loaded"}</b>
                  <small>{server.bootstrap?.active_show?.updated_at ?? "Choose a show from the library"}</small>
                </section>
                <section>
                  <b>{server.shows.length} library shows</b>
                  <small>Portable SQLite files</small>
                </section>
                <section>
                  <b>{server.status}</b>
                  <small>{server.bootstrap?.active_show ? "Autosave active" : "No active show"}</small>
                </section>
              </div>
              <ShowRecoveryFileManager />
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
                    <small>{user.id === server.session?.user.id ? "Current operator" : user.id}</small>
                    {user.enabled && user.id !== server.session?.user.id && <Button onClick={() => server.switchUser(user.name)}>Use this operator</Button>}
                  </article>
                ))}
              </div>
            </>
          )}
          {section === 2 && (
            <>
              <h2>Inputs</h2>
              <div className="setup-list">
                {draft && (
                  <article>
                    <b>Preload capture</b>
                    <SwitchField
                      label="Preload programmer changes"
                      checked={draft.preload_programmer_changes}
                      onChange={(event) =>
                        editDraft({
                          ...draft,
                          preload_programmer_changes: event.target.checked,
                        })
                      }
                    />
                    <SwitchField
                      label="Preload physical playback actions"
                      checked={draft.preload_physical_playback_actions}
                      onChange={(event) =>
                        editDraft({
                          ...draft,
                          preload_physical_playback_actions: event.target.checked,
                        })
                      }
                    />
                    <SwitchField
                      label="Preload virtual playback actions"
                      checked={draft.preload_virtual_playback_actions}
                      onChange={(event) =>
                        editDraft({
                          ...draft,
                          preload_virtual_playback_actions: event.target.checked,
                        })
                      }
                    />
                  </article>
                )}
                <article>
                  <b>MIDI inputs</b>
                  <span>{draft?.midi_inputs.length ? draft.midi_inputs.join(", ") : "No MIDI inputs selected"}</span>
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
                  <SwitchField
                    label="Use the regular 0–9 keys as keypad numbers"
                    checked={state.regularNumberShortcuts}
                    onChange={(event) =>
                      dispatch({
                        type: "SET_REGULAR_NUMBER_SHORTCUTS",
                        value: event.target.checked,
                      })
                    }
                  />
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
                    editDraft({
                      ...draft,
                      frame_rate_hz: Number(event.target.value),
                    })
                  }
                  description="40–44 Hz"
                />
                <TextField label="Output bind address" value={draft.output_bind_ip} onChange={(event) => editDraft({ ...draft, output_bind_ip: event.target.value })} />
                <NumberField
                  label="Backup retention"
                  min="1"
                  max="1000"
                  value={draft.backup_retention}
                  onChange={(event) =>
                    editDraft({
                      ...draft,
                      backup_retention: Number(event.target.value),
                    })
                  }
                />
              </FormLayout>
              <OutputRoutesSetup routes={server.outputRoutes} onSave={server.saveOutputRoute} onDelete={server.deleteOutputRoute}/>
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
                    <small>{source.fallback ? "Fallback allowed" : "Explicit source only"}</small>
                  </article>
                ))}
              </div>
            </>
          )}
          {section === 5 && (
            <>
              <h2>Network & API</h2>
              <FormLayout className="configuration-form" labelPlacement="side">
                <TextField label="Light server URL" value={serverUrl} onChange={(event) => setServerUrl(event.target.value)} description="Tauri can use this desk or a remote Light server." />
                <FormField label="">
                  <Button onClick={() => server.setServerUrl(serverUrl)}>Connect to server</Button>
                </FormField>
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
          {section === 7 && <ScreensSetup />}
          {section === 8 && (
            <>
              <h2>File Manager</h2>
              {draft && <FileManagerRootsSetup
                roots={draft.file_manager_roots}
                systemPickerFallback={draft.file_manager_system_picker_fallback}
                onChange={(file_manager_roots) => editDraft({ ...draft, file_manager_roots })}
                onSystemPickerFallbackChange={(file_manager_system_picker_fallback) => editDraft({ ...draft, file_manager_system_picker_fallback })}
                onOpen={() => dispatch({ type: "OPEN_BUILTIN", kind: "file_manager" })}
              />}
            </>
          )}
          {section === 9 && (
            <>
              <h2>Desk Lock</h2>
              <p>Locking this desk blocks every connected screen and its assigned hardware without changing playback, programmer, or output state.</p>
              <FormLayout labelPlacement="side">
                <TextAreaField label="Lock message" value={lockMessage} onChange={(event) => setLockMessage(event.target.value)} />
                <SelectField
                  label="Unlock control"
                  value={unlockMode}
                  onChange={setUnlockMode}
                  options={[
                    { value: "button", label: "Unlock button" },
                    { value: "pin", label: "PIN required" },
                  ]}
                />
                {unlockMode === "pin" && <TextField label="New PIN" secure inputMode="numeric" value={lockPin} description="4–12 digits. Leave empty to retain the configured PIN." onChange={(event) => setLockPin(event.target.value.replace(/\D/g, ""))} />}
                <FormField label="Wallpaper">
                  <RootConfinedFilePickerButton
                    label="Choose lock wallpaper"
                    allowedExtensions={["png", "jpg", "jpeg", "gif", "webp"]}
                    onFiles={(files) => {
                      const file = files[0];
                      if (!file) return;
                      const reader = new FileReader();
                      reader.onload = () => setLockWallpaper(String(reader.result));
                      reader.readAsDataURL(file);
                    }}
                  />
                  {lockWallpaper && <Button onClick={() => setLockWallpaper(null)}>Use default wallpaper</Button>}
                </FormField>
              </FormLayout>
              <div className="modal-actions">
                <Button
                  onClick={() =>
                    void server.configureDeskLock({
                      message: lockMessage,
                      wallpaper: lockWallpaper,
                      unlock_mode: unlockMode,
                      ...(lockPin ? { pin: lockPin } : {}),
                    })
                  }
                >
                  Save Lock Configuration
                </Button>
                <Button className="danger" onClick={() => void server.lockDesk()}>
                  Lock Desk
                </Button>
              </div>
            </>
          )}
          {server.error && <p className="modal-error">{server.error}</p>}
        </main>
      </div>
    </div>
  );
}
