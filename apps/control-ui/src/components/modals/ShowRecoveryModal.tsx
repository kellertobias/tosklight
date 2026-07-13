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
  return <div className="show-recovery-layer" role="alertdialog" aria-modal="true" aria-label="Show recovery required">
    <section className="show-recovery-card">
      <ModalTitleBar title="Show File Could Not Be Loaded"/>
      <p>The active show file might be corrupted or incompatible with this version. It has not been changed or deleted.</p>
      <pre>{error}</pre>
      <Button disabled={busy} onClick={() => void initialize()}>{busy ? "Initializing…" : "Initialize New Empty Show"}</Button>
      <small>This creates and activates a separate empty show. The damaged file remains available for recovery.</small>
      {server.error && <p className="modal-error">{server.error}</p>}
    </section>
  </div>;
}
