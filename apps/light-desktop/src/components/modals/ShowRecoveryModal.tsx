import { ServerErrorNotice } from "../shell/ServerErrorNotice";
import { useState } from "react";
import {
	useActiveShowError,
	useActiveShowId,
	useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useShowLifecycle } from "../../features/showLifecycle/ShowLifecycleContext";
import { Button, ModalRegistration, ModalTitleBar } from "@tosklight/ui";

export function ShowRecoveryModal() {
  const lifecycle = useShowLifecycle();
  const session = useSessionSnapshot();
  const activeShowId = useActiveShowId();
  const [busy, setBusy] = useState(false);
  const error = useActiveShowError();
  if (!error || !session || !lifecycle) return null;
  const initialize = async () => {
    setBusy(true);
    await lifecycle.initializeEmptyShow();
    setBusy(false);
  };
  const load = async (id: string) => {
    setBusy(true);
    await lifecycle.openShow(id, "safe_blackout");
    setBusy(false);
  };
  const loadCleanDefault = async () => {
    setBusy(true);
    await lifecycle.openCleanDefaultShow();
    setBusy(false);
  };
  const alternatives = lifecycle.shows.filter((show) => show.id !== activeShowId);
  return <ModalRegistration policy={{ escape: false, backdrop: false, explicit: false }} onClose={() => undefined}><div className="show-recovery-layer" role="alertdialog" aria-modal="true" aria-label="Show recovery required">
    <section className="show-recovery-card">
      <ModalTitleBar title="Show File Could Not Be Loaded"/>
      <p>The active show file might be corrupted or incompatible with this version. It has not been changed or deleted.</p>
      <pre>{error}</pre>
      {alternatives.length > 0 && <section className="show-recovery-alternatives" aria-label="Saved recovery shows">
        <b>Open another saved show</b>
        <small>Load Latest Autosave uses a safe blackout and leaves the damaged file untouched.</small>
        {alternatives.map((show) => <Button key={show.id} disabled={busy} aria-label={`Load Latest Autosave for ${show.name}`} onClick={() => void load(show.id)}>Load Latest Autosave · {show.name}</Button>)}
      </section>}
      <Button variant="primary" disabled={busy} onClick={() => void loadCleanDefault()}>{busy ? "Loading…" : "Load Clean Built-in Default"}</Button>
      <small>This creates and activates a separate show from the untouched built-in 49-fixture rig. The damaged file remains available for recovery.</small>
      <Button disabled={busy} onClick={() => void initialize()}>{busy ? "Initializing…" : "Initialize New Empty Show"}</Button>
      <small>This creates and activates a separate empty show. The damaged file remains available for recovery.</small>
      <ServerErrorNotice />
    </section>
  </div></ModalRegistration>;
}
