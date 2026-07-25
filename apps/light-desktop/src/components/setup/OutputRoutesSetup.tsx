import { useMemo, useState } from "react";
import type { OutputRoute, VersionedObject } from "../../api/types";
import { Button, FormLayout, ModalPortal, NumberField, SelectField, SwitchField, TextField } from "../common";

interface RouteDraft {
  id: string;
  revision: number;
  body: OutputRoute;
}

export interface OutputRoutesSetupProps {
  routes: VersionedObject<OutputRoute>[];
  onSave: (id: string, route: OutputRoute, revision: number) => Promise<boolean>;
  onDelete: (id: string, revision: number) => Promise<boolean>;
  outputBindIp?: string;
}

function newRoute(): RouteDraft {
  return {
    id: `route-${crypto.randomUUID()}`,
    revision: 0,
    body: {
      protocol: "art_net",
      logical_universe: 1,
      destination_universe: 1,
      delivery_mode: "broadcast",
      destination: null,
      enabled: true,
      minimum_slots: 128,
    },
  };
}

function isIpv4(value: string): boolean {
  const parts = value.split(".");
  return parts.length === 4 && parts.every((part) => /^\d{1,3}$/.test(part) && Number(part) <= 255);
}

function validate(route: OutputRoute, outputBindIp?: string): string {
  if (!Number.isInteger(route.logical_universe) || route.logical_universe < 1 || route.logical_universe > 65_535)
    return "Logical universe must be a whole number from 1 to 65535.";
  const maximumUniverse = route.protocol === "art_net" ? 32_767 : 63_999;
  if (!Number.isInteger(route.destination_universe) || route.destination_universe < 1 || route.destination_universe > maximumUniverse)
    return `${route.protocol === "art_net" ? "Art-Net" : "sACN"} destination universe must be a whole number from 1 to ${maximumUniverse}.`;
  if (!Number.isInteger(route.minimum_slots) || route.minimum_slots < 1 || route.minimum_slots > 512)
    return "Minimum universe size must be a whole number from 1 to 512.";
  if (route.protocol === "art_net" && route.delivery_mode === "multicast") return "Art-Net supports Broadcast or Unicast delivery.";
  if (route.protocol === "sacn" && route.delivery_mode === "broadcast") return "sACN supports Multicast or Unicast delivery.";
  if (outputBindIp && !isIpv4(outputBindIp)) return "The output bind address must be an available IPv4 interface before this route can be saved.";
  if (route.delivery_mode === "unicast") {
    const destination = route.destination?.trim() ?? "";
    const separator = destination.lastIndexOf(":");
    const address = destination.slice(0, separator);
    const port = Number(destination.slice(separator + 1));
    if (separator < 0 || !isIpv4(address) || !Number.isInteger(port) || port < 1 || port > 65_535)
      return "Unicast delivery requires an IPv4 destination and port, for example 10.0.0.20:6454.";
  }
  return "";
}

function modeLabel(route: OutputRoute): string {
  if (route.protocol === "art_net") return route.delivery_mode === "unicast" ? "Art-Net Unicast" : "Art-Net Broadcast";
  return route.delivery_mode === "unicast" ? "sACN Unicast" : "sACN Multicast";
}

export function OutputRoutesSetup({ routes, onSave, onDelete, outputBindIp }: OutputRoutesSetupProps) {
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
    const destination = draft.body.delivery_mode === "unicast" ? draft.body.destination?.trim() || null : null;
    const next = { ...draft.body, destination };
    const issue = validate(next, outputBindIp);
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
          <small>{modeLabel(route.body)} · {route.body.destination || (route.body.protocol === "art_net" ? "255.255.255.255:6454" : `239.255.${route.body.destination_universe >> 8}.${route.body.destination_universe & 255}:5568`)} · Minimum {route.body.minimum_slots ?? 512} slots</small>
        </span>
        <span className={route.body.enabled ? "route-enabled" : "route-disabled"}>{route.body.enabled ? "Enabled" : "Disabled"}</span>
        <Button onClick={() => edit(route)}>Edit route</Button>
      </article>)}
      {!ordered.length && <p className="empty-window-message">No output routes are configured.</p>}
    </div>
    {draft && <ModalPortal><div className="modal-backdrop" onPointerDown={(event) => event.target === event.currentTarget && close()}>
      <section className="modal-card output-route-editor" role="dialog" aria-modal="true" aria-label="Output route editor">
        <Button className="modal-close" disabled={busy} onClick={close}>×</Button>
        <h2>{draft.revision ? "Edit output route" : "Add output route"}</h2>
        <FormLayout labelPlacement="side">
          <SelectField
            label="Protocol"
            value={draft.body.protocol}
            onChange={(protocol) => setDraft({ ...draft, body: { ...draft.body, protocol, delivery_mode: protocol === "art_net" ? "broadcast" : "multicast", destination: null } })}
            options={[{ value: "art_net", label: "Art-Net" }, { value: "sacn", label: "sACN" }]}
          />
          <SelectField
            label="Delivery mode"
            value={draft.body.delivery_mode}
            onChange={(delivery_mode) => setDraft({ ...draft, body: { ...draft.body, delivery_mode, destination: delivery_mode === "unicast" ? draft.body.destination : null } })}
            options={draft.body.protocol === "art_net"
              ? [{ value: "broadcast", label: "Broadcast" }, { value: "unicast", label: "Unicast" }]
              : [{ value: "multicast", label: "Multicast" }, { value: "unicast", label: "Unicast" }]}
          />
          <NumberField label="Logical universe" min="1" max="65535" value={draft.body.logical_universe} onChange={(event) => setDraft({ ...draft, body: { ...draft.body, logical_universe: Number(event.target.value) } })}/>
          <NumberField label="Destination universe" min="1" max="65535" value={draft.body.destination_universe} onChange={(event) => setDraft({ ...draft, body: { ...draft.body, destination_universe: Number(event.target.value) } })}/>
          <NumberField label="Minimum universe size" min="1" max="512" value={draft.body.minimum_slots ?? 512} description="Enabled routes send at least this many slots. Patched fixtures extend the frame when needed." onChange={(event) => setDraft({ ...draft, body: { ...draft.body, minimum_slots: Number(event.target.value) } })}/>
          {draft.body.delivery_mode === "unicast" && <TextField label="Destination" value={draft.body.destination ?? ""} description="Required IPv4 address and port, for example 10.0.0.20:6454." onChange={(event) => setDraft({ ...draft, body: { ...draft.body, destination: event.target.value } })}/>}
          {draft.body.delivery_mode === "broadcast" && <p className="field-description">Art-Net Broadcast uses the global destination 255.255.255.255:6454. The desk's output bind address selects the lighting-network interface.</p>}
          {draft.body.delivery_mode === "multicast" && <p className="field-description">sACN Multicast derives its 239.255.x.y:5568 destination from the destination universe.</p>}
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
    </div></ModalPortal>}
  </section>;
}
