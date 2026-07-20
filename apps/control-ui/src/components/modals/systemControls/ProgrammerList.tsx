import type { ProgrammerState } from "../../../api/types";
import { useNormalProgrammerValueCount } from "../../../features/programmerValues/useProgrammerValuesActivity";
import { Button } from "../../common";

interface ProgrammerListProps {
	programmers: readonly ProgrammerState[];
	currentUserId: string | null;
	currentUserName: string | null;
	onClear(sessionId: string): void;
}

export function ProgrammerList({
	programmers,
	currentUserId,
	currentUserName,
	onClear,
}: ProgrammerListProps) {
	const currentUserValueCount = useNormalProgrammerValueCount(true);
	return (
		<section>
			<h3>
				Active programmers <small>{programmers.length}</small>
			</h3>
			<div className="programmer-list">
				{programmers.map((programmer) => (
					<ProgrammerRow
						key={programmer.session_id}
						programmer={programmer}
						currentUser={programmer.user_id === currentUserId}
						currentUserName={currentUserName}
						currentUserValueCount={currentUserValueCount}
						onClear={onClear}
					/>
				))}
				{!programmers.length && (
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
	currentUserValueCount,
	onClear,
}: {
	programmer: ProgrammerState;
	currentUser: boolean;
	currentUserName: string | null;
	currentUserValueCount: number | null;
	onClear(sessionId: string): void;
}) {
	const userLabel = currentUser
		? `${currentUserName ?? "User"} · Current user`
		: `User ${programmer.user_id.slice(0, 8)}`;
	const valueSummary = currentUser
		? currentUserValueCount === null
			? "Values loading…"
			: `${currentUserValueCount} values`
		: `${legacyValueCount(programmer)} values`;
	return (
		<article>
			<span>
				<b>{userLabel}</b>
				<small>
					{programmer.selected.length} fixtures · {valueSummary} ·{" "}
					{programmer.connected ? "Connected" : "Disconnected"}
				</small>
			</span>
			<Button
				className="danger"
				aria-label={`Clear programmer ${programmer.user_id}`}
				onClick={() => onClear(programmer.session_id)}
			>
				Clear
			</Button>
		</article>
	);
}

function legacyValueCount(programmer: ProgrammerState) {
	return (
		programmer.values.length +
		Object.values(programmer.group_values ?? {}).reduce(
			(total, values) => total + Object.keys(values).length,
			0,
		)
	);
}
