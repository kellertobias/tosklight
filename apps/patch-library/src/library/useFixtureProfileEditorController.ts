import { useCallback, useMemo, useState } from "react";
import type { FixtureMode, FixtureProfile } from "../wire";
import {
	blankMode,
	cloneProfile,
	liftMotionAttributes,
	reorder,
	validateProfile,
} from "../sheet/fixtureProfileModel";
import type { ModeEditorTab } from "./modeEditor";

export type ProfileEditorTab =
	| "identity"
	| "simulation"
	| "geometry"
	| "modes";

type ControllerOptions = {
	initialProfile: FixtureProfile;
	expectedRevision: number;
	onSave: (
		profile: FixtureProfile,
		expectedRevision: number,
	) => Promise<FixtureProfile>;
	onClose: () => void;
};

export function useFixtureProfileEditorController({
	initialProfile,
	expectedRevision,
	onSave,
	onClose,
}: ControllerOptions) {
	// Opened the way the desk reads it, with moving parts bound per mode, so an untouched legacy
	// profile is not reported as changed.
	const [draft, setDraft] = useState(() =>
		liftMotionAttributes(cloneProfile(initialProfile)),
	);
	const [tab, setTab] = useState<ProfileEditorTab>("identity");
	const [modeEditorId, setModeEditorId] = useState<string | null>(null);
	const [modeTab, setModeTab] = useState<ModeEditorTab>("heads");
	const [openSplit, setOpenSplit] = useState(
		initialProfile.modes[0]?.splits[0]?.number ?? 1,
	);
	const [lookup, setLookup] = useState(false);
	const [lookupQuery, setLookupQuery] = useState("");
	const [closeConfirm, setCloseConfirm] = useState(false);
	const [revisionConfirm, setRevisionConfirm] = useState(false);
	/** The mode waiting for the operator to confirm its removal. */
	const [modeDeleteId, setModeDeleteId] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);
	const [localErrors, setLocalErrors] = useState<string[]>([]);
	const baseline = useMemo(
		() => JSON.stringify(liftMotionAttributes(cloneProfile(initialProfile))),
		[initialProfile],
	);
	const dirty = JSON.stringify(draft) !== baseline;
	const editedMode = modeEditorId
		? (draft.modes.find((mode) => mode.id === modeEditorId) ?? null)
		: null;

	const requestClose = useCallback(
		() => (dirty ? setCloseConfirm(true) : onClose()),
		[dirty, onClose],
	);
	// Escape is the modal stack's: it closes whichever window is on top — a channel's mapping, the
	// mode, a confirmation — and only then the editor. A handler of the editor's own ran before the
	// stack and closed the mode from under a window still open inside it.
	const closeMode = useCallback(() => setModeEditorId(null), []);

	const updateMode = (next: FixtureMode) =>
		setDraft((current) => ({
			...current,
			modes: current.modes.map((mode) => (mode.id === next.id ? next : mode)),
		}));
	const saveNow = async () => {
		setBusy(true);
		setRevisionConfirm(false);
		try {
			const saved = await onSave(draft, expectedRevision);
			if (saved) onClose();
		} catch (reason) {
			const message =
				reason instanceof Error ? reason.message : String(reason ?? "");
			setLocalErrors([
				message.trim() ||
					"The fixture profile could not be saved. Check the server error and try again.",
			]);
		} finally {
			setBusy(false);
		}
	};
	const requestSave = () => {
		const errors = validateProfile(draft);
		setLocalErrors(errors);
		if (errors.length) return;
		if (initialProfile.revision > 0) setRevisionConfirm(true);
		else void saveNow();
	};
	const addMode = () => {
		const mode = blankMode(`Mode ${draft.modes.length + 1}`);
		setDraft((current) => ({ ...current, modes: [...current.modes, mode] }));
		setModeTab("heads");
		setOpenSplit(1);
	};
	const moveMode = (sourceId: string, targetId: string) =>
		setDraft((current) => {
			const from = current.modes.findIndex((mode) => mode.id === sourceId);
			const to = current.modes.findIndex((mode) => mode.id === targetId);
			return from < 0 || to < 0 || from === to
				? current
				: { ...current, modes: reorder(current.modes, from, to) };
		});
	/** A mode carries every channel and function of it, so removing one is asked about first. */
	const requestDeleteMode = (id: string) => {
		if (draft.modes.length === 1) return;
		setModeDeleteId(id);
	};
	const deleteMode = (id: string) => {
		setModeDeleteId(null);
		if (draft.modes.length === 1) return;
		setDraft((current) => ({
			...current,
			modes: current.modes.filter((mode) => mode.id !== id),
		}));
		if (modeEditorId === id) closeMode();
	};
	const modePendingDelete = modeDeleteId
		? (draft.modes.find((mode) => mode.id === modeDeleteId) ?? null)
		: null;
	const openMode = (mode: FixtureMode) => {
		setModeEditorId(mode.id);
		setOpenSplit(mode.splits[0]?.number ?? 1);
		setModeTab("channels");
	};

	return {
		draft,
		setDraft,
		tab,
		setTab,
		modeTab,
		setModeTab,
		openSplit,
		setOpenSplit,
		lookup,
		setLookup,
		lookupQuery,
		setLookupQuery,
		closeConfirm,
		setCloseConfirm,
		revisionConfirm,
		setRevisionConfirm,
		busy,
		localErrors,
		editedMode,
		requestClose,
		requestSave,
		saveNow,
		addMode,
		moveMode,
		requestDeleteMode,
		deleteMode,
		modePendingDelete,
		cancelDeleteMode: () => setModeDeleteId(null),
		openMode,
		closeMode,
		updateMode,
	};
}
