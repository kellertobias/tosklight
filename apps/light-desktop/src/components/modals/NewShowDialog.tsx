import { Button, ModalFrame, TextInput, WindowScrollArea } from "@tosklight/ui";
import { useState } from "react";
import type { ShowEntry } from "../../api/types";
import type { QuickSetupModel } from "./QuickSetupModal";
import "./newShowDialog.css";

function savedTime(value: string) {
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? "Unknown" : date.toLocaleString();
}

export function NewShowDialog({ model }: { model: QuickSetupModel }) {
    const { lifecycle } = model.authorities;
    const { newShowOpen, setNewShowOpen } = model.dialogs;
    const [busy, setBusy] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [editing, setEditing] = useState<ShowEntry | null>(null);
    const [description, setDescription] = useState("");
    if (!newShowOpen) return null;
    const close = () => { if (!busy) setNewShowOpen(false); };
    const create = async (label: string, task: () => Promise<boolean> | undefined) => {
        if (busy) return;
        setBusy(label); setError(null);
        try {
            if (await task()) setNewShowOpen(false);
            else setError("The show could not be opened. Check the desk error and try again.");
        } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
        finally { setBusy(null); }
    };
    const saveDescription = async () => {
        if (!editing || busy || !lifecycle) return;
        setBusy("Saving description…"); setError(null);
        try { await lifecycle.setShowDescription(editing.id, description); setEditing(null); }
        catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
        finally { setBusy(null); }
    };
    const bases = (lifecycle?.shows ?? []).filter(show => show.is_base_show);
    return <>
        <ModalFrame title="New Show" ariaLabel="New show" dialogClassName="nested-modal new-show-modal" closeDisabled={Boolean(busy)} policy={{escape:!busy,backdrop:!busy}} onClose={close}
                groups={[{ id: "new-show-source", actions: [{ id: "mvr", label: "Load from MVR", disabled: Boolean(busy), onPress: () => model.mvr.openMvrImport(() => setNewShowOpen(false)) }] }]} >
            <p>Open a clean default, create an empty show, or start from a saved base show. The current show remains saved on this desk.</p>
            <div className="new-show-first-row">
                <Button disabled={Boolean(busy)} onClick={() => void create("Loading clean default…", () => lifecycle?.openCleanDefaultShow())}>Load Clean Built-in Default</Button>
                <Button variant="primary" disabled={Boolean(busy)} onClick={() => void create("Creating empty show…", () => lifecycle?.initializeEmptyShow())}>Create Empty Show</Button>
            </div>
            <section className="new-show-base-library">
                <h4>Start from a base show</h4>
                {bases.length ? <WindowScrollArea className="new-show-base-scroll"><table className="new-show-base-table" aria-label="Base shows">
                    <thead><tr><th>Show name</th><th>Description</th><th>Saved</th><th aria-label="Actions" /></tr></thead>
                    <tbody>{bases.map(show => <tr key={show.id}>
                        <td>{show.name}</td>
                        <td><p>{show.description || "No description"}</p><Button size="compact" disabled={Boolean(busy)} aria-label={`Edit description for ${show.name}`} onClick={() => { setEditing(show); setDescription(show.description ?? ""); setError(null); }}>Edit description</Button></td>
                        <td><time dateTime={show.updated_at}>{savedTime(show.updated_at)}</time></td>
                        <td><Button disabled={Boolean(busy)} aria-label={`Create show from ${show.name}`} onClick={() => void create(`Creating show from ${show.name}…`, () => lifecycle?.initializeEmptyShow(show.id))}>Create show</Button></td>
                    </tr>)}</tbody>
                </table></WindowScrollArea> : <p>No base shows are saved. Select “Save as Template” in Save As to add one.</p>}
            </section>
            {busy && <p role="status">{busy}</p>}
            {error && <p role="alert" className="modal-warning">{error}</p>}
        </ModalFrame>
        {editing && <ModalFrame title={`Description — ${editing.name}`} ariaLabel={`Description for ${editing.name}`} dialogClassName="nested-modal show-description-modal" closeDisabled={Boolean(busy)} policy={{escape:!busy,backdrop:!busy}} onClose={() => { if (!busy) setEditing(null); }}>
                <TextInput value={description} onChange={event => setDescription(event.target.value)} maxLength={2000} disabled={Boolean(busy)} aria-label="Show description" autoFocus />
                <footer><Button disabled={Boolean(busy)} onClick={() => setEditing(null)}>Cancel</Button><Button variant="primary" disabled={Boolean(busy)} onClick={() => void saveDescription()}>Save description</Button></footer>
                {busy && <p role="status">{busy}</p>}{error && <p role="alert">{error}</p>}
        </ModalFrame>}
    </>;
}
