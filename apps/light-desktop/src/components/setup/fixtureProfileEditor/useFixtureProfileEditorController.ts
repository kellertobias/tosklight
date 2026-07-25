import { useCallback, useEffect, useMemo, useState } from "react";
import type { FixtureMode, FixtureProfile } from "../../../api/types";
import {
	blankMode,
	cloneProfile,
	reorder,
	validateProfile,
} from "../fixtureProfileModel";
import type { ModeEditorTab } from "./modeEditor";

export type ProfileEditorTab = "generic" | "modes";

type ControllerOptions = {
	initialProfile: FixtureProfile;
	expectedRevision: number;
	onSave: (
		profile: FixtureProfile,
		expectedRevision: number,
	) => Promise<FixtureProfile>;
	onClose: () => void;
};

function useEscapeClose({
	dialogOpen,
	modeEditorId,
	onCloseMode,
	onRequestClose,
}: {
	dialogOpen: boolean;
	modeEditorId: string | null;
	onCloseMode: () => void;
	onRequestClose: () => void;
}) {
	useEffect(() => {
		const keydown = (event: KeyboardEvent) => {
			if (event.key !== "Escape" || dialogOpen) return;
			event.preventDefault();
			event.stopPropagation();
			if (modeEditorId) onCloseMode();
			else onRequestClose();
		};
		window.addEventListener("keydown", keydown, true);
		return () => window.removeEventListener("keydown", keydown, true);
	}, [dialogOpen, modeEditorId, onCloseMode, onRequestClose]);
}

export function useFixtureProfileEditorController({
	initialProfile,
	expectedRevision,
	onSave,
	onClose,
}: ControllerOptions) {
	const [draft, setDraft] = useState(() => cloneProfile(initialProfile));
	const [tab, setTab] = useState<ProfileEditorTab>("generic");
	const [modeEditorId, setModeEditorId] = useState<string | null>(null);
	const [modeTab, setModeTab] = useState<ModeEditorTab>("heads");
	const [openSplit, setOpenSplit] = useState(
		initialProfile.modes[0]?.splits[0]?.number ?? 1,
	);
	const [lookup, setLookup] = useState(false);
	const [lookupQuery, setLookupQuery] = useState("");
	const [closeConfirm, setCloseConfirm] = useState(false);
	const [revisionConfirm, setRevisionConfirm] = useState(false);
	const [busy, setBusy] = useState(false);
	const [localErrors, setLocalErrors] = useState<string[]>([]);
	const baseline = useMemo(
		() => JSON.stringify(initialProfile),
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
	const closeMode = useCallback(() => setModeEditorId(null), []);
	useEscapeClose({
		dialogOpen: lookup || closeConfirm || revisionConfirm,
		modeEditorId,
		onCloseMode: closeMode,
		onRequestClose: requestClose,
	});

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
	const deleteMode = (id: string) => {
		if (draft.modes.length === 1) return;
		setDraft((current) => ({
			...current,
			modes: current.modes.filter((mode) => mode.id !== id),
		}));
		if (modeEditorId === id) closeMode();
	};
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
		deleteMode,
		openMode,
		closeMode,
		updateMode,
	};
}
