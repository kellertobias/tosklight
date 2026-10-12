import { Button, formatErrorDetails } from "@tosklight/ui";
import { useEffect, useState } from "react";
import {
 documentSession,
 type DocumentSummary,
 type StartupRecovery as Recovery,
} from "./document/session";
import { chooseNewShow, chooseShowToOpen } from "./FileBar";
import { WindowControls } from "./WindowChrome";
import "./startupRecovery.css";

type RecoveryProps = {
 onDocument: (summary: DocumentSummary) => void;
 onError: (reason: unknown) => void;
};

/** Startup has already refused the file; reading this never opens or changes it. */
export function StartupRecovery({ onDocument, onError }: RecoveryProps) {
 const [recovery, setRecovery] = useState<Recovery | null>(null);
 const [busy, setBusy] = useState(false);
 const [actionError, setActionError] = useState<string | null>(null);
 useEffect(() => {
  let mounted = true;
  documentSession.startupRecovery()
   .then(value => { if (mounted) setRecovery(value); })
   .catch(reason => { if (mounted) onError(reason); });
  return () => { mounted = false; };
 }, [onError]);
 async function choose(action: () => Promise<DocumentSummary | null>) {
  setBusy(true);
  setActionError(null);
  try {
   const summary = await action();
   if (summary) {
    onDocument(summary);
    setRecovery(null);
   }
  } catch (reason) {
   setActionError(formatErrorDetails(reason));
  } finally {
   setBusy(false);
  }
 }
 if (!recovery) return null;
 return (
  <section className="viz-startup-recovery" role="alert" aria-label="Recent show could not be opened">
   <strong>Recent show could not be opened</strong>
   <p>{recovery.path}</p>
   <p>{recovery.reason}</p>
   <p>{recovery.fallback ? (
    <>Opened a separate fallback: {recovery.fallback.name} — {recovery.fallback.path}.</>
   ) : (
    <>No fallback show is open.{recovery.fallbackError ? ` ${recovery.fallbackError}` : ""}</>
   )}</p>
   <p>The original file has not been changed. Open a supported show or create a separate new show.</p>
   {actionError && <p>{actionError}</p>}
   <div>
    <Button disabled={busy} onClick={() => void choose(chooseShowToOpen)}>Open Show</Button>
    <Button disabled={busy} onClick={() => void choose(chooseNewShow)}>New Show</Button>
    <Button disabled={busy} onClick={() => setRecovery(null)}>Dismiss</Button>
   </div>
  </section>
 );
}

export function ArchitectStartupHeader(props: RecoveryProps) {
 return <><WindowControls /><StartupRecovery {...props} /></>;
}
