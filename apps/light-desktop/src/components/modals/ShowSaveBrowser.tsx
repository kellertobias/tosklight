import { formatErrorDetails } from "@tosklight/ui";
import { ErrorAlert } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { Button, ModalFrame, SelectField, SwitchField, TextInput } from "@tosklight/ui";
import { useEffect, useRef, useState } from "react";
import type { FileEntry, FileRoot, NetworkShowPeer } from "../../api/types";
import { useFiles } from "../../features/files/FilesContext";
import type { QuickSetupModel } from "./QuickSetupModal";

type Source = "internal" | "usb" | "network";
const sources = [{id:"internal",label:"Internal"},{id:"usb",label:"USB"},{id:"network",label:"Network"}] as const;

/** Save destinations are confined server folders, including mounted shares and discovered desks. */
export function ShowSaveBrowser({model}: {model: QuickSetupModel}) {
    const files = useFiles();
    const lifecycle = model.authorities.lifecycle;
    const dialogs = model.dialogs;
    const [source, setSource] = useState<Source>("internal");
    const [roots, setRoots] = useState<FileRoot[]>([]);
    const [remoteRoots, setRemoteRoots] = useState<FileRoot[]>([]);
    const [peers, setPeers] = useState<NetworkShowPeer[]>([]);
    const [peer, setPeer] = useState<NetworkShowPeer | null>(null);
    const [rootId, setRootId] = useState("shows");
    const [path, setPath] = useState("");
    const [folders, setFolders] = useState<FileEntry[]>([]);
    const [reading, setReading] = useState(false);
    const [saving, setSaving] = useState(false);
    const [locationOpen, setLocationOpen] = useState(false);
    const [error, setError] = useState("");
    const [status, setStatus] = useState("");
    const busy = (locationOpen && reading) || saving;
    const directoryCache = useRef(new Map<string, {roots?: FileRoot[]; entries: FileEntry[]; root_id?: string | null}>());
    const availableRoots = peer ? remoteRoots : roots.filter(root =>
        (source === "internal" ? root.id === "shows" : source === "usb" ? root.removable && !root.network : root.network));
    const root = availableRoots.find(item => item.id === rootId);
    const localDefault = source === "internal" && rootId === "shows" && !path;
    const target = {rootId, path, ...(peer ? {instance:peer.instance} : {})};
    const close = () => {if (!saving) dialogs.setSaveAsOpen(false);};

    useEffect(() => {
        let current = true;
        void files.fileRoots().then(found => {if(current) setRoots(found);}).catch(reason => {if(current) setError(String(reason));});
        return () => {current = false;};
    }, [files]);
    useEffect(() => {
        let current = true;
        if (!locationOpen) { setReading(false); return; }
        setReading(true); setError(""); setFolders([]);
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
                setFolders(directory.entries.filter(item => item.kind === "folder"));
            } else if (rootId) {
                const key = `local:${rootId}:${path}`;
                const directory = directoryCache.current.get(key) ?? await files.fileEntries(rootId, path);
                directoryCache.current.set(key, directory);
                if(current) setFolders(directory.entries.filter(item => item.kind === "folder").sort((a,b) => a.name.localeCompare(b.name)));
            }
        };
        void read().catch(reason => {if(current) setError(formatErrorDetails(reason));}).finally(() => {if(current) setReading(false);});
        return () => {current = false;};
    }, [source, peer, rootId, path, files, lifecycle, locationOpen]);

    function switchSource(next: Source, driveId?: string) {
        setSource(next); setPeer(null); setPath(""); setStatus(""); setLocationOpen(true);
        setRootId(next === "internal" ? "shows" : next === "usb" ? driveId ?? roots.find(item => item.removable && !item.network)?.id ?? "" : "");
    }
    async function save() {
        const name = dialogs.showName.trim();
        if (!name || busy || !root?.writable) return;
        setSaving(true); setError(""); setStatus("Saving show…");
        try {
            if (localDefault) {
                if (!(await model.actions.saveAs(name))) throw new Error("Could not save this show. Check the name and desk connection, then try again.");
            }
            else {
                const saved = await lifecycle!.saveShowCopy(name, target, dialogs.baseShow);
                setStatus(`Saved ${saved.name} to ${peer?.name ?? root.label} / ${path || "/"}`);
            }
        } catch(reason) {setError(formatErrorDetails(reason)); setStatus("");}
        finally {setSaving(false);}
    }
    useEffect(() => {
        dialogs.saveDestinationSubmit.current = {save, busy:saving};
        return () => {dialogs.saveDestinationSubmit.current = null;};
    });
    async function exportMvr() {
        if (!root?.writable || busy) return;
        setSaving(true); setError(""); setStatus("Exporting MVR…");
        try {
            const saved = await lifecycle!.exportMvrFile(dialogs.showName.trim() || model.view.activeShow?.name || "Show", target);
            setStatus(`Exported MVR to ${peer?.name ?? root.label} / ${saved.path}`);
        } catch(reason) {setError(formatErrorDetails(reason));setStatus("");}
        finally {setSaving(false);}
    }
    const sourceLabel = source === "usb" ? `USB: ${root?.label ?? "No drive connected"}` : sources.find(item=>item.id===source)?.label;
    const locationLabel = `Location: ${peer ? `${peer.name} · ` : ""}${root?.label ?? "Choose a destination"}${root ? ` / ${path || ""}` : ""}`;
    const title = model.view.activeShowIsProvisional && localDefault ? "Name Empty Show" : "Save Show As";
    return <ModalFrame title={title} ariaLabel="Save show" dialogClassName="nested-modal save-show-modal"
        closeLabel="Close Save Show" closeDisabled={saving} policy={{escape:!saving,backdrop:!saving}} onClose={close}
        groups={[{id:"export",actions:[{id:"export-mvr",label:"Export MVR",disabled:busy || !root?.writable || !model.view.activeShow,onPress:()=>void exportMvr()}]},{id:"save-source",actions:[{id:"source",kind:"dropdown",ariaLabel:`Source: ${sourceLabel}`,label:<>Source: {sourceLabel} <span aria-hidden="true">⌄</span></>,disabled:busy,
            dropdown:{kind:"items",ariaLabel:"Show source",items:[{kind:"action",id:"internal",label:"Internal",onPress:()=>switchSource("internal")},
                ...roots.filter(root=>root.removable && !root.network).map(root=>({kind:"action" as const,id:`usb-${root.id}`,label:`USB: ${root.label}`,onPress:()=>switchSource("usb",root.id)})),
                ...(!roots.some(root=>root.removable && !root.network) ? [{kind:"action" as const,id:"usb-empty",label:"USB (No drives connected)",disabled:true,onPress:()=>{}}] : []),
                {kind:"action",id:"network",label:"Network",onPress:()=>switchSource("network")}]}}]}]}
        accept={{id:"save",label:model.view.activeShowIsProvisional && localDefault ? "Name Empty Show" : "Save as New Show",variant:"primary",disabled:busy || !root?.writable || !dialogs.showName.trim(),onPress:()=>void save()}}>
        <div className="show-save-body">
            <div className="show-save-location-section">
            <Button className="show-save-location" title={locationLabel} aria-expanded={locationOpen} aria-controls="show-save-folders" disabled={saving} onClick={()=>setLocationOpen(!locationOpen)}>
                <span>{locationLabel}</span>
                <span aria-hidden="true">{locationOpen ? "▴" : "▾"}</span>
            </Button>
            {locationOpen && <div id="show-save-folders">
                {((source === "network" && availableRoots.length > 1) || (source === "network" && (rootId || peer))) && <div className="show-save-folder-toolbar">
                    {availableRoots.length > 1 && <SelectField label={peer ? "Desk folder root" : "Network drive"} value={rootId} options={availableRoots.map(item=>({value:item.id,label:item.label}))} disabled={busy} onChange={value=>{setRootId(value);setPath("");}} />}
                    {source === "network" && (rootId || peer) && <Button disabled={busy} onClick={()=>{setPeer(null);setRootId("");setPath("");}}>Network destinations</Button>}
                </div>}
                {reading && <p className="show-save-message" role="status">Reading folders…</p>}
                {error ? <ErrorAlert as="p" className="show-browser-error" role="alert">{error}</ErrorAlert> : !reading && <WindowScrollArea className="show-save-folder-scroll"><table className="show-browser-table"><thead><tr><th>Folder / destination</th><th>Location</th></tr></thead><tbody>
                    {source === "network" && !rootId && !peer ? <>
                        {availableRoots.map(item=><tr key={item.id}><td><Button disabled={busy} onClick={()=>{setRootId(item.id);setPath("");}}>{item.label}</Button></td><td>Mounted network drive</td></tr>)}
                        {peers.map(item=><tr key={item.instance}><td><Button disabled={busy || item.role !== "desk" || !!item.error} onClick={()=>{setPeer(item);setRootId("");setPath("");}}>{item.name}</Button></td><td>{item.role === "desk" ? "Control desk" : "Architect · Load its show through Load Show"}{item.error ? ` · ${item.error}` : ""}</td></tr>)}
                    </> : <>
                        {path && <tr><td><Button disabled={busy} onClick={()=>setPath(path.split("/").slice(0,-1).join("/"))}>↑ Up one folder</Button></td><td>Parent folder</td></tr>}
                        {root && <tr><td>Current folder</td><td>{root.label} / {path || ""}</td></tr>}
                        {folders.map(folder=><tr key={folder.path}><td><Button disabled={busy} onClick={()=>setPath(folder.path)}>📁 {folder.name}</Button></td><td>{folder.path}</td></tr>)}
                    </>}
                </tbody></table></WindowScrollArea>}
                {!reading && peer && !availableRoots.length && <p className="show-save-message">This desk has no writable folder destinations.</p>}
                {!reading && source === "usb" && !root && <p className="show-save-message">No writable USB drive connected.</p>}
                {!reading && source === "network" && !rootId && !peer && !availableRoots.length && !peers.length && <p className="show-save-message">No writable network drives or announced Control desks available.</p>}
            </div>}
            </div>
            {error && !locationOpen && <ErrorAlert as="p" className="show-browser-error" role="alert">{error}</ErrorAlert>}{status && <p className="show-save-message" role="status">{status}</p>}
            <div className="show-save-fields">
            <TextInput clearable className="show-name-input" autoFocus value={dialogs.showName} onChange={event=>dialogs.setShowName(event.target.value)} onKeyboardCommit={()=>void save()} placeholder="New show name" aria-label="Show name" disabled={saving} />
            <div className="show-row-actions">
                <SwitchField bare className="show-save-template" label="Save as Template" checked={dialogs.baseShow} disabled={saving} onChange={event=>dialogs.setBaseShow(event.target.checked)} />
            </div>
            </div>
        </div>
    </ModalFrame>;
}
