import { formatErrorDetails } from "@tosklight/ui";
import { ErrorAlert } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { Button, ModalFrame, SelectField, SwitchField, TextInput } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type { FileEntry, FileRoot, NetworkShowPeer } from "../../api/types";
import { useFiles } from "../../features/files/FilesContext";
import { validItemName } from "../../windows/fileManagerWindow/fileUtilities";
import type { QuickSetupModel } from "./QuickSetupModal";
import { MvrExportSummary, type MvrExportReport } from "./MvrExportSummary";
import { ShowEntryName, ShowLocationBar } from "./showBrowserParts";

type Source = "internal" | "usb" | "network";
const sources = [{id:"internal",label:"Internal"},{id:"usb",label:"USB"},{id:"network",label:"Network"}] as const;
const isShowFile = (entry: FileEntry) => entry.kind === "file" && entry.name.toLowerCase().endsWith(".show");
const showStem = (name: string) => name.replace(/\.show$/i, "");
const dateLabel = (value: number | null) => value == null ? "—" : new Date(value).toLocaleString();

/** Save destinations are confined server folders, including mounted shares and discovered desks. */
export function ShowSaveBrowser({model}: {model: QuickSetupModel}) {
    const files = useFiles();
    const lifecycle = model.authorities.lifecycle;
    const dialogs = model.dialogs;
    const [source, setSource] = useState<Source>("internal");
    const [roots, setRoots] = useState<FileRoot[]>([]);
    const [peer, setPeer] = useState<NetworkShowPeer | null>(null);
    const [rootId, setRootId] = useState("shows");
    const [path, setPath] = useState("");
    const [saving, setSaving] = useState(false);
    const [locationOpen, setLocationOpen] = useState(false);
    const [folderDraft, setFolderDraft] = useState<string | null>(null);
    const [replaceTarget, setReplaceTarget] = useState<FileEntry | null>(null);
    const [error, setError] = useState("");
    const {entries, peers, remoteRoots, reading, reread} = useDestinationListing({files, lifecycle, source, peer, rootId, path, locationOpen, setRootId, setError});
    const [status, setStatus] = useState("");
    const [exported, setExported] = useState<MvrExportReport | null>(null);
    const busy = (locationOpen && reading) || saving;
    const availableRoots = peer ? remoteRoots : roots.filter(root =>
        (source === "internal" ? root.id === "shows" : source === "usb" ? root.removable && !root.network : root.network));
    const root = availableRoots.find(item => item.id === rootId);
    const localDefault = source === "internal" && rootId === "shows" && !path;
    const target = {rootId, path, ...(peer ? {instance:peer.instance} : {})};
    const folders = entries.filter(item => item.kind === "folder");
    const shows = entries.filter(isShowFile);
    const typedName = dialogs.showName.trim();
    const existingShow = shows.find(item => showStem(item.name).toLowerCase() === showStem(typedName).toLowerCase());
    const close = () => {if (!saving) dialogs.setSaveAsOpen(false);};

    useEffect(() => {
        let current = true;
        void files.fileRoots().then(found => {if(current) setRoots(found);}).catch(reason => {if(current) setError(String(reason));});
        return () => {current = false;};
    }, [files]);
    function navigate(next: string) {
        setPath(next); setFolderDraft(null); setStatus("");
    }
    function switchSource(next: Source, driveId?: string) {
        setSource(next); setPeer(null); setPath(""); setStatus(""); setExported(null); setFolderDraft(null); setLocationOpen(true);
        setRootId(next === "internal" ? "shows" : next === "usb" ? driveId ?? roots.find(item => item.removable && !item.network)?.id ?? "" : "");
    }
    async function createFolder() {
        const name = folderDraft?.trim() ?? "";
        if (!validItemName(name) || busy || peer || !root?.writable) return;
        setSaving(true); setError(""); setStatus("Creating folder…");
        try {
            await files.fileOperation(rootId, {operation:"create_folder", destination:path, name});
            reread();
            setFolderDraft(null);
            setPath(path ? `${path}/${name}` : name);
            setStatus(`Created folder ${name}. Shows now save here.`);
        } catch(reason) {setError(formatErrorDetails(reason)); setStatus("");}
        finally {setSaving(false);}
    }
    async function save(overwrite = false) {
        const name = dialogs.showName.trim();
        if (!name || busy || !root?.writable) return;
        if (existingShow && !overwrite) { setReplaceTarget(existingShow); return; }
        setReplaceTarget(null);
        setSaving(true); setError(""); setExported(null); setStatus(overwrite ? "Replacing show…" : "Saving show…");
        try {
            if (localDefault && !overwrite) {
                if (!(await model.actions.saveAs(name))) throw new Error("Could not save this show. Check the name and desk connection, then try again.");
            }
            else {
                const saved = await lifecycle!.saveShowCopy(name, target, dialogs.baseShow, overwrite);
                setStatus(`${overwrite ? "Replaced" : "Saved"} ${saved.name} in ${peer?.name ?? root.label} / ${path || ""}`);
                reread();
            }
        } catch(reason) {
            const message = formatErrorDetails(reason);
            // Another desk or operator may have created the file since this folder was read.
            if (!overwrite && /already exists/i.test(message) && !/folder/i.test(message)) {
                setReplaceTarget({name:`${showStem(name)}.show`, path:"", kind:"file", size:0, modified_millis:null, created_millis:null, hidden:false, writable:true});
                setStatus(""); reread();
            } else {setError(message); setStatus("");}
        }
        finally {setSaving(false);}
    }
    useEffect(() => {
        dialogs.saveDestinationSubmit.current = {save:() => save(), busy:saving};
        return () => {dialogs.saveDestinationSubmit.current = null;};
    });
    async function exportMvr() {
        if (!root?.writable || busy) return;
        setSaving(true); setError(""); setExported(null); setStatus("Exporting MVR…");
        try {
            const saved = await lifecycle!.exportMvrFile(dialogs.showName.trim() || model.view.activeShow?.name || "Show", target);
            setExported({...saved, location: peer?.name ?? root.label}); setStatus("");
        } catch(reason) {setError(formatErrorDetails(reason));setStatus("");}
        finally {setSaving(false);}
    }
    const sourceLabel = source === "usb" ? `USB: ${root?.label ?? "No drive connected"}` : sources.find(item=>item.id===source)?.label;
    const locationLabel = `Location: ${peer ? `${peer.name} · ` : ""}${root?.label ?? "Choose a destination"}${root ? ` / ${path || ""}` : ""}`;
    const title = model.view.activeShowIsProvisional && localDefault ? "Name Empty Show" : "Save Show As";
    const browsingDestinations = source === "network" && !rootId && !peer;
    const canCreateFolder = Boolean(root?.writable) && !peer && !browsingDestinations;
    const acceptLabel = existingShow ? "Replace Show" : model.view.activeShowIsProvisional && localDefault ? "Name Empty Show" : "Save as New Show";
    return <><ModalFrame title={title} ariaLabel="Save show" dialogClassName="nested-modal save-show-modal"
        closeLabel="Close Save Show" closeDisabled={saving} policy={{escape:!saving && !replaceTarget,backdrop:!saving && !replaceTarget}} onClose={close}
        groups={[{id:"export",actions:[{id:"export-mvr",label:"Export MVR",disabled:busy || !root?.writable || !model.view.activeShow,onPress:()=>void exportMvr()}]},{id:"save-source",actions:[{id:"source",kind:"dropdown",ariaLabel:`Source: ${sourceLabel}`,label:<>Source: {sourceLabel} <span aria-hidden="true">⌄</span></>,disabled:busy,
            dropdown:{kind:"items",ariaLabel:"Show source",items:[{kind:"action",id:"internal",label:"Internal",onPress:()=>switchSource("internal")},
                ...roots.filter(root=>root.removable && !root.network).map(root=>({kind:"action" as const,id:`usb-${root.id}`,label:`USB: ${root.label}`,onPress:()=>switchSource("usb",root.id)})),
                ...(!roots.some(root=>root.removable && !root.network) ? [{kind:"action" as const,id:"usb-empty",label:"USB (No drives connected)",disabled:true,onPress:()=>{}}] : []),
                {kind:"action",id:"network",label:"Network",onPress:()=>switchSource("network")}]}}]}]}
        accept={{id:"save",label:acceptLabel,variant:"primary",disabled:busy || !root?.writable || !typedName,onPress:()=>void save()}}>
        <div className="show-save-body">
            <div className="show-save-location-section">
            <Button className="show-save-location" title={locationLabel} aria-expanded={locationOpen} aria-controls="show-save-folders" disabled={saving} onClick={()=>setLocationOpen(!locationOpen)}>
                <span>{locationLabel}</span>
                <span aria-hidden="true">{locationOpen ? "▴" : "▾"}</span>
            </Button>
            {locationOpen && <SaveDestinationPanel panel={{source, peer, peers, availableRoots, root, rootId, path, folders, shows, existingShow, reading, busy, saving, error, folderDraft, browsingDestinations, canCreateFolder,
                navigate, setRootId, setPeer, setFolderDraft, createFolder:()=>void createFolder(), chooseShow:(show:FileEntry)=>{dialogs.setShowName(showStem(show.name));setStatus("");}, newFolder:()=>{setFolderDraft("");setStatus("");}}} />}
            </div>
            {error && !locationOpen && <ErrorAlert as="p" className="show-browser-error" role="alert">{error}</ErrorAlert>}{status && <p className="show-save-message" role="status">{status}</p>}
            {exported && <MvrExportSummary report={exported} onDismiss={() => setExported(null)} />}
            <div className="show-save-fields">
            <TextInput clearable className="show-name-input" autoFocus value={dialogs.showName} onChange={event=>dialogs.setShowName(event.target.value)} onKeyboardCommit={()=>void save()} placeholder="New show name" aria-label="Show name" disabled={saving} />
            {existingShow && <p className="show-save-replace-note" role="note">{`${existingShow.name} already exists here. Saving asks before replacing it.`}</p>}
            <div className="show-row-actions">
                <SwitchField bare className="show-save-template" label="Save as Template" checked={dialogs.baseShow} disabled={saving} onChange={event=>dialogs.setBaseShow(event.target.checked)} />
            </div>
            </div>
        </div>
    </ModalFrame>
    {replaceTarget && <ReplaceShowConfirmation name={replaceTarget.name} location={`${peer?.name ?? root?.label ?? ""} / ${path || ""}`} saving={saving} onCancel={()=>setReplaceTarget(null)} onReplace={()=>void save(true)} />}
    </>;
}

