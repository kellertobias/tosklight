import { Button, ModalFrame, SelectField, TextInput } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { FileEntry, FileRoot } from "../../api/types";
import type { NetworkShowPeer } from "../../api/generated/light-wire";
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
    const [reading, setReading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [error, setError] = useState("");
    const [status, setStatus] = useState("");
    const busy = reading || saving;
    const availableRoots = peer ? remoteRoots : roots.filter(root => root.writable &&
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
        setReading(true); setError(""); setFolders([]);
        const read = async () => {
            if (source === "network" && !rootId && !peer) {
                const catalog = await lifecycle!.networkShows();
                if(current) setPeers(catalog.peers.filter(item => item.role === "desk"));
            } else if (peer) {
                const directory = await lifecycle!.networkSaveFolders(peer.instance, rootId, path);
                if (!current) return;
                setRemoteRoots(directory.roots.filter(item => item.writable));
                if (!rootId && directory.root_id) setRootId(directory.root_id);
                setFolders(directory.entries.filter(item => item.kind === "folder"));
            } else if (rootId) {
                const directory = await files.fileEntries(rootId, path);
                if(current) setFolders(directory.entries.filter(item => item.kind === "folder").sort((a,b) => a.name.localeCompare(b.name)));
            }
        };
        void read().catch(reason => {if(current) setError(reason instanceof Error ? reason.message : String(reason));}).finally(() => {if(current) setReading(false);});
        return () => {current = false;};
    }, [source, peer, rootId, path, files, lifecycle]);

    function switchSource(next: Source) {
        setSource(next); setPeer(null); setPath(""); setStatus("");
        setRootId(next === "internal" ? "shows" : next === "usb" ? roots.find(item => item.removable && item.writable && !item.network)?.id ?? "" : "");
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
        } catch(reason) {setError(reason instanceof Error ? reason.message : String(reason)); setStatus("");}
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
        } catch(reason) {setError(reason instanceof Error ? reason.message : String(reason));setStatus("");}
        finally {setSaving(false);}
    }
    const title = model.view.activeShowIsProvisional && localDefault ? "Name Empty Show" : "Save Show As";
    return <ModalFrame title={title} ariaLabel="Save show" dialogClassName="nested-modal save-show-modal"
        closeLabel="Close Save Show" closeDisabled={saving} policy={{escape:!saving,backdrop:!saving}} onClose={close}
        groups={[{id:"save-source",actions:[{id:"source",kind:"dropdown",label:"Source",disabled:busy,
            dropdown:{kind:"items",ariaLabel:"Show source",items:sources.map(item => ({kind:"action",id:item.id,label:item.label,onPress:()=>switchSource(item.id)}))}}]}]}
        accept={{id:"save",label:model.view.activeShowIsProvisional && localDefault ? "Name Empty Show" : "Save as New Show",variant:"primary",disabled:busy || !root?.writable || !dialogs.showName.trim(),onPress:()=>void save()}}>
        <div className="show-save-body">
            <div className="show-browser-toolbar">
                {availableRoots.length > 1 && <SelectField label={peer ? "Desk folder root" : source === "usb" ? "USB drive" : "Network drive"} value={rootId} options={availableRoots.map(item=>({value:item.id,label:item.label}))} disabled={busy} onChange={value=>{setRootId(value);setPath("");}} />}
                {source === "network" && (rootId || peer) && <Button disabled={busy} onClick={()=>{setPeer(null);setRootId("");setPath("");}}>Network destinations</Button>}
                {path && <Button disabled={busy} onClick={()=>setPath(path.split("/").slice(0,-1).join("/"))}>Up one folder</Button>}
            </div>
            <p>{sources.find(item=>item.id===source)?.label} · {peer?.name ?? root?.label ?? "Choose a destination"} / {path}</p>
            {error && <p role="alert">{error}</p>}{status && <p role="status">{status}</p>}{reading && <p role="status">Reading folders…</p>}
            <div className="show-browser-table-scroll"><table className="show-browser-table"><thead><tr><th>Folder / destination</th><th>Location</th></tr></thead><tbody>
                {source === "network" && !rootId && !peer ? <>
                    {availableRoots.map(item=><tr key={item.id}><td><Button disabled={busy} onClick={()=>{setRootId(item.id);setPath("");}}>{item.label}</Button></td><td>Mounted network drive</td></tr>)}
                    {peers.map(item=><tr key={item.instance}><td><Button disabled={busy} onClick={()=>{setPeer(item);setRootId("");setPath("");}}>{item.name}</Button></td><td>Control desk{item.error ? ` · ${item.error}` : ""}</td></tr>)}
                </> : <>
                    {root && <tr><td>Save in this folder</td><td>{root.label} / {path || "/"}</td></tr>}
                    {folders.map(folder=><tr key={folder.path}><td><Button disabled={busy} onClick={()=>setPath(folder.path)}>📁 {folder.name}</Button></td><td>{folder.path}</td></tr>)}
                </>}
            </tbody></table></div>
            {!reading && peer && !availableRoots.length && <p>This desk has no writable folder destinations.</p>}
            {!reading && source === "usb" && !root && <p>No writable USB drive connected.</p>}
            {!reading && source === "network" && !rootId && !peer && !availableRoots.length && !peers.length && <p>No writable network drives or announced Control desks available.</p>}
            <TextInput clearable className="show-name-input" autoFocus value={dialogs.showName} onChange={event=>dialogs.setShowName(event.target.value)} onKeyboardCommit={()=>void save()} placeholder="New show name" aria-label="Show name" disabled={saving} />
            <div className="show-row-actions">
                <Button aria-label="Save as a base show" aria-pressed={dialogs.baseShow} active={dialogs.baseShow} disabled={saving} onClick={()=>dialogs.setBaseShow(!dialogs.baseShow)}><span aria-hidden="true">{dialogs.baseShow ? "◉" : "○"}</span> Save as a base show</Button>
                <Button disabled={busy || !root?.writable || !model.view.activeShow} onClick={()=>void exportMvr()}>Export MVR</Button>
            </div>
            {localDefault && model.view.activeShow && !model.view.activeShowIsProvisional && <section className="current-autosave"><h4>Current show's Latest Autosave</h4><p><b>{model.view.activeShow.name}</b> · {new Date(model.view.activeShow.updated_at).toLocaleString()}</p><Button disabled={busy} onClick={()=>void model.actions.saveAs(model.view.activeShow!.name,true)}>Save to Latest Autosave</Button></section>}
        </div>
    </ModalFrame>;
}
