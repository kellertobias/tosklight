import { Button, ModalFrame, SelectField } from "@tosklight/ui";
import { useEffect, useState } from "react";
import type { FileEntry, FileRoot, ShowEntry, ShowRevision } from "../../api/types";
import type { NetworkShowPeer, NetworkShow } from "../../api/generated/light-wire";
import { useFiles } from "../../features/files/FilesContext";
import type { QuickSetupModel } from "./QuickSetupModal";

type Source = "internal" | "usb" | "network";
interface Row {
    key: string; name: string; updated: string | null;
    local?: ShowEntry; file?: FileEntry; peer?: NetworkShowPeer; remote?: NetworkShow;
}
const dateLabel = (value: string | null) => {
    const date = value ? new Date(value) : null;
    return date && !Number.isNaN(date.getTime()) ? date.toLocaleString() : "Date unavailable";
};

export function ShowLoadBrowser({model}: {model: QuickSetupModel}) {
    const files = useFiles();
    const lifecycle = model.authorities.lifecycle;
    const [source, setSource] = useState<Source>("internal");
    const [roots, setRoots] = useState<FileRoot[]>([]);
    const [rootId, setRootId] = useState("shows");
    const [path, setPath] = useState("");
    const [entries, setEntries] = useState<FileEntry[]>([]);
    const [peers, setPeers] = useState<NetworkShowPeer[]>([]);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState("");
    const [selected, setSelected] = useState<Row | null>(null);
    const [revisions, setRevisions] = useState<ShowRevision[]>([]);
    useEffect(() => { let current = true; void files.fileRoots().then(found => current && setRoots(found)).catch(reason => current && setError(String(reason))); return () => {current = false;}; }, [files]);
    useEffect(() => {
        let current = true; setError(""); setEntries([]); setPeers([]); setBusy(true);
        const task = source === "network"
            ? lifecycle?.networkShows().then(catalog => {if(current) setPeers(catalog.peers);})
            : files.fileEntries(rootId, path).then(directory => {if(current) setEntries(directory.entries);});
        void task?.catch(reason => current && setError(reason instanceof Error ? reason.message : String(reason))).finally(() => current && setBusy(false));
        return () => {current = false;};
    }, [source, rootId, path, files, lifecycle]);
    const root = roots.find(root => root.id === rootId);
    const folders = entries.filter(entry => entry.kind === "folder").sort((a,b) => a.name.localeCompare(b.name));
    const rows: Row[] = source === "network" ? peers.flatMap(peer => peer.shows.map(show => ({key:`${peer.instance}:${show.id ?? "current"}`,name:show.name,updated:show.updated_at,peer,remote:show})))
        : entries.filter(entry => entry.kind === "file" && /\.show$/i.test(entry.name)).map(file => {
            const local = source === "internal" ? lifecycle?.shows.find(show => (show.path.replaceAll("\\", "/").endsWith(`/${file.path}`) || show.path === file.path)) : undefined;
            return {key:file.path,name:local?.name ?? file.name.replace(/\.show$/i,""),updated:local?.updated_at ?? (file.modified_millis === null ? null : new Date(file.modified_millis).toISOString()),local,file};
        }).sort((a,b) => a.name.localeCompare(b.name));
    async function run(task: () => Promise<void>) {
        setBusy(true); setError("");
        try {await task();} catch(reason) {setError(reason instanceof Error ? reason.message : String(reason));} finally {setBusy(false);}
    }
    function switchSource(next: Source) {
        setSource(next); setPath(""); setSelected(null);
        setRootId(next === "usb" ? roots.find(root => root.removable)?.id ?? "" : "shows");
    }
    async function revisionsFor(row: Row) {
        setSelected(row); setRevisions([]);
        await run(async () => {
            if (row.local) setRevisions(await lifecycle!.listShowRevisions(row.local.id));
            else if (row.remote) setRevisions(row.remote.revisions);
        });
    }
    async function perform(row: Row, named: number | null, partial: boolean) {
        await run(async () => {
            if (!lifecycle) throw new Error("Desk is disconnected");
            if (partial) {
                const prepared = row.local ? named === null ? row.local : await lifecycle.prepareShowRevision(row.local.id, named)
                    : row.peer ? await lifecycle.importRemoteShow(row.peer.instance, row.remote?.id ?? null, named, false)
                    : await lifecycle.prepareShowFile(rootId, row.file!.path, row.file!.name);
                if (!prepared) throw new Error("Could not prepare the selected source");
                model.dialogs.setPartialSource(prepared); model.dialogs.setLoadOpen(false); model.dialogs.setSelectiveImportOpen(true);
            } else {
                const loaded = row.local ? named === null ? await lifecycle.openShow(row.local.id) : await lifecycle.openShowRevision(row.local.id,named)
                    : row.peer ? await lifecycle.importRemoteShow(row.peer.instance,row.remote?.id ?? null,named,true)
                    : await lifecycle.openShowFile(rootId,row.file!.path,row.file!.name);
                if (!loaded) throw new Error("The selected show could not be loaded; check the desk error message");
                model.dialogs.setLoadOpen(false);
            }
        });
    }
    return <ModalFrame title="Load Show" ariaLabel="Load show" dialogClassName="nested-modal load-show-modal"
        closeLabel="Close Load Show" closeDisabled={busy} onClose={() => !busy && model.dialogs.setLoadOpen(false)}
        groups={[{id:"show-source", actions:[{id:"source", kind:"dropdown", label:"Source", disabled:busy,
            dropdown:{kind:"items", ariaLabel:"Show source", items:([
                ["internal","Internal"],["usb","USB"],["network","Network"]
            ] as const).map(([id,label]) => ({kind:"action" as const,id,label,disabled:busy,onPress:() => switchSource(id)}))}
        }]}]}>
            <div className="show-browser-toolbar">
                {source === "usb" && <SelectField label="USB drive" value={rootId} options={roots.filter(root => root.removable).map(root => ({value:root.id,label:root.label}))} onChange={value => {setRootId(value);setPath("");}} />}
                {source !== "network" && path && <Button disabled={busy} onClick={() => setPath(path.split("/").slice(0,-1).join("/"))}>Up one folder</Button>}
            </div>
            {source !== "network" && <p className="show-browser-path">{root?.label ?? (source === "usb" ? "No USB drive connected" : "Internal")} / {path}</p>}
            {busy && <p role="status">Loading…</p>}
            {error && <p role="alert">{error}</p>}
            {source === "network" && peers.map(peer => <p key={peer.instance} className={peer.error ? "modal-warning" : "show-peer"}>{peer.name} · {peer.address}{peer.error ? ` · ${peer.error}` : peer.shows.length === 0 ? " · No shows available" : ""}</p>)}
            {!busy && rows.length === 0 && folders.length === 0 && <p>No shows available in this source.</p>}
            <div className="show-browser-table-scroll"><table className="show-browser-table"><thead><tr><th>Show / folder</th><th>Last saved</th><th>Actions</th></tr></thead><tbody>
                {folders.map(folder => <tr key={folder.path}><td colSpan={3}><Button disabled={busy} onClick={() => setPath(folder.path)}>📁 {folder.name}</Button></td></tr>)}
                {rows.map(row => <tr key={row.key}><td><strong>{row.name}</strong>{row.peer && <small>{row.peer.name}</small>}</td><td>{dateLabel(row.updated)}</td><td><div className="show-row-actions"><Button disabled={busy} onClick={() => void perform(row,null,false)}>Load Latest</Button><Button aria-label={`Revisions for ${row.name}`} disabled={busy} onClick={() => void revisionsFor(row)}>…</Button></div></td></tr>)}
            </tbody></table></div>
            {selected && <ModalFrame title={selected.name} ariaLabel={`Revisions for ${selected.name}`} dialogClassName="nested-modal show-revisions-modal"
                closeDisabled={busy} closeLabel="Close revisions" onClose={() => !busy && setSelected(null)}>
                <p>Named revisions load as independent copies. Partial Load previews dependencies and conflicts before changing the current show.</p>
                <div className="show-browser-table-scroll"><table className="show-browser-table"><thead><tr><th>Revision</th><th>Last saved</th><th>Actions</th></tr></thead><tbody>
                    {[{revision:null,name:"Latest Autosave",created_at:selected.updated},...revisions.map(item => ({revision:item.revision,name:`Revision ${item.revision} · ${item.name}`,created_at:item.created_at}))].map(item => <tr key={item.revision ?? "latest"}>
                        <td><strong>{item.name}</strong></td><td>{dateLabel(item.created_at)}</td><td><div className="show-row-actions">
                            <Button disabled={busy} onClick={() => void perform(selected,item.revision,false)}>Load</Button>
                            <Button disabled={busy} onClick={() => void perform(selected,item.revision,true)}>Partial Load</Button>
                        </div></td>
                    </tr>)}
                </tbody></table></div>
                {busy && <p role="status">Preparing show…</p>}{error && <p role="alert">{error}</p>}
            </ModalFrame>}
    </ModalFrame>;
}
