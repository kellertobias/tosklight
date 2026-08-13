// Bringing media into the library.
//
// A library carried over from the previous Media Server is full of `.mp4` and `.png` files, and
// so is a folder an operator has just dropped clips into. None of it plays until it has been
// converted, so this panel says what is waiting, converts it, and shows the work happening.
//
// Import is slow by nature — minutes for a show's worth of clips — so nothing here is silent: each
// job reports how far along it is, a failure keeps its reason on screen, and anything still going
// can be stopped.

import { Button } from "@tosklight/ui/controls";
import { useEffect, useState } from "react";
import { addressLabel } from "../../entities/catalog";
import { ApiFailure, api } from "../../shared/api/client";
import { requestId } from "../../shared/api/editing";
import type {
	ImportJobView,
	ImportsView,
} from "../../shared/api/generated/media-wire";
import { useTelemetry } from "../../shared/api/telemetry";

/// How often to re-read what is waiting. Only the *jobs* are pushed; what is on disk changes when
/// somebody copies a file in, which no socket can know about.
const PENDING_POLL_MS = 5_000;

export interface ImportPanelProps {
	/// Called when an import finishes, so the catalog is read again and the new clip appears.
	onImported: () => void;
}

export function ImportPanel({ onImported }: ImportPanelProps) {
	const [imports, setImports] = useState<ImportsView | undefined>(undefined);
	const [failure, setFailure] = useState<ApiFailure | undefined>(undefined);
	const [busy, setBusy] = useState(false);
	const telemetry = useTelemetry();

	// The socket carries the jobs; this read carries what is still waiting on disk.
	useEffect(() => {
		let cancelled = false;
		const read = async () => {
			try {
				const next = await api.imports();
				if (!cancelled) setImports(next);
			} catch {
				// The connection indicator in the shell already says the server is unreachable.
			}
		};
		void read();
		const timer = setInterval(() => void read(), PENDING_POLL_MS);
		return () => {
			cancelled = true;
			clearInterval(timer);
		};
	}, []);

	// Pushed jobs win over the ones the snapshot came with, so a progress bar moves smoothly.
	const jobs = telemetry.frame?.imports ?? imports?.jobs ?? [];
	const finished = jobs.filter((job) => job.state === "succeeded").length;

	// Once something has been converted the catalog holds something new, and what was waiting has
	// shrunk. Both are re-read rather than guessed at.
	useEffect(() => {
		if (finished === 0) return;
		onImported();
		void api
			.imports()
			.then(setImports)
			.catch(() => undefined);
	}, [finished, onImported]);

	if (!imports) return null;

	const pending = imports.pending;
	const running = jobs.filter(
		(job) => job.state === "queued" || job.state === "running",
	);

	if (pending.length === 0 && jobs.length === 0) return null;

	const start = async () => {
		setBusy(true);
		try {
			setImports(await api.startImport({ requestId: requestId() }));
			setFailure(undefined);
		} catch (error) {
			setFailure(
				error instanceof ApiFailure
					? error
					: new ApiFailure("unexpected-error", String(error), 0),
			);
		} finally {
			setBusy(false);
		}
	};

	return (
		<article className="media-settings-section" aria-label="Import">
			<header className="media-import-header">
				<h2>Import</h2>
				{pending.length > 0 && (
					<Button
						variant="primary"
						loading={busy}
						disabled={!imports.canImport || running.length > 0}
						onClick={() => void start()}
					>
						Convert {pending.length} {pending.length === 1 ? "file" : "files"}
					</Button>
				)}
			</header>

			{!imports.canImport && (
				<p className="media-state is-error" role="alert">
					This machine cannot convert media: FFmpeg is not installed or not on
					PATH. Install it and restart the server.
				</p>
			)}

			{failure && (
				<p className="media-state is-error" role="alert">
					{failure.message}{" "}
					<Button size="compact" onClick={() => setFailure(undefined)}>
						Dismiss
					</Button>
				</p>
			)}

			{pending.length > 0 && (
				<>
					<p className="media-state is-notice">
						These files are in the library but not in a format this server can
						play. Converting them leaves the originals where they are.
					</p>
					<table className="media-table">
						<caption className="media-visually-hidden">
							Waiting to be imported
						</caption>
						<thead>
							<tr>
								<th scope="col">Address</th>
								<th scope="col">File</th>
								<th scope="col">Becomes</th>
							</tr>
						</thead>
						<tbody>
							{pending.map((item) => (
								<tr key={`${item.address.folder}/${item.address.file}`}>
									<td>
										{addressLabel(item.address.folder, item.address.file)}
									</td>
									<td>{item.filename}</td>
									<td>{item.name}</td>
								</tr>
							))}
						</tbody>
					</table>
				</>
			)}

			{jobs.length > 0 && (
				<ul className="media-import-jobs" aria-label="Imports">
					{jobs.map((job) => (
						<ImportRow key={job.id} job={job} />
					))}
				</ul>
			)}
		</article>
	);
}

function ImportRow({ job }: { job: ImportJobView }) {
	const percent =
		job.fraction === null ? undefined : Math.round(job.fraction * 100);
	const active = job.state === "queued" || job.state === "running";

	return (
		<li className={`media-import-job is-${job.state}`}>
			<span className="media-import-name">
				{addressLabel(job.address.folder, job.address.file)} · {job.filename}
			</span>

			{active ? (
				<div
					className="media-import-progress"
					role="progressbar"
					aria-label={`Converting ${job.filename}`}
					aria-valuenow={percent}
					aria-valuemin={0}
					aria-valuemax={100}
				>
					{/* An indeterminate bar rather than an invented percentage: some sources do
					    not report a frame count, and a fake number is worse than none. */}
					<span
						className={percent === undefined ? "is-indeterminate" : undefined}
						style={percent === undefined ? undefined : { width: `${percent}%` }}
					/>
				</div>
			) : (
				<span
					className={`media-badge ${job.state === "succeeded" ? "is-good" : "is-bad"}`}
				>
					{describe(job)}
				</span>
			)}

			{active && (
				<Button
					size="compact"
					onClick={() => void api.cancelImport(job.id).catch(() => undefined)}
				>
					Stop
				</Button>
			)}
			{job.reason && <span className="media-import-reason">{job.reason}</span>}
		</li>
	);
}

function describe(job: ImportJobView): string {
	switch (job.state) {
		case "succeeded":
			if (job.framesDone === null) return "Imported";
			return `${job.framesDone} ${job.framesDone === 1 ? "frame" : "frames"}`;
		case "cancelled":
			return "Stopped";
		default:
			return "Failed";
	}
}
