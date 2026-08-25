import type { ProgrammerLifecycleRow } from "../../../features/programmerLifecycle/contracts";
import { Button } from "@tosklight/ui";

interface ProgrammerListProps {
	programmers: readonly ProgrammerLifecycleRow[];
	loading: boolean;
	onClear(sessionId: string): void;
}

export function ProgrammerList({
	programmers,
	loading,
	onClear,
}: ProgrammerListProps) {
	return (
		<section>
			<h3>
				Active programmers <small>{programmers.length}</small>
			</h3>
			<div className="programmer-list">
				{programmers.map((programmer) => (
					<ProgrammerRow
						key={programmer.programmerId}
						programmer={programmer}
						onClear={onClear}
					/>
				))}
				{loading && (
					<p className="empty-window-message">Programmers loading…</p>
				)}
				{!loading && !programmers.length && (
					<p className="empty-window-message">No active programmers.</p>
				)}
			</div>
		</section>
	);
}

function ProgrammerRow({
	programmer,
	onClear,
}: {
	programmer: ProgrammerLifecycleRow;
	onClear(sessionId: string): void;
}) {
	// There is one Programmer, and it is this desk's.
	const deskLabel = "Operator · This desk";
	const sessionSummary = `${programmer.sessions.length} session${programmer.sessions.length === 1 ? "" : "s"}`;
	const fixtureSummary = `${programmer.selectedFixtureCount} selected fixture${programmer.selectedFixtureCount === 1 ? "" : "s"}`;
	const clearSession = programmer.sessions[0]?.sessionId;
	return (
		<article>
			<span>
				<b>{deskLabel}</b>
				<small>
					{fixtureSummary} · {programmer.normalValueCount} values · {sessionSummary} ·{" "}
					{programmer.connected ? "Connected" : "Disconnected"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label="Clear programmer"
				disabled={!clearSession}
				onClick={() => clearSession && onClear(clearSession)}
			>
				Clear
			</Button>
		</article>
	);
}
