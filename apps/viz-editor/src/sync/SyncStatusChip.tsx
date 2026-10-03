import { Button, ModalFrame } from "@tosklight/ui";
import { useEffect, useState } from "react";
import {
	type SyncConflict,
	type SyncResolution,
	type SyncStatus,
	syncSession,
} from "../document/sync";

/**
 * How a document opened from ToskLight Control stands with its desk, always in view.
 *
 * A document bound to no desk shows nothing. The chip never says "Saved to Control" unless the
 * desk has confirmed every edit; pending, offline, conflicted and failed edits are said as such,
 * and every one of them is already saved on this computer.
 */
export function SyncStatusChip({ documentKey }: { documentKey: string | null }) {
	const [status, setStatus] = useState<SyncStatus | null>(null);
	const [open, setOpen] = useState(false);
	useEffect(() => {
		let active = true;
		syncSession
			.status()
			.then((next) => {
				if (active) setStatus(next ?? null);
			})
			.catch(() => {
				if (active) setStatus(null);
			});
		return () => {
			active = false;
		};
	}, [documentKey]);
	useEffect(() => {
		let unlisten: (() => void) | undefined;
		let disposed = false;
		syncSession
			.onStatusChanged(setStatus)
			.then((stop) => {
				if (disposed) stop();
				else unlisten = stop;
			})
			.catch(() => undefined);
		return () => {
			disposed = true;
			unlisten?.();
		};
	}, []);
	if (!status) return null;
	return (
		<>
			<button
				type="button"
				className={`viz-sync-chip is-${status.state}`}
				title={status.detail}
				aria-label={`Sync with ${status.deskName}: ${status.label}`}
				onClick={() => setOpen(true)}
			>
				<span className="viz-sync-dot" aria-hidden="true" />
				<span>{status.label}</span>
			</button>
			{open && <SyncPanel status={status} onClose={() => setOpen(false)} />}
		</>
	);
}

function describe(value: unknown): string {
	if (value === null || value === undefined) return "(none)";
	if (typeof value === "string") return value;
	const text = JSON.stringify(value);
	return text.length > 80 ? `${text.slice(0, 77)}…` : text;
}

/** The status in words, and every decision the operator owes, each with both versions shown. */
export function SyncPanel({
	status,
	onClose,
}: {
	status: SyncStatus;
	onClose: () => void;
}) {
	const [conflicts, setConflicts] = useState<SyncConflict[]>([]);
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<string | null>(null);
	useEffect(() => {
		syncSession
			.conflicts()
			.then(setConflicts)
			.catch((reason) => setFailure(String(reason)));
	}, [status]);
	async function act(action: () => Promise<void>) {
		setBusy(true);
		setFailure(null);
		try {
			await action();
			setConflicts(await syncSession.conflicts());
		} catch (reason) {
			setFailure(String(reason));
		} finally {
			setBusy(false);
		}
	}
	const resolve = (entry: number, resolution: SyncResolution) =>
		act(() => syncSession.resolve(entry, resolution));
	const entries = [...new Set(conflicts.map((conflict) => conflict.entry))];
	const offline = status.state === "offline" && status.detail.startsWith("Working offline");
	return (
		<ModalFrame
			title={`Sync with ${status.deskName}`}
			ariaLabel={`Sync with ${status.deskName}`}
			closeLabel="Close sync status"
			dialogClassName="viz-sync-panel"
			onClose={onClose}
		>
			<p className={`viz-sync-detail is-${status.state}`}>{status.detail}</p>
			<dl className="viz-sync-facts">
				<dt>On this computer</dt>
				<dd>{status.savedOnThisComputer ? "Saved" : "Not saved"}</dd>
				<dt>On Control</dt>
				<dd>
					{status.savedToControl
						? "Saved to Control"
						: status.pending > 0
							? `${status.pending} change${status.pending === 1 ? "" : "s"} not yet confirmed`
							: "Not confirmed"}
				</dd>
			</dl>
			{entries.map((entry) => {
				const items = conflicts.filter((conflict) => conflict.entry === entry);
				const refused = items.every((conflict) => conflict.reason === "refused");
				return (
					<section key={entry} className="viz-sync-conflict" aria-label="Sync conflict">
						<ul>
							{items.map((conflict) => (
								<li key={`${conflict.kind}/${conflict.id}${conflict.path}`}>
									<strong>{conflict.label}</strong>
									{!refused && (
										<span>
											Control: {describe(conflict.theirs)} · Yours: {describe(conflict.mine)}
										</span>
									)}
								</li>
							))}
						</ul>
						<div className="viz-sync-conflict-actions">
							<Button disabled={busy} onClick={() => void resolve(entry, "keep_control")}>
								{refused ? "Discard my change" : "Keep Control's"}
							</Button>
							<Button disabled={busy} onClick={() => void resolve(entry, "use_mine")}>
								{refused ? "Send again" : "Use mine"}
							</Button>
						</div>
					</section>
				);
			})}
			{failure && <p role="alert">{failure}</p>}
			<div className="viz-sync-panel-actions">
				{status.state === "error" && conflicts.length === 0 && (
					<Button disabled={busy} onClick={() => void act(syncSession.dismissError)}>
						Dismiss
					</Button>
				)}
				<Button
					disabled={busy}
					onClick={() => void act(() => syncSession.setOnline(offline))}
				>
					{offline ? "Reconnect" : "Work offline"}
				</Button>
			</div>
		</ModalFrame>
	);
}
