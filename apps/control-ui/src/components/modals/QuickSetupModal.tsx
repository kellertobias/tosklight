import {
	type Dispatch,
	type ReactNode,
	type SetStateAction,
	useEffect,
	useRef,
	useState,
} from "react";
import { createPortal } from "react-dom";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { useScreens } from "../../features/screens/ScreensContext";
import { Button, Input, ModalTitleBar, NumberField, SelectField, TextInput } from "../common";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";
import type { MvrExportPreview, MvrImportPreview, ShowEntry, ShowRevision } from "../../api/types";
import { getShowIndicator } from "../shell/showIndicator";
import { screenForAddAction } from "../setup/screenConfiguration";
import { useDesktopBridge } from "../../platform/desktop";
import { SelectiveShowImportModal } from "./SelectiveShowImportModal";
import { useSelectiveImport } from "../../features/selectiveImport/SelectiveImportContext";

interface QuickSetupKeyboardOptions {
	enabled: boolean;
	revisionOpen: boolean;
	saveAsOpen: boolean;
	changeUserOpen: boolean;
	newUserName: string;
	closeTopLayer: () => void;
	saveNamedRevision: () => Promise<void>;
	saveAs: () => Promise<void>;
	createUser: (name: string) => Promise<void>;
	setRevisionName: Dispatch<SetStateAction<string>>;
	setShowName: Dispatch<SetStateAction<string>>;
	setNewUserName: Dispatch<SetStateAction<string>>;
}

function useQuickSetupKeyboard(options: QuickSetupKeyboardOptions) {
	const optionsRef = useRef(options);
	optionsRef.current = options;
	useEffect(() => {
		if (!options.enabled) return;
		const handle = (event: KeyboardEvent) => {
			if (document.querySelector(".ui-input-modal-layer")) return;
			const current = optionsRef.current;
			if (event.key === "Escape") {
				event.preventDefault();
				event.stopImmediatePropagation();
				current.closeTopLayer();
				return;
			}
			if (!current.revisionOpen && !current.saveAsOpen && !current.changeUserOpen)
				return;
			event.preventDefault();
			event.stopImmediatePropagation();
			if (event.key === "Enter") {
				if (current.revisionOpen) void current.saveNamedRevision();
				else if (current.saveAsOpen) void current.saveAs();
				else if (current.newUserName.trim())
					void current.createUser(current.newUserName.trim());
				return;
			}
			const setValue = current.revisionOpen
				? current.setRevisionName
				: current.saveAsOpen
					? current.setShowName
					: current.setNewUserName;
			if (event.key === "Backspace") setValue((value) => value.slice(0, -1));
			else if (event.key.length === 1) setValue((value) => value + event.key);
		};
		window.addEventListener("keydown", handle, true);
		return () => window.removeEventListener("keydown", handle, true);
	}, [options.enabled, optionsRef]);
}

interface ShowRevisionControllerOptions {
	enabled: boolean;
	activeShowId?: string;
	revisionName: string;
	shows: ShowEntry[];
	listShowRevisions: (showId: string) => Promise<ShowRevision[]>;
	saveShowRevision: (name: string) => Promise<ShowRevision | null>;
	openShowRevision: (showId: string, revision: number) => Promise<boolean>;
	setRevisionName: Dispatch<SetStateAction<string>>;
	setRevisionOpen: Dispatch<SetStateAction<boolean>>;
	setLoadOpen: Dispatch<SetStateAction<boolean>>;
}

