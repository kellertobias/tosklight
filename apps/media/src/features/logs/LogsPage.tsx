// The log.
//
// This exists because an operator at a venue has a browser and nothing else: no terminal, no log
// file they can reach. So the window the server keeps is readable here, newest last, with the level
// filter someone looking for a failure actually uses.
//
// Records accumulate rather than being replaced, and the cursor is the newest record already held,
// so a refresh cannot show one twice or step over one. When the server has had to discard records
// to stay bounded, that is said out loud.

import { Button, CheckboxField, SelectField } from "@tosklight/ui/controls";
import { useCallback, useEffect, useRef, useState } from "react";
import { ApiFailure, api } from "../../shared/api/client";
import type { LogRecordView } from "../../shared/api/generated/media-wire";

/// How often to ask for what has arrived. A log is not a meter; a couple of seconds is plenty.
const FOLLOW_MS = 2_000;

const LEVELS = [
	{ value: "trace", label: "Everything" },
	{ value: "debug", label: "Debug and above" },
	{ value: "info", label: "Information and above" },
	{ value: "warn", label: "Warnings and errors" },
	{ value: "error", label: "Errors only" },
] as const;

/// How many records to keep on screen. Beyond this the browser, not the server, is the bottleneck.
const KEEP = 2_000;

export function LogsPage() {
	const [level, setLevel] = useState<string>("info");
	const [following, setFollowing] = useState(true);
	const [records, setRecords] = useState<LogRecordView[]>([]);
	const [dropped, setDropped] = useState(0);
	const [failure, setFailure] = useState<ApiFailure | undefined>(undefined);
	// The newest record held, so the next read asks only for what came after it. A ref rather than
	// state because the follow timer reads it without wanting to be restarted by it.
	const cursor = useRef<number | undefined>(undefined);

	const read = useCallback(async () => {
		try {
			const page = await api.logs({ after: cursor.current, level, limit: 500 });
			setFailure(undefined);
			setDropped(page.dropped);
			if (page.records.length > 0) {
				cursor.current = page.records[page.records.length - 1].sequence;
				setRecords((current) => [...current, ...page.records].slice(-KEEP));
			}
		} catch (error) {
			setFailure(
				error instanceof ApiFailure
					? error
					: new ApiFailure("unexpected-error", String(error), 0),
			);
		}
	}, [level]);

	// Changing the level is a different question, so the answer starts again rather than mixing
	// records selected under two filters.
	useEffect(() => {
		cursor.current = undefined;
		setRecords([]);
	}, [level]);

	useEffect(() => {
		void read();
		if (!following) return;
		const timer = setInterval(() => void read(), FOLLOW_MS);
		return () => clearInterval(timer);
	}, [read, following]);

	return (
		<section className="media-page">
			<div className="media-logs-controls">
				<SelectField
					label="Show"
					value={level}
					options={LEVELS.map((entry) => ({ value: entry.value, label: entry.label }))}
					onChange={setLevel}
				/>
				<CheckboxField
					label="Keep up to date"
					stateLabel="Read new records automatically"
					checked={following}
					onChange={(event) => setFollowing(event.target.checked)}
				/>
				<Button onClick={() => void read()}>Read now</Button>
			</div>

			{failure && (
				<p className="media-state is-error" role="alert">
					{failure.disconnected
						? "The Media Server is not answering, so the log cannot be read."
						: failure.message}
				</p>
			)}

			{dropped > 0 && (
				<p className="media-state is-notice">
					This server has discarded {dropped} older {dropped === 1 ? "record" : "records"}{" "}
					to stay within the window it keeps in memory.
				</p>
			)}

			{records.length === 0 ? (
				<p className="media-state is-notice">
					Nothing has been logged at this level yet.
				</p>
			) : (
				<ol className="media-log" aria-label="Log records">
					{records.map((record) => (
						<li key={record.sequence} className={`media-log-${record.level}`}>
							<span className="media-log-at">{elapsed(record.millisSinceStart)}</span>
							<span className="media-log-level">{record.level}</span>
							<span className="media-log-message">{record.message}</span>
							<span className="media-log-target">{record.target}</span>
						</li>
					))}
				</ol>
			)}
		</section>
	);
}

/// How long after this server started a record was emitted.
///
/// Time since start rather than a wall clock, because that is what the process knows and what
/// matters when reading a startup sequence.
export function elapsed(millis: number): string {
	const total = Math.floor(millis / 1000);
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor((total % 3600) / 60);
	const seconds = total % 60;
	const pad = (value: number) => String(value).padStart(2, "0");
	return hours > 0
		? `${hours}:${pad(minutes)}:${pad(seconds)}`
		: `${pad(minutes)}:${pad(seconds)}`;
}
