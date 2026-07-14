import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { DebugModal } from "./DebugModal";
import { Button, Input, ModalTitleBar, NumberField, SelectField, TextInput } from "../common";
import type { MvrExportPreview, MvrImportPreview, ShowEntry, ShowRevision } from "../../api/types";
import { getShowIndicator } from "../shell/showIndicator";

export function QuickSetupModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const [showName, setShowName] = useState("");
  const [revisionOpen, setRevisionOpen] = useState(false);
  const [revisionName, setRevisionName] = useState("");
  const [revisionsByShow, setRevisionsByShow] = useState<Record<string, ShowRevision[]>>({});
  const [saveAsOpen, setSaveAsOpen] = useState(false);
  const [loadOpen, setLoadOpen] = useState(false);
  const [newShowOpen, setNewShowOpen] = useState(false);
  const [confirmShutdown, setConfirmShutdown] = useState(false);
  const [changeUserOpen, setChangeUserOpen] = useState(false);
  const [newUserName, setNewUserName] = useState("");
  const [destination, setDestination] = useState<"local" | "flash">("local");
  const upload = useRef<HTMLInputElement>(null);
  const mvrFile = useRef<HTMLInputElement>(null);
  const [mvrMode, setMvrMode] = useState<"new" | "merge" | "export" | null>(null);
  const [mvrTarget, setMvrTarget] = useState<ShowEntry | null>(null);
  const [mvrPreview, setMvrPreview] = useState<MvrImportPreview | null>(null);
  const [mvrExportPreview, setMvrExportPreview] = useState<MvrExportPreview | null>(null);
  const [mvrName, setMvrName] = useState("");
  const [mvrBusy, setMvrBusy] = useState(false);
  const [mvrResolutions, setMvrResolutions] = useState<Record<string, { action: string; universe?: number; address?: number }>>({});
  const flashDriveConnected = false;
  const showIndicator = getShowIndicator(server.status);
  const activeShowId = server.bootstrap?.active_show?.id;
  const activeRevisions = activeShowId ? revisionsByShow[activeShowId] ?? [] : [];
  useEffect(() => {
    if (!state.setupOpen || !activeShowId) return;
    void server.listShowRevisions(activeShowId).then((revisions) =>
      setRevisionsByShow((current) => ({ ...current, [activeShowId]: revisions })),
    );
  }, [state.setupOpen, activeShowId]);
  useEffect(() => {
    if (!state.setupOpen) return;
    const handle = (event: KeyboardEvent) => {
      if (document.querySelector(".ui-input-modal-layer")) return;
      const keyboardInputOpen = saveAsOpen || revisionOpen || changeUserOpen;
      if (event.key === "Escape") {
        event.preventDefault(); event.stopImmediatePropagation();
        if (revisionOpen) setRevisionOpen(false);
        else if (saveAsOpen) setSaveAsOpen(false);
        else if (changeUserOpen) setChangeUserOpen(false);
        else if (loadOpen) setLoadOpen(false);
        else if (newShowOpen) setNewShowOpen(false);
        else if (confirmShutdown) setConfirmShutdown(false);
        else dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
        return;
      }
      if (!keyboardInputOpen) return;
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.key === "Enter") { if (revisionOpen) void saveNamedRevision(); else if (saveAsOpen) void saveAs(); else if (newUserName.trim()) void server.createUser(newUserName.trim()); return; }
      const setValue = revisionOpen ? setRevisionName : saveAsOpen ? setShowName : setNewUserName;
      if (event.key === "Backspace") setValue((value) => value.slice(0, -1));
      else if (event.key.length === 1) setValue((value) => value + event.key);
    };
    window.addEventListener("keydown", handle, true);
    return () => window.removeEventListener("keydown", handle, true);
  }, [state.setupOpen, revisionOpen, saveAsOpen, changeUserOpen, loadOpen, newShowOpen, confirmShutdown, revisionName, showName, newUserName]);
  if (!state.setupOpen) return null;
  const close = () => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
  async function saveNamedRevision(value = revisionName) {
    const name = value.trim();
    if (!name || !activeShowId) return;
    const revision = await server.saveShowRevision(name);
    if (!revision) return;
    setRevisionsByShow((current) => ({
      ...current,
      [activeShowId]: [revision, ...(current[activeShowId] ?? [])],
    }));
    setRevisionName("");
    setRevisionOpen(false);
  }
  async function openLoadMenu() {
    setLoadOpen(true);
    const entries = await Promise.all(server.shows.map(async (show) => [show.id, await server.listShowRevisions(show.id)] as const));
    setRevisionsByShow(Object.fromEntries(entries));
  }
  async function loadNamedRevision(showId: string, revision: number) {
    if (!await server.openShowRevision(showId, revision)) return;
    setLoadOpen(false);
  }
  async function saveAs(value = showName) {
    const name = value.trim();
    if (!name) return;
    if (!await server.saveShowAs(name)) return;
    if (destination === "flash" && server.bootstrap?.active_show) await server.downloadShow({ ...server.bootstrap.active_show, name });
    setSaveAsOpen(false);
    setShowName("");
  }
  async function inspectMvr(file: File) {
    setMvrBusy(true);
    try { const preview = await server.previewMvr(file, mvrMode === "merge" ? mvrTarget?.id : undefined); setMvrPreview(preview); setMvrName(file.name.replace(/\.mvr$/i, "")); const conflicted = new Set(preview.address_conflicts.map((message) => preview.fixtures.find((fixture) => message.startsWith(fixture.name))?.uuid).filter(Boolean)); setMvrResolutions(Object.fromEntries([...conflicted].map((uuid) => [uuid!, { action: "import_unpatched" }]))); }
    finally { setMvrBusy(false); }
  }
  async function applyMvr() {
    if (!mvrPreview) return; setMvrBusy(true);
    try { await server.applyMvr(mvrPreview.token, mvrMode === "new" ? { new_show: { name: mvrName.trim(), open_after_import: true }, resolutions: mvrResolutions } : { existing_show_id: mvrTarget!.id, resolutions: mvrResolutions }); setMvrMode(null); setMvrPreview(null); }
    finally { setMvrBusy(false); }
  }
  async function shutDownDesk() {
    if (!await server.shutdownServer()) return;
    if ("__TAURI_INTERNALS__" in window) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("exit_desktop_app");
    }
  }
  async function inspectExport(show: ShowEntry) { setMvrTarget(show); setMvrBusy(true); try { setMvrExportPreview(await server.previewMvrExport(show.id)); } finally { setMvrBusy(false); } }
  function openMvrImport(closeSource: () => void) { closeSource(); setMvrMode("new"); setMvrTarget(null); setMvrPreview(null); }
  function openMvrExport() {
    setSaveAsOpen(false); setMvrMode("export"); setMvrExportPreview(null);
    const active = server.bootstrap?.active_show;
    if (active) void inspectExport(active); else setMvrTarget(null);
  }
  const stacked = (content: ReactNode, closeLayer: () => void) => createPortal(
    <div className="stacked-modal-layer" onPointerDown={(event) => { if (event.target === event.currentTarget) closeLayer(); }}>{content}</div>,
    document.body,
  );
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.currentTarget === event.target) close(); }}><section className="modal-card show-modal">
    <ModalTitleBar title="Show" closeLabel="Close Show" onClose={close} actions={<>
      <Button onClick={() => setChangeUserOpen(true)}><span aria-hidden="true">♙</span> Change User</Button>
      <Button onClick={() => dispatch({ type: "SET_MODAL", modal: "debugOpen", value: true })}><span aria-hidden="true">⌁</span> Desk Status</Button>
    </>}/>
    <div className="show-details"><b>{server.bootstrap?.active_show?.name ?? "No active show"}</b><div className={`show-status-explanation ${showIndicator.className}`} role="status"><span className="show-status-dot" aria-hidden="true">●</span><span><strong>{showIndicator.label}</strong><small>{showIndicator.detail}</small></span></div><span>Server connected <strong>{server.status === "connected" ? "Yes" : "No"}</strong></span><span>Latest named revision <strong>{activeRevisions[0] ? `${activeRevisions[0].revision} · ${activeRevisions[0].name}` : "None"}</strong></span><span>Operator <strong>{server.session?.user.name ?? "—"}</strong></span><div className="show-primary-actions"><Button onClick={() => setRevisionOpen(true)}><span aria-hidden="true">💾</span> Save Named Revision</Button><Button onClick={() => setSaveAsOpen(true)}><span aria-hidden="true">✎</span> Save As</Button><Button onClick={() => void openLoadMenu()}><span aria-hidden="true">↥</span> Load</Button><Button onClick={() => setNewShowOpen(true)}><span aria-hidden="true">＋</span> New Show</Button></div></div>
    <div className="show-navigation-primary"><Button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "patch" }); close(); }}><span className="show-navigation-icon" aria-hidden="true">▦</span><span>Show Patch</span></Button><Button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "setup" }); close(); }}><span className="show-navigation-icon" aria-hidden="true">⚙</span><span>Enter Setup</span></Button><Button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "dmx" }); close(); }}><span className="show-navigation-icon" aria-hidden="true">◉</span><span>DMX</span></Button></div>
    <div className="modal-actions show-secondary-actions"><Button className="help-action" onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "help" }); close(); }}><span aria-hidden="true">?</span> Help</Button><Button className="danger shutdown-action" onClick={() => setConfirmShutdown(true)}><span aria-hidden="true">⏻</span> Shut Down Desk</Button></div>
    {revisionOpen && stacked(<div className="nested-modal named-revision-modal" role="dialog" aria-modal="true" aria-label="Save named revision"><Button className="modal-close" onClick={() => setRevisionOpen(false)}>×</Button><h3>Save Named Revision</h3><p>This creates a restore point from the current autosaved show. Autosave continues afterward.</p><TextInput clearable className="show-name-input" autoFocus value={revisionName} onChange={(event) => setRevisionName(event.target.value)} onKeyboardCommit={(value) => void saveNamedRevision(value)} placeholder="e.g. Before trying alternate cue timing" aria-label="Revision name"/><footer><Button onClick={() => setRevisionOpen(false)}>Cancel</Button><Button variant="primary" disabled={!revisionName.trim()} onClick={() => void saveNamedRevision()}>Save Revision {(activeRevisions[0]?.revision ?? 0) + 1}</Button></footer></div>, () => setRevisionOpen(false))}
    {saveAsOpen && stacked(<div className="nested-modal" role="dialog" aria-modal="true" aria-label="Save show"><Button className="modal-mode-switch" onClick={openMvrExport}>Export as MVR</Button><Button className="modal-close" onClick={() => setSaveAsOpen(false)}>×</Button><h3>Save Show As</h3><div className="save-destination"><Button className={destination === "local" ? "active" : ""} onClick={() => setDestination("local")}>This desk</Button>{flashDriveConnected && <Button className={destination === "flash" ? "active" : ""} onClick={() => setDestination("flash")}>Connected flash drive</Button>}</div><TextInput clearable className="show-name-input" autoFocus value={showName} onChange={(event) => setShowName(event.target.value)} onKeyboardCommit={(value) => void saveAs(value)} placeholder="Show name" aria-label="Show name"/><footer><Button onClick={() => setSaveAsOpen(false)}>Cancel</Button><Button variant="primary" disabled={!showName.trim()} onClick={() => void saveAs()}>Save show</Button></footer></div>, () => setSaveAsOpen(false))}
    {loadOpen && stacked(<div className="nested-modal load-show-modal" role="dialog" aria-modal="true" aria-label="Load show"><Button className="modal-mode-switch" onClick={() => openMvrImport(() => setLoadOpen(false))}>Load from MVR</Button><Button className="modal-close" onClick={() => setLoadOpen(false)}>×</Button><h3>Load Show</h3><p>Load Latest Autosave resumes the newest state. A named revision restores that saved point and makes it the new autosaved state.</p><div className="show-library revision-show-library">{server.shows.map((show) => <article key={show.id} className={show.id === activeShowId ? "active" : ""}><span><b>{show.name}</b><small>{(revisionsByShow[show.id] ?? []).length} named revisions</small></span><Button onClick={() => { void server.openShow(show.id); setLoadOpen(false); }}>Load Latest Autosave</Button><div className="named-revision-list">{(revisionsByShow[show.id] ?? []).map((revision) => <Button key={revision.revision} onClick={() => void loadNamedRevision(show.id, revision.revision)}><span><b>Revision {revision.revision} · {revision.name}</b><small>{new Date(revision.created_at).toLocaleString()}</small></span><i>Load Revision</i></Button>)}{(revisionsByShow[show.id] ?? []).length === 0 && <small>No manually saved revisions</small>}</div></article>)}</div><Button onClick={() => upload.current?.click()}>Load from flash drive</Button><Input ref={upload} hidden type="file" accept=".show,application/x-sqlite3" onChange={(event) => { const file = event.target.files?.[0]; if (file) void server.uploadShow(file); event.target.value = ""; }}/></div>, () => setLoadOpen(false))}
    {newShowOpen && stacked(<div className="nested-modal new-show-modal" role="dialog" aria-modal="true" aria-label="New show"><Button className="modal-mode-switch" onClick={() => openMvrImport(() => setNewShowOpen(false))}>Load from MVR</Button><Button className="modal-close" onClick={() => setNewShowOpen(false)}>×</Button><h3>New Show</h3><p>Create and open a new empty show. The current show remains saved on this desk.</p><Button className="primary" onClick={async () => { if (await server.initializeEmptyShow()) setNewShowOpen(false); }}>Create Empty Show</Button></div>, () => setNewShowOpen(false))}
    {mvrMode && stacked(<div className="nested-modal mvr-modal" role="dialog" aria-modal="true" aria-label="MVR import and export"><Button className="modal-close" onClick={() => setMvrMode(null)}>×</Button><h3>{mvrMode === "new" ? "New Show from MVR" : mvrMode === "merge" ? "Add MVR to Show" : "Export Show as MVR"}</h3>
      {mvrMode !== "new" && !mvrTarget && <><p>Select any show in the desk library.</p><div className="show-library">{server.shows.map((show) => <article key={show.id}><span><b>{show.name}</b><small>Autosaved show file</small></span><Button onClick={() => mvrMode === "export" ? void inspectExport(show) : setMvrTarget(show)}>Select</Button></article>)}</div></>}
      {mvrMode === "merge" && mvrTarget && !mvrPreview && <><p>Import into <b>{mvrTarget.name}</b>. Existing programming and unmatched scenery are retained.</p><Button className="primary" disabled={mvrBusy} onClick={() => mvrFile.current?.click()}>{mvrBusy ? "Inspecting…" : "Choose MVR file"}</Button></>}
      {mvrMode === "new" && !mvrPreview && <><p>Create a new show from MVR fixtures, patch, transforms, and scene geometry.</p><Button className="primary" disabled={mvrBusy} onClick={() => mvrFile.current?.click()}>{mvrBusy ? "Inspecting…" : "Choose MVR file"}</Button></>}
      {mvrPreview && <><div className="mvr-summary"><b>{mvrPreview.fixtures.length} fixtures · {mvrPreview.scenery} scenery objects</b>{mvrPreview.missing_profiles.length > 0 && <p className="modal-warning">{mvrPreview.missing_profiles.length} fixture profiles will be imported as unresolved.</p>}{mvrPreview.address_conflicts.map((warning) => <p className="modal-warning" key={warning}>{warning}</p>)}</div>{mvrMode === "new" && <TextInput clearable value={mvrName} onChange={(event) => setMvrName(event.target.value)} placeholder="Show name" aria-label="Show name"/>}<div className="mvr-fixture-list">{mvrPreview.fixtures.map((fixture) => { const resolution=mvrResolutions[fixture.uuid]; return <article key={fixture.uuid}><span><b>{fixture.name}</b><small>{fixture.gdtf_spec} · {fixture.gdtf_mode}{fixture.universe && fixture.address ? ` · U${fixture.universe}.${fixture.address}` : " · Unpatched"}</small></span>{mvrPreview.address_conflicts.some((warning) => warning.startsWith(fixture.name)) && <div><SelectField label={`Resolution for ${fixture.name}`} value={resolution?.action ?? "import_unpatched"} options={[{value:"import_unpatched",label:"Import unpatched"},{value:"address",label:"Choose address"},{value:"skip",label:"Skip"},{value:"replace",label:"Replace conflict"}]} onChange={(action) => setMvrResolutions((current) => ({ ...current, [fixture.uuid]: { action, universe: fixture.universe ?? 1, address: fixture.address ?? 1 } }))}/>{resolution?.action === "address" && <div className="mvr-address-fields"><NumberField label="Universe" min={1} max={65535} aria-label={`Universe for ${fixture.name}`} value={resolution.universe ?? 1} onChange={(event) => setMvrResolutions((current) => ({...current,[fixture.uuid]:{...current[fixture.uuid],action:"address",universe:Number(event.target.value)}}))}/><NumberField label="Address" min={1} max={512} aria-label={`Address for ${fixture.name}`} value={resolution.address ?? 1} onChange={(event) => setMvrResolutions((current) => ({...current,[fixture.uuid]:{...current[fixture.uuid],action:"address",address:Number(event.target.value)}}))}/></div>}</div>}</article>;})}</div><Button className="primary" disabled={mvrBusy || (mvrMode === "new" && !mvrName.trim())} onClick={() => void applyMvr()}>{mvrBusy ? "Importing…" : mvrMode === "new" ? "Create and Open Show" : `Add to ${mvrTarget?.name}`}</Button></>}
      {mvrMode === "export" && mvrTarget && mvrExportPreview && <><div className="mvr-summary"><b>{mvrExportPreview.fixtures} fixtures · {mvrExportPreview.scenery} scenery objects</b><p>Not included: {mvrExportPreview.omitted.join(", ")}</p>{mvrExportPreview.warnings.map((warning) => <p className="modal-warning" key={warning}>{warning}</p>)}</div><Button className="primary" onClick={() => { void server.downloadMvr(mvrTarget); setMvrMode(null); }}>Download {mvrTarget.name}.mvr</Button></>}
      <Input ref={mvrFile} hidden type="file" accept=".mvr,application/zip" onChange={(event) => { const file=event.target.files?.[0]; if(file) void inspectMvr(file); event.currentTarget.value=""; }}/>
    </div>, () => setMvrMode(null))}
    {changeUserOpen && stacked(<div className="nested-modal" role="dialog" aria-modal="true" aria-label="Change user"><Button className="modal-close" onClick={() => setChangeUserOpen(false)}>×</Button><h3>Change User</h3><div className="show-library">{server.bootstrap?.users.filter((user) => user.enabled).map((user) => <article key={user.id}><span><b>{user.name}</b><small>{user.id === server.session?.user.id ? "Current user" : "Use this user's programmer"}</small></span><Button disabled={user.id === server.session?.user.id} onClick={() => void server.changeUser(user)}>{user.id === server.session?.user.id ? "Logged in" : "Log in"}</Button></article>)}</div><div className="user-create-row"><TextInput clearable value={newUserName} onChange={(event) => setNewUserName(event.target.value)} onKeyboardCommit={(value) => { if (value.trim()) void server.createUser(value.trim()); }} placeholder="New user name" aria-label="New user name"/><Button variant="primary" disabled={!newUserName.trim()} onClick={() => void server.createUser(newUserName.trim())}>Add user</Button></div></div>, () => setChangeUserOpen(false))}
    {confirmShutdown && stacked(<div className="nested-modal shutdown-modal" role="alertdialog" aria-modal="true"><Button className="modal-close" onClick={() => setConfirmShutdown(false)}>×</Button><h3>Shut Down Desk?</h3><p>Hazardous fixtures will be driven to their safe values before the server stops. This desk application will then close.</p><div className="modal-actions"><Button onClick={() => setConfirmShutdown(false)}>Cancel</Button><Button className="danger" onClick={() => void shutDownDesk()}>Shut Down Safely</Button></div></div>, () => setConfirmShutdown(false))}
    {server.error && <p className="modal-error">{server.error}</p>}
    <DebugModal />
  </section></div>;
}
