import { useCallback, useRef, useState } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";

interface HistoryState {
	past: TimecodeDefinition[];
	present: TimecodeDefinition;
	future: TimecodeDefinition[];
}

export function useTimecodeEditorHistory(initial: TimecodeDefinition) {
	const [history, setHistory] = useState<HistoryState>({
		past: [],
		present: initial,
		future: [],
	});
	const gestureStart = useRef<TimecodeDefinition | null>(null);

	const commit = useCallback((next: TimecodeDefinition) => {
		setHistory((current) => {
			if (equal(current.present, next)) return current;
			return {
				past: [...current.past.slice(-99), current.present],
				present: next,
				future: [],
			};
		});
	}, []);

	const beginGesture = useCallback(() => {
		setHistory((current) => {
			gestureStart.current = current.present;
			return current;
		});
	}, []);

	const preview = useCallback((next: TimecodeDefinition) => {
		setHistory((current) => ({ ...current, present: next }));
	}, []);

	const endGesture = useCallback(() => {
		const start = gestureStart.current;
		gestureStart.current = null;
		if (!start) return;
		setHistory((current) =>
			equal(start, current.present)
				? current
				: {
						past: [...current.past.slice(-99), start],
						present: current.present,
						future: [],
					},
		);
	}, []);

	const undo = useCallback(() => {
		setHistory((current) => {
			const previous = current.past.at(-1);
			if (!previous) return current;
			return {
				past: current.past.slice(0, -1),
				present: previous,
				future: [current.present, ...current.future].slice(0, 100),
			};
		});
	}, []);

	const redo = useCallback(() => {
		setHistory((current) => {
			const next = current.future[0];
			if (!next) return current;
			return {
				past: [...current.past.slice(-99), current.present],
				present: next,
				future: current.future.slice(1),
			};
		});
	}, []);

	return {
		draft: history.present,
		commit,
		preview,
		beginGesture,
		endGesture,
		undo,
		redo,
		canUndo: history.past.length > 0,
		canRedo: history.future.length > 0,
	};
}

function equal(left: TimecodeDefinition, right: TimecodeDefinition): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
