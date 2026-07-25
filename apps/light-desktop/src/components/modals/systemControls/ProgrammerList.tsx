import type { ProgrammerLifecycleRow } from "../../../features/programmerLifecycle/contracts";
import { Button } from "../../common";

interface ProgrammerListProps {
	programmers: readonly ProgrammerLifecycleRow[];
	loading: boolean;
	currentUserId: string | null;
	currentUserName: string | null;
	onClear(sessionId: string): void;
}

export function ProgrammerList({
	programmers,
	loading,
	currentUserId,
	currentUserName,
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
						currentUser={programmer.userId === currentUserId}
						currentUserName={currentUserName}
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
	currentUser,
	currentUserName,
	onClear,
}: {
	programmer: ProgrammerLifecycleRow;
	currentUser: boolean;
	currentUserName: string | null;
	onClear(sessionId: string): void;
}) {
	const userLabel = currentUser
		? `${currentUserName ?? "User"} · Current user`
		: `User ${programmer.userId.slice(0, 8)}`;
	const sessionSummary = `${programmer.sessions.length} session${programmer.sessions.length === 1 ? "" : "s"}`;
	const clearSession = programmer.sessions[0]?.sessionId;
	return (
		<article>
			<span>
				<b>{userLabel}</b>
				<small>
					{programmer.selectedFixtureCount} fixtures ·{" "}
					{programmer.normalValueCount} values · {sessionSummary} ·{" "}
					{programmer.connected ? "Connected" : "Disconnected"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Clear programmer ${programmer.userId}`}
				disabled={!clearSession}
				onClick={() => clearSession && onClear(clearSession)}
			>
				Clear
			</Button>
		</article>
	);
}
