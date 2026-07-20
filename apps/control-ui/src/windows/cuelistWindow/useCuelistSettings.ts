import { useEffect, useRef, useState } from "react";
import type { CueList, VersionedObject } from "../../api/types";
import {
	cueListWriteBasis,
	type SaveCueListTopology,
} from "../../features/playbackTopology/useCueListTopologyWriter";

export interface CuelistSettingsProps {
	object: VersionedObject<CueList>;
	speedGroupsBpm: number[];
	close: () => void;
	save: SaveCueListTopology;
}

function legacyChaserXfadePercent(
	cueList: CueList,
	speedGroupsBpm: number[],
): number {
	const groupIndex = cueList.speed_group
		? cueList.speed_group.charCodeAt(0) - 65
		: -1;
	const stepMillis =
		groupIndex >= 0
			? Math.round(
					60_000 /
						Math.max(0.1, speedGroupsBpm[groupIndex] ?? 120) /
						Math.max(0.01, cueList.speed_multiplier ?? 1),
				)
			: (cueList.chaser_step_millis ?? 1_000);
	return Math.min(
		100,
		Math.max(
			0,
			Math.round(
				((cueList.chaser_xfade_millis ?? 0) / Math.max(1, stepMillis)) * 100,
			),
		),
	);
}

function initialSettingsDraft(object: CueList, speedGroupsBpm: number[]) {
	return {
		...object,
		intensity_priority_mode: object.intensity_priority_mode ?? "htp",
		wrap_mode: object.wrap_mode ?? (object.looped ? "tracking" : "off"),
		restart_mode: object.restart_mode ?? "first_cue",
		force_cue_timing: object.force_cue_timing ?? false,
		disable_cue_timing: object.disable_cue_timing ?? false,
		chaser_xfade_millis: object.chaser_xfade_millis ?? 0,
		chaser_xfade_percent:
			object.chaser_xfade_percent ??
			legacyChaserXfadePercent(object, speedGroupsBpm),
		speed_multiplier: object.speed_multiplier ?? 1,
	} satisfies CueList;
}

export function useCuelistSettings({
	object,
	speedGroupsBpm,
	close,
	save,
}: CuelistSettingsProps) {
	const [draft, setDraft] = useState<CueList>(() =>
		initialSettingsDraft(object.body, speedGroupsBpm),
	);
	const writeBasis = useRef(cueListWriteBasis(object)).current;
	const draftRef = useRef(draft);
	const priorityInputRef = useRef<HTMLInputElement>(null);
	const [renumberOpen, setRenumberOpen] = useState(false);
	const [startCue, setStartCue] = useState("");
	const [settingsError, setSettingsError] = useState("");
	const [renumberError, setRenumberError] = useState("");
	const [closeConfirm, setCloseConfirm] = useState(false);
	const [modeMenuOpen, setModeMenuOpen] = useState(false);
	const initialDraft = useRef(JSON.stringify(draft));
	const replaceDraft = (next: CueList) => {
		draftRef.current = next;
		setDraft(next);
	};
	const update = <K extends keyof CueList>(key: K, value: CueList[K]) =>
		replaceDraft({ ...draftRef.current, [key]: value });
	const requestClose = () => {
		const changed =
			JSON.stringify(draftRef.current) !== initialDraft.current ||
			String(priorityInputRef.current?.value ?? object.body.priority) !==
				String(object.body.priority);
		if (changed) setCloseConfirm(true);
		else close();
	};
	const submit = async () => {
		setSettingsError("");
		const priority = Number(
			priorityInputRef.current?.value ?? object.body.priority,
		);
		if (
			!Number.isInteger(priority) ||
			priority < -32_768 ||
			priority > 32_767
		) {
			setSettingsError(
				"Numeric priority must be a whole number from -32768 to 32767.",
			);
			return;
		}
		const next = { ...draftRef.current, priority };
		if (
			!Number.isInteger(next.chaser_xfade_percent) ||
			(next.chaser_xfade_percent ?? 0) < 0 ||
			(next.chaser_xfade_percent ?? 0) > 100
		) {
			setSettingsError(
				"Chaser X-fade must be a whole percentage from 0% to 100%.",
			);
			return;
		}
		if (
			!Number.isFinite(next.speed_multiplier) ||
			(next.speed_multiplier ?? 0) < 0.01 ||
			(next.speed_multiplier ?? 0) > 100
		) {
			setSettingsError("Speed multiplier must be from 0.01× to 100×.");
			return;
		}
		next.chaser_xfade_millis = 0;
		if (await save(writeBasis, next)) close();
		else
			setSettingsError(
				"Unable to save Cuelist settings. Check the values or refresh after a revision conflict.",
			);
	};
	const renumber = async () => {
		const start = startCue.trim() === "" ? 1 : Number(startCue);
		if (
			!Number.isSafeInteger(start) ||
			start <= 0 ||
			start + object.body.cues.length - 1 > Number.MAX_SAFE_INTEGER
		) {
			setRenumberError(
				"Start Cue must be a positive whole number whose resulting Cue numbers are safe integers.",
			);
			return;
		}
		const next = {
			...draftRef.current,
			priority: Number(priorityInputRef.current?.value ?? object.body.priority),
			cues: object.body.cues.map((cue, index) => ({
				...cue,
				number: start + index,
			})),
		};
		setRenumberError("");
		if (await save(writeBasis, next)) {
			setRenumberOpen(false);
			close();
		} else
			setRenumberError(
				"Renumbering was not applied. Refresh after a revision conflict and try again.",
			);
	};

	useEffect(() => {
		if (!renumberOpen && !modeMenuOpen) return;
		const closeOnEscape = (event: KeyboardEvent) => {
			if (event.key !== "Escape") return;
			event.preventDefault();
			if (renumberOpen) {
				setRenumberOpen(false);
				setRenumberError("");
			} else setModeMenuOpen(false);
		};
		window.addEventListener("keydown", closeOnEscape, true);
		return () => window.removeEventListener("keydown", closeOnEscape, true);
	}, [modeMenuOpen, renumberOpen]);

	return {
		draft,
		priorityInputRef,
		update,
		replaceDraft,
		requestClose,
		submit,
		renumber,
		renumberOpen,
		setRenumberOpen,
		startCue,
		setStartCue,
		settingsError,
		renumberError,
		setRenumberError,
		closeConfirm,
		setCloseConfirm,
		modeMenuOpen,
		setModeMenuOpen,
	};
}

export type CuelistSettingsController = ReturnType<typeof useCuelistSettings>;
