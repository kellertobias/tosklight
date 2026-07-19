import { useEffect, useRef, useState } from "react";
import type { ControlDesk, PlaybackSurfaceLayout } from "../../../api/types";
import { defaultDeskPlaybackLayout } from "../screenConfiguration";

export interface ScreenUndoHandle {
	current: (() => void) | null;
}

interface DeskSnapshot {
	desk: ControlDesk;
	regularNumberShortcuts: boolean;
}

interface DefaultScreenDraftOptions {
	desk: ControlDesk | null | undefined;
	regularNumberShortcuts: boolean;
	onKeyboardShortcuts: (enabled: boolean) => void;
	onPlaybackLayout: (layout: PlaybackSurfaceLayout) => void;
	onPersistDesk: (desk: ControlDesk) => Promise<unknown>;
	undoRef?: ScreenUndoHandle;
	onUndoAvailabilityChange?: (available: boolean) => void;
}

export function useDefaultScreenDraft({
	desk,
	regularNumberShortcuts,
	onKeyboardShortcuts,
	onPlaybackLayout,
	onPersistDesk,
	undoRef,
	onUndoAvailabilityChange,
}: DefaultScreenDraftOptions) {
	const [draft, setDraft] = useState<ControlDesk | null>(desk ?? null);
	const [playbackLayout, setPlaybackLayout] =
		useState<PlaybackSurfaceLayout | null>(() =>
			desk ? defaultDeskPlaybackLayout(desk) : null,
		);
	const [undoHistory, setUndoHistory] = useState<DeskSnapshot[]>([]);
	const draftRef = useRef<ControlDesk | null>(desk ?? null);
	const saveQueue = useRef(Promise.resolve());
	const pendingSaves = useRef(0);
	const textEditRecorded = useRef({ name: false, osc_alias: false });

	useEffect(() => {
		if (!desk || pendingSaves.current > 0) return;
		draftRef.current = desk;
		setDraft(desk);
		const layout = defaultDeskPlaybackLayout(desk);
		setPlaybackLayout(layout);
		onPlaybackLayout(layout);
	}, [desk, onPlaybackLayout]);

	const snapshot = (): DeskSnapshot | null => {
		const current = draftRef.current ?? desk;
		return current
			? { desk: structuredClone(current), regularNumberShortcuts }
			: null;
	};
	const rememberCurrent = () => {
		const current = snapshot();
		if (current) setUndoHistory((history) => [...history, current]);
	};
	const applyDesk = (next: ControlDesk, remember: boolean) => {
		const current = draftRef.current ?? desk;
		if (!current || JSON.stringify(current) === JSON.stringify(next))
			return false;
		if (remember) rememberCurrent();
		draftRef.current = next;
		setDraft(next);
		const layout = defaultDeskPlaybackLayout(next);
		setPlaybackLayout(layout);
		onPlaybackLayout(layout);
		pendingSaves.current += 1;
		saveQueue.current = saveQueue.current
			.then(() => onPersistDesk(next))
			.then(() => undefined)
			.finally(() => {
				pendingSaves.current -= 1;
			});
		return true;
	};
	const updateDesk = (changes: Partial<ControlDesk>, remember = true) => {
		const current = draftRef.current ?? desk;
		return current ? applyDesk({ ...current, ...changes }, remember) : false;
	};
	const updateText = (field: "name" | "osc_alias", value: string) => {
		const changed = updateDesk(
			{ [field]: value },
			!textEditRecorded.current[field],
		);
		if (changed) textEditRecorded.current[field] = true;
	};
	const beginTextEdit = (field: "name" | "osc_alias") => {
		textEditRecorded.current[field] = false;
	};
	const endTextEdit = beginTextEdit;
	const updateKeyboardShortcuts = (enabled: boolean) => {
		if (enabled === regularNumberShortcuts) return;
		rememberCurrent();
		onKeyboardShortcuts(enabled);
	};
	const undo = () => {
		const previous = undoHistory.at(-1);
		if (!previous) return;
		setUndoHistory((history) => history.slice(0, -1));
		applyDesk(previous.desk, false);
		if (previous.regularNumberShortcuts !== regularNumberShortcuts) {
			onKeyboardShortcuts(previous.regularNumberShortcuts);
		}
		textEditRecorded.current = { name: false, osc_alias: false };
	};
	if (undoRef) undoRef.current = undo;

	useEffect(() => {
		onUndoAvailabilityChange?.(undoHistory.length > 0);
	}, [onUndoAvailabilityChange, undoHistory.length]);
	useEffect(
		() => () => onUndoAvailabilityChange?.(false),
		[onUndoAvailabilityChange],
	);

	return {
		beginTextEdit,
		draft,
		endTextEdit,
		playbackLayout,
		updateDesk,
		updateKeyboardShortcuts,
		updateText,
	};
}