interface DestinationPanel {
    source: Source; peer: NetworkShowPeer | null; peers: NetworkShowPeer[]; availableRoots: FileRoot[]; root: FileRoot | undefined;
    rootId: string; path: string; folders: FileEntry[]; shows: FileEntry[]; existingShow: FileEntry | undefined;
    reading: boolean; busy: boolean; saving: boolean; error: string; folderDraft: string | null; browsingDestinations: boolean; canCreateFolder: boolean;
    navigate: (path: string) => void; setRootId: (id: string) => void; setPeer: (peer: NetworkShowPeer | null) => void; setFolderDraft: (value: string | null) => void;
    createFolder: () => void; chooseShow: (show: FileEntry) => void; newFolder: () => void;
}

/** The expanded Location: where the show will be saved, what is already there, and New Folder. */
function SaveDestinationPanel({panel}: {panel: DestinationPanel}) {
    const {source, peer, peers, availableRoots, root, rootId, path, folders, shows, existingShow, reading, busy, saving, error, folderDraft, browsingDestinations, canCreateFolder,
        navigate, setRootId, setPeer, setFolderDraft, createFolder, chooseShow, newFolder} = panel;
    return <div id="show-save-folders">
                {((source === "network" && availableRoots.length > 1) || (source === "network" && (rootId || peer))) && <div className="show-save-folder-toolbar">
                    {availableRoots.length > 1 && <SelectField label={peer ? "Desk folder root" : "Network drive"} value={rootId} options={availableRoots.map(item=>({value:item.id,label:item.label}))} disabled={busy} onChange={value=>{setRootId(value);navigate("");}} />}
                    {source === "network" && (rootId || peer) && <Button disabled={busy} onClick={()=>{setPeer(null);setRootId("");navigate("");}}>Network destinations</Button>}
                </div>}
                {root && !browsingDestinations && <div className="show-save-folder-toolbar show-save-navigation">
                    <ShowLocationBar rootLabel={peer ? `${peer.name} · ${root.label}` : root.label} rootKind={source === "usb" ? "location-drive" : source === "network" ? "location" : "location-shows"} path={path} busy={busy} onNavigate={navigate} />
                    {canCreateFolder && folderDraft === null && <Button className="show-save-new-folder" disabled={busy} onClick={newFolder}>New Folder</Button>}
                </div>}
                {folderDraft !== null && <div className="show-save-folder-toolbar show-save-new-folder-form">
                    <TextInput autoFocus value={folderDraft} onChange={event=>setFolderDraft(event.target.value)} onKeyboardCommit={createFolder} placeholder="Folder name" aria-label="New folder name" disabled={saving} />
                    <Button variant="primary" disabled={saving || !validItemName(folderDraft)} onClick={createFolder}>Create Folder</Button>
                    <Button disabled={saving} onClick={()=>setFolderDraft(null)}>Cancel</Button>
                </div>}
                {reading && <p className="show-save-message" role="status">Reading folders…</p>}
                {error ? <ErrorAlert as="p" className="show-browser-error" role="alert">{error}</ErrorAlert> : !reading && <WindowScrollArea className="show-save-folder-scroll"><table className="show-browser-table"><thead><tr><th>{browsingDestinations ? "Destination" : "Name"}</th><th>{browsingDestinations ? "Location" : "Modified"}</th></tr></thead><tbody>
                    {browsingDestinations ? <>
                        {availableRoots.map(item=><tr key={item.id}><td><Button disabled={busy} onClick={()=>{setRootId(item.id);navigate("");}}><ShowEntryName kind="location" name={item.label} /></Button></td><td>Mounted network drive</td></tr>)}
                        {peers.map(item=><tr key={item.instance}><td><Button disabled={busy || item.role !== "desk" || !!item.error} onClick={()=>{setPeer(item);setRootId("");navigate("");}}><ShowEntryName kind="location" name={item.name} /></Button></td><td>{item.role === "desk" ? "Control desk" : "Architect · Load its show through Load Show"}{item.error ? ` · ${item.error}` : ""}</td></tr>)}
                    </> : <>
                        {folders.map(folder=><tr key={folder.path}><td><Button disabled={busy} onClick={()=>navigate(folder.path)}><ShowEntryName kind="folder" name={folder.name} /></Button></td><td>{dateLabel(folder.modified_millis)}</td></tr>)}
                        {shows.map(show=><tr key={show.path} className={show === existingShow ? "is-replace-target" : undefined}><td><Button aria-label={`Use name ${showStem(show.name)}`} disabled={saving} onClick={()=>chooseShow(show)}><ShowEntryName kind="show" name={show.name} detail={show === existingShow ? "Saving replaces this show" : undefined} /></Button></td><td>{dateLabel(show.modified_millis)}</td></tr>)}
                        {root && !folders.length && !shows.length && <tr><td colSpan={2} className="show-save-empty">This folder is empty.</td></tr>}
                    </>}
                </tbody></table></WindowScrollArea>}
                {!reading && peer && !availableRoots.length && <p className="show-save-message">This desk has no writable folder destinations.</p>}
                {!reading && source === "usb" && !root && <p className="show-save-message">No writable USB drive connected.</p>}
                {!reading && browsingDestinations && !availableRoots.length && !peers.length && <p className="show-save-message">No writable network drives or announced Control desks available.</p>}
            </div>;
}

