import { useMemo, useState } from "react";
import type { OutputRoute, VersionedObject } from "../../api/types";
import { Button, FormLayout, NumberField, SelectField, SwitchField, TextField } from "../common";

interface RouteDraft {
  id: string;
  revision: number;
  body: OutputRoute;
}

export interface OutputRoutesSetupProps {
  routes: VersionedObject<OutputRoute>[];
  onSave: (id: string, route: OutputRoute, revision: number) => Promise<boolean>;
  onDelete: (id: string, revision: number) => Promise<boolean>;
}

function newRoute(): RouteDraft {
  return {
    id: `route-${crypto.randomUUID()}`,
    revision: 0,
    body: {
      protocol: "art_net",
      logical_universe: 1,
      destination_universe: 1,
      destination: "",
      enabled: true,
      minimum_slots: 128,
    },
  };
}

function validate(route: OutputRoute): string {
  if (!Number.isInteger(route.logical_universe) || route.logical_universe < 1 || route.logical_universe > 65_535)
    return "Logical universe must be a whole number from 1 to 65535.";
  if (!Number.isInteger(route.destination_universe) || route.destination_universe < 1 || route.destination_universe > 65_535)
    return "Destination universe must be a whole number from 1 to 65535.";
  if (!Number.isInteger(route.minimum_slots) || route.minimum_slots < 1 || route.minimum_slots > 512)
    return "Minimum universe size must be a whole number from 1 to 512.";
  if (route.protocol === "art_net" && !route.destination?.trim())
    return "Art-Net routes require a destination address and port.";
  return "";
}

export function OutputRoutesSetup({ routes, onSave, onDelete }: OutputRoutesSetupProps) {
  const [draft, setDraft] = useState<RouteDraft | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const ordered = useMemo(
    () => [...routes].sort((left, right) =>
      left.body.logical_universe - right.body.logical_universe
      || left.body.protocol.localeCompare(right.body.protocol)
      || left.id.localeCompare(right.id)),
    [routes],
  );
  const edit = (route: VersionedObject<OutputRoute>) => {
    setError("");
    setConfirmDelete(false);
    setDraft({ id: route.id, revision: route.revision, body: { ...route.body } });
  };
  const close = () => {
    setDraft(null);
    setError("");
    setConfirmDelete(false);
  };
  const save = async () => {
    if (!draft) return;
    const destination = draft.body.destination?.trim() || null;
    const next = { ...draft.body, destination };
    const issue = validate(next);
    if (issue) {
      setError(issue);
      return;
    }
    setBusy(true);
    const saved = await onSave(draft.id, next, draft.revision);
    setBusy(false);
    if (saved) close();
    else setError("The route was not saved. Check its destination and refresh after a revision conflict.");
  };
  const remove = async () => {
    if (!draft || draft.revision === 0) return close();
    setBusy(true);
    const removed = await onDelete(draft.id, draft.revision);
    setBusy(false);
    if (removed) close();
    else setError("The route was not removed. Refresh after a revision conflict and try again.");
  };

  return <section className="output-routes-setup" aria-label="Output routes">
    <header>
      <div><h3>Routes</h3><small>Map logical show universes to Art-Net or sACN destinations.</small></div>
      <Button onClick={() => { setDraft(newRoute()); setError(""); setConfirmDelete(false); }}>Add route</Button>
    </header>
    <div className="setup-list output-route-list">
      {ordered.map((route) => <article key={route.id}>
        <span>
          <b>Logical {route.body.logical_universe} → {route.body.protocol === "art_net" ? "Art-Net" : "sACN"} {route.body.destination_universe}</b>
          <small>{route.body.destination || "Protocol default destination"} · Minimum {route.body.minimum_slots ?? 512} slots</small>
        </span>
        <span className={route.body.enabled ? "route-enabled" : "route-disabled"}>{route.body.enabled ? "Enabled" : "Disabled"}</span>
        <Button onClick={() => edit(route)}>Edit route</Button>
      </article>)}
      {!ordered.length && <p className="empty-window-message">No output routes are configured.</p>}
    </div>
    {draft && <div className="modal-backdrop" onPointerDown={(event) => event.target === event.currentTarget && close()}>
      <section className="modal-card output-route-editor" role="dialog" aria-modal="true" aria-label="Output route editor">
        <Button className="modal-close" disabled={busy} onClick={close}>×</Button>
        <h2>{draft.revision ? "Edit output route" : "Add output route"}</h2>
        <FormLayout labelPlacement="side">
          <SelectField
            label="Protocol"
            value={draft.body.protocol}
            onChange={(protocol) => setDraft({ ...draft, body: { ...draft.body, protocol } })}
            options={[{ value: "art_net", label: "Art-Net" }, { value: "sacn", label: "sACN" }]}
          />
          <NumberField label="Logical universe" min="1" max="65535" value={draft.body.logical_universe} onChange={(event) => setDraft({ ...draft, body: { ...draft.body, logical_universe: Number(event.target.value) } })}/>
          <NumberField label="Destination universe" min="1" max="65535" value={draft.body.destination_universe} onChange={(event) => setDraft({ ...draft, body: { ...draft.body, destination_universe: Number(event.target.value) } })}/>
          <NumberField label="Minimum universe size" min="1" max="512" value={draft.body.minimum_slots ?? 512} description="Enabled routes send at least this many slots. Patched fixtures extend the frame when needed." onChange={(event) => setDraft({ ...draft, body: { ...draft.body, minimum_slots: Number(event.target.value) } })}/>
          <TextField label="Destination" value={draft.body.destination ?? ""} description={draft.body.protocol === "art_net" ? "Required IP address and port, for example 10.0.0.20:6454." : "Optional IP address and port. Leave empty for sACN multicast."} onChange={(event) => setDraft({ ...draft, body: { ...draft.body, destination: event.target.value } })}/>
          <SwitchField label="Enabled" checked={draft.body.enabled} onChange={(event) => setDraft({ ...draft, body: { ...draft.body, enabled: event.target.checked } })}/>
        </FormLayout>
        {error && <p className="ui-field-error" role="alert">{error}</p>}
        {confirmDelete ? <div className="delete-confirm">
          <b>Remove this output route?</b>
          <Button disabled={busy} onClick={() => setConfirmDelete(false)}>Cancel</Button>
          <Button className="danger" disabled={busy} onClick={() => void remove()}>Confirm remove</Button>
        </div> : <footer className="modal-actions">
          {draft.revision > 0 && <Button className="danger" disabled={busy} onClick={() => setConfirmDelete(true)}>Remove route</Button>}
          <Button disabled={busy} onClick={close}>Cancel</Button>
          <Button variant="primary" disabled={busy} onClick={() => void save()}>{busy ? "Saving…" : "Save route"}</Button>
        </footer>}
      </section>
    </div>}
  </section>;
}
