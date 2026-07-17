import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import { Button, ModalTitleBar } from "../common";

export function ShowRecoveryModal() {
  const server = useServer();
  const [busy, setBusy] = useState(false);
  const error = server.bootstrap?.active_show_error;
  if (!error || !server.session) return null;
  const initialize = async () => {
    setBusy(true);
    await server.initializeEmptyShow();
    setBusy(false);
  };
  const load = async (id: string) => {
    setBusy(true);
    await server.openShow(id, "safe_blackout");
    setBusy(false);
  };
  const alternatives = server.shows.filter((show) => show.id !== server.bootstrap?.active_show?.id);
  return <div className="show-recovery-layer" role="alertdialog" aria-modal="true" aria-label="Show recovery required">
    <section className="show-recovery-card">
      <ModalTitleBar title="Show File Could Not Be Loaded"/>
      <p>The active show file might be corrupted or incompatible with this version. It has not been changed or deleted.</p>
      <pre>{error}</pre>
      {alternatives.length > 0 && <section className="show-recovery-alternatives" aria-label="Saved recovery shows">
        <b>Open another saved show</b>
        <small>Load Latest Autosave uses a safe blackout and leaves the damaged file untouched.</small>
        {alternatives.map((show) => <Button key={show.id} disabled={busy} aria-label={`Load Latest Autosave for ${show.name}`} onClick={() => void load(show.id)}>Load Latest Autosave · {show.name}</Button>)}
      </section>}
      <Button disabled={busy} onClick={() => void initialize()}>{busy ? "Initializing…" : "Initialize New Empty Show"}</Button>
      <small>This creates and activates a separate empty show. The damaged file remains available for recovery.</small>
      {server.error && <p className="modal-error">{server.error}</p>}
    </section>
  </div>;
}