function useShowRevisionController(options: ShowRevisionControllerOptions) {
	const optionsRef = useRef(options);
	optionsRef.current = options;
	const [byShow, setByShow] = useState<Record<string, ShowRevision[]>>({});
	useEffect(() => {
		if (!options.enabled || !options.activeShowId) return;
		const showId = options.activeShowId;
		void optionsRef.current.listShowRevisions(showId).then((revisions) =>
			setByShow((current) => ({ ...current, [showId]: revisions })),
		);
	}, [options.enabled, options.activeShowId, optionsRef]);
	const saveNamed = async (value = optionsRef.current.revisionName) => {
		const current = optionsRef.current;
		const name = value.trim();
		if (!name || !current.activeShowId) return;
		const showId = current.activeShowId;
		const revision = await current.saveShowRevision(name);
		if (!revision) return;
		setByShow((loaded) => ({
			...loaded,
			[showId]: [revision, ...(loaded[showId] ?? [])],
		}));
		current.setRevisionName("");
		current.setRevisionOpen(false);
	};
	const openLoadMenu = async () => {
		const current = optionsRef.current;
		current.setLoadOpen(true);
		const entries = await Promise.all(
			current.shows.map(async (show) => [
				show.id,
				await current.listShowRevisions(show.id),
			] as const),
		);
		setByShow(Object.fromEntries(entries));
	};
	const loadNamed = async (showId: string, revision: number) => {
		const current = optionsRef.current;
		if (await current.openShowRevision(showId, revision))
			current.setLoadOpen(false);
	};
	return {
		activeRevisions: options.activeShowId ? byShow[options.activeShowId] ?? [] : [],
		byShow,
		loadNamed,
		openLoadMenu,
		saveNamed,
	};
}

function AddScreenAction({
	layout,
	onAdded,
}: {
	layout: Parameters<typeof screenForAddAction>[1];
	onAdded: () => void;
}) {
	const screens = useScreens();
	const add = async () => {
		await screens.saveScreen(
			screenForAddAction(screens.screens?.screens ?? [], layout),
		);
		onAdded();
	};
	return (
		<Button onClick={() => void add()}>
			<span aria-hidden="true">▣</span> Add Screen
		</Button>
	);
}

function stackedModal(content: ReactNode, closeLayer: () => void) {
	return createPortal(
		<div
			className="stacked-modal-layer"
			onPointerDown={(event) => {
				if (event.target === event.currentTarget) closeLayer();
			}}
		>
			{content}
		</div>,
		document.body,
	);
}

