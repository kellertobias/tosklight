import { useCallback, useRef, useState } from "react";
import type { CueList } from "../../api/types";
import type { TimecodeDefinition } from "../../api/types/timecode";

export interface CueTimingEdit {
	cueListId: string;
	before: CueList;
	after: CueList;
}
interface HistoryEntry {
	before: TimecodeDefinition;
	after: TimecodeDefinition;
	timings: CueTimingEdit[];
}
interface HistoryState {
	past: HistoryEntry[];
	present: TimecodeDefinition;
	future: HistoryEntry[];
}

export function useTimecodeEditorHistory(
	initial: TimecodeDefinition,
	apply?: (
		definition: TimecodeDefinition,
		timings: CueTimingEdit[],
		undo: boolean,
	) => Promise<void>,
) {
	const [history, setHistory] = useState<HistoryState>({
		past: [],
		present: initial,
		future: [],
	});
	const latest = useRef(history);
	latest.current = history;
	const gestureStart = useRef<{
		definition: TimecodeDefinition;
		timings: CueTimingEdit[];
	} | null>(null);
	const pending = useRef(false);
	const [saving, setSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const commit = useCallback((next: TimecodeDefinition) => {
		if (pending.current) return;
		setHistory((current) =>
			equal(current.present, next)
				? current
				: {
						past: [
							...current.past.slice(-99),
							{ before: current.present, after: next, timings: [] },
						],
						present: next,
						future: [],
					},
		);
	}, []);
	const beginGesture = useCallback(() => {
		if (pending.current) return;
		gestureStart.current = { definition: latest.current.present, timings: [] };
	}, []);
	const preview = useCallback((next: TimecodeDefinition) => {
		if (pending.current) return;
		setHistory((current) => ({ ...current, present: next }));
	}, []);
	const recordCueTiming = useCallback(
		(cueListId: string, before: CueList, after: CueList) => {
			if (equal(before, after)) return;
			const edit = { cueListId, before, after };
			if (gestureStart.current) {
				const existing = gestureStart.current.timings.find(
					(item) => item.cueListId === cueListId,
				);
				if (existing) existing.after = after;
				else gestureStart.current.timings.push(edit);
			} else {
				setHistory((current) => ({
					...current,
					future: [],
					past: [
						...current.past.slice(-99),
						{
							before: current.present,
							after: current.present,
							timings: [edit],
						},
					],
				}));
			}
		},
		[],
	);
	const endGesture = useCallback(() => {
		const start = gestureStart.current;
		gestureStart.current = null;
		if (!start || pending.current) return;
		setHistory((current) =>
			equal(start.definition, current.present) && !start.timings.length
				? current
				: {
						past: [
							...current.past.slice(-99),
							{
								before: start.definition,
								after: current.present,
								timings: start.timings,
							},
						],
						present: current.present,
						future: [],
					},
		);
	}, []);
	const move = useCallback(
		async (undo: boolean) => {
			if (pending.current) return;
			const current = latest.current;
			const entry = undo ? current.past.at(-1) : current.future[0];
			if (!entry) return;
			pending.current = true;
			setSaving(true);
			setError(null);
			try {
				const target = undo ? entry.before : entry.after;
				await apply?.(target, entry.timings, undo);
				if (latest.current !== current)
					throw new Error(
						"The Timecode changed while history was saving. Try again after the edit settles.",
					);
				setHistory(
					undo
						? {
								past: current.past.slice(0, -1),
								present: target,
								future: [entry, ...current.future].slice(0, 100),
							}
						: {
								past: [...current.past.slice(-99), entry],
								present: target,
								future: current.future.slice(1),
							},
				);
			} catch (cause) {
				setError(
					`Could not ${undo ? "undo" : "redo"} the complete Timecode edit: ${String(cause)}`,
				);
			} finally {
				pending.current = false;
				setSaving(false);
			}
		},
		[apply],
	);
	return {
		draft: history.present,
		commit,
		preview,
		beginGesture,
		endGesture,
		recordCueTiming,
		canEdit: () => !pending.current,
		undo: () => void move(true),
		redo: () => void move(false),
		saving,
		error,
		canUndo: history.past.length > 0,
		canRedo: history.future.length > 0,
	};
}
function equal(left: unknown, right: unknown): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
