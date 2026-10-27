import { useEffect, useRef, useState } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import type { StageStillStatus, StageStillPhase, StagePanePicture } from "../../platform/desktop/types";

export const terminalStillPhase = (phase: StageStillPhase) => phase === "cancelled" || (typeof phase === "object" && ("saved" in phase || "failed" in phase));
type Request = { paneId: string; requestId: string; jobId: string | null; terminal: boolean };

/** One frozen request per pane, driven only by pushed native observations. */
export function useStageCapture(paneId: string, enabled: boolean, picture: StagePanePicture) {
	const desktop = useDesktopBridge();
	const [status, setStatus] = useState<StageStillStatus | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [subscribed, setSubscribed] = useState(false);
	const [commandPending, setCommandPending] = useState(false);
	const request = useRef<Request | null>(null);
	const command = useRef(false);
	const mounted = useRef(false);
	const available = Boolean(desktop.available && desktop.startStagePaneCapture && desktop.cancelStagePaneCapture && desktop.onStageStill);
	useEffect(() => {
		mounted.current = true;
		request.current = null; command.current = false;
		setStatus(null); setError(null); setBusy(false); setSubscribed(false); setCommandPending(false);
		let disposed = false;
		let unsubscribe: (() => void) | undefined;
		if (available) {
			void desktop.onStageStill?.((event) => {
				const current = request.current;
				if (disposed || !current || current.terminal || event.paneId !== current.paneId || event.requestId !== current.requestId) return;
				if (event.error) {
					current.terminal = true; setError(event.error); setBusy(false); command.current = false; setCommandPending(false); return;
				}
				const next = event.status;
				if (!next || next.paneId !== current.paneId || next.requestId !== current.requestId || next.width !== 3840 || next.height !== 2160 ||
					(current.jobId !== null && next.jobId !== current.jobId)) return;
				current.jobId = next.jobId;
				current.terminal = terminalStillPhase(next.phase);
				setStatus(next); setBusy(!current.terminal);
				command.current = false; setCommandPending(false);
			}).then((stop) => {
				if (disposed) stop(); else { unsubscribe = stop; setSubscribed(true); }
			}).catch((reason) => { if (!disposed) setError(`Cannot receive render progress: ${String(reason)}`); });
		}
		return () => { disposed = true; mounted.current = false; unsubscribe?.(); };
	}, [available, desktop, paneId]);

	const start = async () => {
		if (!enabled || !available || !subscribed || (request.current && !request.current.terminal)) return;
		// Own the invocation look now, before any bridge operation or state changes can await.
		const invocationPicture = structuredClone(picture);
		const current: Request = { paneId, requestId: crypto.randomUUID(), jobId: null, terminal: false };
		request.current = current; command.current = false;
		setStatus(null); setError(null); setBusy(true); setCommandPending(false);
		try { await desktop.startStagePaneCapture?.(paneId, current.requestId, invocationPicture); }
		catch (reason) {
			if (mounted.current && request.current === current && !current.terminal) {
				current.terminal = true; setBusy(false); setError(String(reason));
			}
		}
	};
	const cancel = async () => {
		const current = request.current;
		if (!current?.jobId || current.terminal || command.current) return;
		command.current = true; setCommandPending(true); setError(null);
		try {
			await desktop.cancelStagePaneCapture?.(paneId, current.jobId);
		} catch (reason) {
			if (mounted.current && request.current === current && !current.terminal) {
				command.current = false; setCommandPending(false); setError(String(reason));
			}
		}
	};
	return { status, error, busy, commandPending, available, ready: available && subscribed && enabled,
		start, cancel, openSaved: async (reveal: boolean) => {
			const owner = request.current;
			try { await desktop.openStagePaneCapture?.(paneId, reveal); }
			catch (reason) { if (mounted.current && request.current === owner) setError(String(reason)); }
		}, canOpenSaved: Boolean(desktop.openStagePaneCapture),
		canCancel: busy && Boolean(request.current?.jobId) && !commandPending && !(status && typeof status.phase === "object" && "cancelling" in status.phase) };
}