function ReplaceShowConfirmation({name, location, saving, onCancel, onReplace}: {name: string; location: string; saving: boolean; onCancel: () => void; onReplace: () => void}) {
    return <ModalFrame title={`Replace ${name}?`} ariaLabel="Confirm replace show" dialogClassName="nested-modal show-replace-modal"
        closeLabel="Keep existing show" closeDisabled={saving} policy={{escape:!saving,backdrop:!saving}} onClose={onCancel}>
        <p>{`${name} in ${location} will be replaced by the current show. A show this desk knows keeps its name and gets a backup copy first.`}</p>
        <footer><Button disabled={saving} onClick={onCancel}>Cancel</Button><Button variant="primary" disabled={saving} onClick={onReplace}>Replace Show</Button></footer>
    </ModalFrame>;
}

/** Reads the folder the Save browser shows, caching each folder until a save or New Folder changes it. */
function useDestinationListing({files, lifecycle, source, peer, rootId, path, locationOpen, setRootId, setError}: {
    files: ReturnType<typeof useFiles>; lifecycle: QuickSetupModel["authorities"]["lifecycle"]; source: Source; peer: NetworkShowPeer | null;
    rootId: string; path: string; locationOpen: boolean; setRootId: (id: string) => void; setError: (message: string) => void;
}) {
    const [remoteRoots, setRemoteRoots] = useState<FileRoot[]>([]);
    const [peers, setPeers] = useState<NetworkShowPeer[]>([]);
    const [entries, setEntries] = useState<FileEntry[]>([]);
    const [refresh, setRefresh] = useState(0);
    const [reading, setReading] = useState(false);
    const directoryCache = useRef(new Map<string, {roots?: FileRoot[]; entries: FileEntry[]; root_id?: string | null}>());
    useEffect(() => {
        let current = true;
        // Folders are read only once the operator opens the location; a save that meets an
        // existing show without a listing still asks before replacing it.
        if (!locationOpen) { setReading(false); return; }
        setReading(true); setError(""); setEntries([]);
        const read = async () => {
            if (source === "network" && !rootId && !peer) {
                const catalog = await lifecycle!.networkShows();
                if(current) setPeers(catalog.peers);
            } else if (peer) {
                const key = `${peer.instance}:${rootId}:${path}`;
                const directory = directoryCache.current.get(key) ?? await lifecycle!.networkSaveFolders(peer.instance, rootId, path);
                directoryCache.current.set(key, directory);
                if (!current) return;
                setRemoteRoots((directory.roots ?? []).filter(item => item.writable));
                if (!rootId && directory.root_id) setRootId(directory.root_id);
                setEntries(directory.entries);
            } else if (rootId) {
                const key = `local:${rootId}:${path}`;
                const directory = directoryCache.current.get(key) ?? await files.fileEntries(rootId, path);
                directoryCache.current.set(key, directory);
                if(current) setEntries([...directory.entries].sort((a,b) => a.name.localeCompare(b.name)));
            }
        };
        void read().catch(reason => {if(current) setError(formatErrorDetails(reason));}).finally(() => {if(current) setReading(false);});
        return () => {current = false;};
    }, [source, peer, rootId, path, files, lifecycle, locationOpen, refresh]);

    function reread() {
        directoryCache.current.delete(peer ? `${peer.instance}:${rootId}:${path}` : `local:${rootId}:${path}`);
        setRefresh(value => value + 1);
    }
    return {entries, peers, remoteRoots, reading, reread};
}