export function QuickSetupModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  const selectiveImport = useSelectiveImport();
  const desktop = useDesktopBridge();
  const [showName, setShowName] = useState("");
  const [revisionOpen, setRevisionOpen] = useState(false);
  const [revisionName, setRevisionName] = useState("");
  const [saveAsOpen, setSaveAsOpen] = useState(false);
  const [copySaveOpen, setCopySaveOpen] = useState(false);
  const [overwriteTarget, setOverwriteTarget] = useState<ShowEntry | null>(null);
  const [overwriteBusy, setOverwriteBusy] = useState(false);
  const [loadOpen, setLoadOpen] = useState(false);
  const [selectiveImportOpen, setSelectiveImportOpen] = useState(false);
  const selectiveImportClose = useRef<(() => void) | null>(null);
  const usbShowPickerTrigger = useRef<(() => void) | null>(null);
  const osShowPickerInput = useRef<HTMLInputElement | null>(null);
  const [newShowOpen, setNewShowOpen] = useState(false);
  const [confirmShutdown, setConfirmShutdown] = useState(false);
  const [changeUserOpen, setChangeUserOpen] = useState(false);
  const [newUserName, setNewUserName] = useState("");
  const [destination, setDestination] = useState<"local" | "flash">("local");
  const [mvrMode, setMvrMode] = useState<"new" | "merge" | "export" | null>(null);
  const [mvrTarget, setMvrTarget] = useState<ShowEntry | null>(null);
  const [mvrPreview, setMvrPreview] = useState<MvrImportPreview | null>(null);
  const [mvrExportPreview, setMvrExportPreview] = useState<MvrExportPreview | null>(null);
  const [mvrName, setMvrName] = useState("");
  const [mvrBusy, setMvrBusy] = useState(false);
  const [mvrResolutions, setMvrResolutions] = useState<Record<string, { action: string; universe?: number; address?: number }>>({});
  const flashDriveConnected = false;
  const showIndicator = getShowIndicator(server.status);
  const activeShow = server.bootstrap?.active_show;
  const activeShowIsProvisional = /^New Empty Show(?: [1-9]\d*)?$/.test(activeShow?.name ?? "");
  const activeShowId = activeShow?.id;
  const revisionCopy = activeShow?.revision_copy;
  const originalShow = revisionCopy ? server.shows.find((show) => show.id === revisionCopy.show_id) : undefined;
  const close = () => dispatch({ type: "SET_MODAL", modal: "setupOpen", value: false });
  const {
    activeRevisions, byShow: revisionsByShow, loadNamed: loadNamedRevision,
    openLoadMenu, saveNamed: saveNamedRevision,
  } = useShowRevisionController({
    enabled: state.setupOpen, activeShowId, revisionName, shows: server.shows,
    listShowRevisions: server.listShowRevisions, saveShowRevision: server.saveShowRevision,
    openShowRevision: server.openShowRevision,
    setRevisionName, setRevisionOpen, setLoadOpen,
  });
  function closeTopLayer() {
    if (overwriteTarget && !overwriteBusy) setOverwriteTarget(null);
    else if (copySaveOpen) setCopySaveOpen(false);
    else if (revisionOpen) setRevisionOpen(false);
    else if (saveAsOpen) setSaveAsOpen(false);
    else if (changeUserOpen) setChangeUserOpen(false);
    else if (selectiveImportOpen) selectiveImportClose.current?.();
    else if (loadOpen) setLoadOpen(false);
    else if (newShowOpen) setNewShowOpen(false);
    else if (confirmShutdown) setConfirmShutdown(false);
    else close();
  }
  useQuickSetupKeyboard({
    enabled: state.setupOpen,
    revisionOpen,
    saveAsOpen,
    changeUserOpen,
    newUserName,
    closeTopLayer,
    saveNamedRevision,
    saveAs: () => saveAs(),
    createUser: server.createUser,
    setRevisionName,
    setShowName,
    setNewUserName,
  });
  if (!state.setupOpen) return null;
  async function saveAs(value = showName) {
    const name = value.trim();
    if (!name) return;
    if (!await server.saveShowAs(name)) return;
    if (destination === "flash" && server.bootstrap?.active_show) await server.downloadShow({ ...server.bootstrap.active_show, name });
    setSaveAsOpen(false);
    setShowName("");
  }
  function requestOverwrite(show: ShowEntry) {
    setSaveAsOpen(false);
    setCopySaveOpen(false);
    setOverwriteTarget(show);
  }
  async function confirmOverwrite() {
    if (!overwriteTarget) return;
    setOverwriteBusy(true);
    try {
      if (!await server.overwriteShow(overwriteTarget.id)) return;
      setOverwriteTarget(null);
    } finally {
      setOverwriteBusy(false);
    }
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
    if (desktop.available) await desktop.exitApplication();
  }
  async function lockDesk() {
    close();
    await server.lockDesk();
  }
  async function inspectExport(show: ShowEntry) { setMvrTarget(show); setMvrBusy(true); try { setMvrExportPreview(await server.previewMvrExport(show.id)); } finally { setMvrBusy(false); } }
  function openMvrImport(closeSource: () => void) { closeSource(); setMvrMode("new"); setMvrTarget(null); setMvrPreview(null); }
  function openMvrExport() {
    setSaveAsOpen(false); setMvrMode("export"); setMvrExportPreview(null);
    const active = server.bootstrap?.active_show;
    if (active) void inspectExport(active); else setMvrTarget(null);
  }
  return <div className="modal-backdrop" onPointerDown={(event) => { if (event.currentTarget === event.target) close(); }}><section className="modal-card show-modal" role="dialog" aria-modal="true" aria-label="Show">
    <ModalTitleBar title="Show" closeLabel="Close Show" onClose={close} actions={<>
      <Button onClick={() => setChangeUserOpen(true)}><span aria-hidden="true">♙</span> Change User</Button>
      {desktop.available && <AddScreenAction layout={{ desks: state.desks, activeDeskId: state.activeDeskId }} onAdded={close} />}
      <Button onClick={() => dispatch({ type: "SET_MODAL", modal: "debugOpen", value: true })}><span aria-hidden="true">⌁</span> Desk Status</Button>
    </>}/>
    <div className="show-details"><b>{activeShow?.name ?? "No active show"}</b>{revisionCopy && <div className="revision-copy-notice" role="status"><strong>Separate revision copy</strong><span>Created from <b>{revisionCopy.show_name}</b>, Revision {revisionCopy.revision} · {revisionCopy.revision_name}</span><small>Created {new Date(revisionCopy.copied_at).toLocaleString()}. Current changes are autosaved to this copy, not to {revisionCopy.show_name}.</small></div>}<div className={`show-status-explanation ${showIndicator.className}`} role="status"><span className="show-status-dot" aria-hidden="true">●</span><span><strong>{showIndicator.label}</strong><small>{showIndicator.detail}</small></span></div><span>Server connected <strong>{server.status === "connected" ? "Yes" : "No"}</strong></span><span>Latest named revision <strong>{activeRevisions[0] ? `${activeRevisions[0].revision} · ${activeRevisions[0].name}` : "None"}</strong></span><span>Operator <strong>{server.session?.user.name ?? "—"}</strong></span><div className="show-primary-actions"><Button onClick={() => setRevisionOpen(true)}><span aria-hidden="true">💾</span> Save Named Revision</Button>{revisionCopy && <Button onClick={() => setCopySaveOpen(true)}><span aria-hidden="true">✓</span> Save</Button>}<Button onClick={() => setSaveAsOpen(true)}><span aria-hidden="true">✎</span> Save As</Button><Button onClick={() => void openLoadMenu()}><span aria-hidden="true">↥</span> Load</Button><Button onClick={() => setNewShowOpen(true)}><span aria-hidden="true">＋</span> New Show</Button></div></div>
    <div className="show-navigation-primary"><Button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "patch" }); close(); }}><span className="show-navigation-icon" aria-hidden="true">▦</span><span>Show Patch</span></Button><Button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "setup" }); close(); }}><span className="show-navigation-icon" aria-hidden="true">⚙</span><span>Enter Setup</span></Button><Button onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "dmx" }); close(); }}><span className="show-navigation-icon" aria-hidden="true">◉</span><span>DMX</span></Button></div>
    <div className="modal-actions show-secondary-actions"><Button className="help-action" onClick={() => { dispatch({ type: "OPEN_BUILTIN", kind: "help" }); close(); }}><span aria-hidden="true">?</span> Help</Button><Button variant="warning" className="lock-action" onClick={() => void lockDesk()}><span aria-hidden="true">🔒</span> Lock Desk</Button><Button className="danger shutdown-action" onClick={() => setConfirmShutdown(true)}><span aria-hidden="true">⏻</span> Shut Down Desk</Button></div>
    {revisionOpen && stackedModal(<div className="nested-modal named-revision-modal" role="dialog" aria-modal="true" aria-label="Save named revision"><Button className="modal-close" onClick={() => setRevisionOpen(false)}>×</Button><h3>Save Named Revision</h3><p>This creates a restore point from the current autosaved show. Autosave continues afterward.</p><TextInput clearable className="show-name-input" autoFocus value={revisionName} onChange={(event) => setRevisionName(event.target.value)} onKeyboardCommit={(value) => void saveNamedRevision(value)} placeholder="e.g. Before trying alternate cue timing" aria-label="Revision name"/><footer><Button onClick={() => setRevisionOpen(false)}>Cancel</Button><Button variant="primary" disabled={!revisionName.trim()} onClick={() => void saveNamedRevision()}>Save Revision {(activeRevisions[0]?.revision ?? 0) + 1}</Button></footer></div>, () => setRevisionOpen(false))}
    {copySaveOpen && stackedModal(<div className="nested-modal revision-copy-save-modal" role="dialog" aria-modal="true" aria-label="Save revision copy"><Button className="modal-close" onClick={() => setCopySaveOpen(false)}>×</Button><h3>Save Revision Copy</h3><p>Autosave already protects this copy. Choose where this copy should remain.</p><div className="dialog-grid"><Button variant="primary" onClick={() => setCopySaveOpen(false)}>Keep as Separate Show</Button>{originalShow ? <Button onClick={() => requestOverwrite(originalShow)}>Overwrite Original Show</Button> : <p className="modal-warning">The original show is no longer available. This copy remains an independent show.</p>}<Button onClick={() => setCopySaveOpen(false)}>Cancel</Button></div></div>, () => setCopySaveOpen(false))}
    {saveAsOpen && stackedModal(<div className="nested-modal" role="dialog" aria-modal="true" aria-label="Save show"><ModalTitleBar title={activeShowIsProvisional ? "Name Empty Show" : "Save Show As"} actions={<><Button onClick={openMvrExport}>Export as MVR</Button><Button variant="primary" disabled={!showName.trim()} onClick={() => void saveAs()}>{activeShowIsProvisional ? "Name Empty Show" : "Save as New Show"}</Button></>} closeLabel="Close Save Show" onClose={() => setSaveAsOpen(false)}/>{activeShowIsProvisional && <p>This empty show is already autosaved. Naming it keeps the same show and all current programming.</p>}<div className="save-destination"><Button className={destination === "local" ? "active" : ""} onClick={() => setDestination("local")}>This desk</Button>{flashDriveConnected && <Button className={destination === "flash" ? "active" : ""} onClick={() => setDestination("flash")}>Connected flash drive</Button>}</div><TextInput clearable className="show-name-input" autoFocus value={showName} onChange={(event) => setShowName(event.target.value)} onKeyboardCommit={(value) => void saveAs(value)} placeholder="New show name" aria-label="Show name"/>{!activeShowIsProvisional && server.shows.some((show) => show.id !== activeShowId) && <><h4>Or replace an existing Latest Autosave</h4><div className="show-library overwrite-destination-list">{server.shows.filter((show) => show.id !== activeShowId).map((show) => <article key={show.id}><span><b>{show.name}</b><small>{show.id === revisionCopy?.show_id ? "Original show" : "Existing show"}</small></span><Button onClick={() => requestOverwrite(show)}>Choose Destination</Button></article>)}</div></>}</div>, () => setSaveAsOpen(false))}
    {overwriteTarget && stackedModal(<div className="nested-modal overwrite-show-confirm" role="alertdialog" aria-modal="true" aria-label={`Confirm overwrite ${overwriteTarget.name}`}><h3>Replace {overwriteTarget.name} Latest Autosave?</h3><p>This replaces only <b>{overwriteTarget.name}</b>&apos;s mutable Latest Autosave with the active show state. Its identity and named revisions are preserved.</p><p>The active revision copy and its immutable source revision are retained.</p><div className="modal-actions"><Button autoFocus disabled={overwriteBusy} onClick={() => setOverwriteTarget(null)}>Cancel</Button><Button className="danger" disabled={overwriteBusy} onClick={() => void confirmOverwrite()}>{overwriteBusy ? "Replacing Latest Autosave…" : `Replace ${overwriteTarget.name} Latest Autosave`}</Button></div>{server.error && <p className="modal-error" role="alert">{server.error}</p>}</div>, () => { if (!overwriteBusy) setOverwriteTarget(null); })}
    {loadOpen && stackedModal(<div className="nested-modal load-show-modal" role="dialog" aria-modal="true" aria-label="Load show"><ModalTitleBar title="Load Show" actions={<><Button onClick={() => { setLoadOpen(false); setSelectiveImportOpen(true); }}>Partial Show Load</Button><Button onClick={() => openMvrImport(() => setLoadOpen(false))}>Load from MVR</Button><Button onClick={() => usbShowPickerTrigger.current?.()}>Show from USB</Button><Button onClick={() => osShowPickerInput.current?.click()}>Show from OS</Button><Input ref={osShowPickerInput} hidden type="file" accept=".show" onChange={(event) => { const file = event.target.files?.[0]; if (file) void server.uploadShow(file); event.target.value = ""; }}/></>} closeLabel="Close Load Show" onClose={() => setLoadOpen(false)}/><p>Load Latest Autosave always resumes that show&apos;s newest work. Load Clean Built-in Default creates a separate show from the untouched built-in rig. Load Revision as Copy creates and activates a separate autosaved show without changing the original.</p><div className="show-library revision-show-library"><article className="built-in-default-show"><span><b>Built-in Default Stage Show</b><small>Untouched 49-fixture factory rig</small></span><Button variant="primary" onClick={async () => { if (await server.openCleanDefaultShow()) setLoadOpen(false); }}>Load Clean Built-in Default</Button></article>{server.shows.map((show) => <article key={show.id} className={show.id === activeShowId ? "active" : ""}><span><b>{show.name}</b><small>{(revisionsByShow[show.id] ?? []).length} named revisions</small></span><Button onClick={() => { void server.openShow(show.id); setLoadOpen(false); }}>Load Latest Autosave</Button><div className="named-revision-list">{(revisionsByShow[show.id] ?? []).map((revision) => <Button key={revision.revision} onClick={() => void loadNamedRevision(show.id, revision.revision)}><span><b>Revision {revision.revision} · {revision.name}</b><small>{new Date(revision.created_at).toLocaleString()}</small></span><i>Load Revision as Copy</i></Button>)}{(revisionsByShow[show.id] ?? []).length === 0 && <small>No manually saved revisions</small>}</div></article>)}</div><RootConfinedFilePickerButton hideButton triggerRef={usbShowPickerTrigger} label="Show from USB" allowedExtensions={["show"]} onFiles={(files) => { const file = files[0]; if (file) return server.uploadShow(file); }}/></div>, () => setLoadOpen(false))}
    {selectiveImportOpen && activeShow && stackedModal(<SelectiveShowImportModal activeShow={activeShow} shows={server.shows} closeTriggerRef={selectiveImportClose} onClose={() => setSelectiveImportOpen(false)} loadCatalog={selectiveImport.catalog} previewImport={selectiveImport.preview} applyImport={selectiveImport.apply}/>, () => selectiveImportClose.current?.())}
    {newShowOpen && stackedModal(<div className="nested-modal new-show-modal" role="dialog" aria-modal="true" aria-label="New show"><Button className="modal-mode-switch" onClick={() => openMvrImport(() => setNewShowOpen(false))}>Load from MVR</Button><Button className="modal-close" onClick={() => setNewShowOpen(false)}>×</Button><h3>New Show</h3><p>Create and open a new empty show. The current show remains saved on this desk.</p><Button className="primary" onClick={async () => { if (await server.initializeEmptyShow()) setNewShowOpen(false); }}>Create Empty Show</Button></div>, () => setNewShowOpen(false))}
    {mvrMode && stackedModal(<div className="nested-modal mvr-modal" role="dialog" aria-modal="true" aria-label="MVR import and export"><Button className="modal-close" onClick={() => setMvrMode(null)}>×</Button><h3>{mvrMode === "new" ? "New Show from MVR" : mvrMode === "merge" ? "Add MVR to Show" : "Export Show as MVR"}</h3>
      {mvrMode !== "new" && !mvrTarget && <><p>Select any show in the desk library.</p><div className="show-library">{server.shows.map((show) => <article key={show.id}><span><b>{show.name}</b><small>Autosaved show file</small></span><Button onClick={() => mvrMode === "export" ? void inspectExport(show) : setMvrTarget(show)}>Select</Button></article>)}</div></>}
      {mvrMode === "merge" && mvrTarget && !mvrPreview && <><p>Import into <b>{mvrTarget.name}</b>. Existing programming and unmatched scenery are retained.</p><RootConfinedFilePickerButton variant="primary" disabled={mvrBusy} label={mvrBusy ? "Inspecting…" : "Choose MVR file"} allowedExtensions={["mvr"]} onFiles={(files) => { const file = files[0]; if (file) return inspectMvr(file); }}/></>}
      {mvrMode === "new" && !mvrPreview && <><p>Create a new show from MVR fixtures, patch, transforms, and scene geometry.</p><RootConfinedFilePickerButton variant="primary" disabled={mvrBusy} label={mvrBusy ? "Inspecting…" : "Choose MVR file"} allowedExtensions={["mvr"]} onFiles={(files) => { const file = files[0]; if (file) return inspectMvr(file); }}/></>}
      {mvrPreview && <><div className="mvr-summary"><b>{mvrPreview.fixtures.length} fixtures · {mvrPreview.scenery} scenery objects</b>{mvrPreview.missing_profiles.length > 0 && <p className="modal-warning">{mvrPreview.missing_profiles.length} fixture profiles will be imported as unresolved.</p>}{mvrPreview.address_conflicts.map((warning) => <p className="modal-warning" key={warning}>{warning}</p>)}</div>{mvrMode === "new" && <TextInput clearable value={mvrName} onChange={(event) => setMvrName(event.target.value)} placeholder="Show name" aria-label="Show name"/>}<div className="mvr-fixture-list">{mvrPreview.fixtures.map((fixture) => { const resolution=mvrResolutions[fixture.uuid]; return <article key={fixture.uuid}><span><b>{fixture.name}</b><small>{fixture.gdtf_spec} · {fixture.gdtf_mode}{fixture.universe && fixture.address ? ` · U${fixture.universe}.${fixture.address}` : " · Unpatched"}</small></span>{mvrPreview.address_conflicts.some((warning) => warning.startsWith(fixture.name)) && <div><SelectField label={`Resolution for ${fixture.name}`} value={resolution?.action ?? "import_unpatched"} options={[{value:"import_unpatched",label:"Import unpatched"},{value:"address",label:"Choose address"},{value:"skip",label:"Skip"},{value:"replace",label:"Replace conflict"}]} onChange={(action) => setMvrResolutions((current) => ({ ...current, [fixture.uuid]: { action, universe: fixture.universe ?? 1, address: fixture.address ?? 1 } }))}/>{resolution?.action === "address" && <div className="mvr-address-fields"><NumberField label="Universe" min={1} max={65535} aria-label={`Universe for ${fixture.name}`} value={resolution.universe ?? 1} onChange={(event) => setMvrResolutions((current) => ({...current,[fixture.uuid]:{...current[fixture.uuid],action:"address",universe:Number(event.target.value)}}))}/><NumberField label="Address" min={1} max={512} aria-label={`Address for ${fixture.name}`} value={resolution.address ?? 1} onChange={(event) => setMvrResolutions((current) => ({...current,[fixture.uuid]:{...current[fixture.uuid],action:"address",address:Number(event.target.value)}}))}/></div>}</div>}</article>;})}</div><Button className="primary" disabled={mvrBusy || (mvrMode === "new" && !mvrName.trim())} onClick={() => void applyMvr()}>{mvrBusy ? "Importing…" : mvrMode === "new" ? "Create and Open Show" : `Add to ${mvrTarget?.name}`}</Button></>}
      {mvrMode === "export" && mvrTarget && mvrExportPreview && <><div className="mvr-summary"><b>{mvrExportPreview.fixtures} fixtures · {mvrExportPreview.scenery} scenery objects</b><p>Not included: {mvrExportPreview.omitted.join(", ")}</p>{mvrExportPreview.warnings.map((warning) => <p className="modal-warning" key={warning}>{warning}</p>)}</div><Button className="primary" onClick={() => { void server.downloadMvr(mvrTarget); setMvrMode(null); }}>Download {mvrTarget.name}.mvr</Button></>}
    </div>, () => setMvrMode(null))}
    {changeUserOpen && stackedModal(<div className="nested-modal" role="dialog" aria-modal="true" aria-label="Change user"><Button className="modal-close" onClick={() => setChangeUserOpen(false)}>×</Button><h3>Change User</h3><div className="show-library">{server.bootstrap?.users.filter((user) => user.enabled).map((user) => <article key={user.id}><span><b>{user.name}</b><small>{user.id === server.session?.user.id ? "Current user" : "Use this user's programmer"}</small></span><Button disabled={user.id === server.session?.user.id} onClick={() => void server.changeUser(user)}>{user.id === server.session?.user.id ? "Logged in" : "Log in"}</Button></article>)}</div><div className="user-create-row"><TextInput clearable value={newUserName} onChange={(event) => setNewUserName(event.target.value)} onKeyboardCommit={(value) => { if (value.trim()) void server.createUser(value.trim()); }} placeholder="New user name" aria-label="New user name"/><Button variant="primary" disabled={!newUserName.trim()} onClick={() => void server.createUser(newUserName.trim())}>Add user</Button></div></div>, () => setChangeUserOpen(false))}
    {confirmShutdown && stackedModal(<div className="nested-modal shutdown-modal" role="alertdialog" aria-modal="true"><Button className="modal-close" onClick={() => setConfirmShutdown(false)}>×</Button><h3>Shut Down Desk?</h3><p>Hazardous fixtures will be driven to their safe values before the server stops. This desk application will then close.</p><div className="modal-actions"><Button onClick={() => setConfirmShutdown(false)}>Cancel</Button><Button className="danger" onClick={() => void shutDownDesk()}>Shut Down Safely</Button></div></div>, () => setConfirmShutdown(false))}
    {server.error && <p className="modal-error">{server.error}</p>}
  </section></div>;
}
