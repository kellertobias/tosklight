import { createPortal } from "react-dom";
import { useEffect, useMemo, useState } from "react";
import type { PlaybackDefinition } from "../../api/types";
import {
	Button,
	DEFAULT_COLORS,
	ModalRegistration,
} from "@tosklight/ui";
import { ModalTitleBar } from "@tosklight/ui";
import { WindowScrollArea } from "@tosklight/ui/window-kit";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import {
	useCueLists,
	useDynamics,
	usePlaybackDefinitions,
	usePortableGroups,
} from "../../features/showObjects/ShowObjectsState";
import {
	cueListWriteBasis,
	useCueListTopologyWriter,
} from "../../features/playbackTopology/useCueListTopologyWriter";
import {
	InactivePlaybackDetail,
	PlaybackBehaviorTab,
	PlaybackFunctionTab,
	PlaybackLayoutTab,
	familyFromTarget,
	isSpecial,
	playbackLayoutOptions,
	type PlaybackFamily,
} from "./PlaybackConfigurationTabs";

export { playbackImageDataUrl } from "./PlaybackConfigurationTabs";

export const PLAYBACK_COLORS = DEFAULT_COLORS;

type PlaybackTab = "function" | "behavior" | "layout";

export interface PlaybackConfigurationModalProps {
	playback: PlaybackDefinition;
	page: number;
	slot: number;
	empty?: boolean;
	virtual?: boolean;
	onClose: () => void;
}

export interface PlaybackConfigurationDialogProps
	extends PlaybackConfigurationModalProps {
	fallbackButtons: number;
	save: (
		page: number,
		slot: number,
		playback: PlaybackDefinition,
	) => Promise<boolean>;
	clear: (page: number, slot: number) => Promise<boolean>;
	error?: string | null;
}

/** Shared modal body; callers choose the physical or scoped topology mutation boundary. */
export function PlaybackConfigurationDialog({
	playback,
	page,
	slot,
	empty = false,
	virtual = false,
	fallbackButtons,
	save,
	clear,
	error,
	onClose,
}: PlaybackConfigurationDialogProps) {
	useShowObjectView("group");
	useShowObjectView("cue_list");
	useShowObjectView("dynamic");
	const { groups, dynamics, cueListObjects, cueLists } = usePlaybackConfigurationObjects();
	const saveCueList = useCueListTopologyWriter();
	const [initialDraft] = useState(() =>
		normalizePlaybackTopology(playback, fallbackButtons, !virtual),
	);
	const [draft, setDraft] = useState(initialDraft);
	const initialFamily = familyFromTarget(initialDraft.target.type);
	const [family, setFamily] = useState<PlaybackFamily>(initialFamily);
	const [tab, setTab] = useState<PlaybackTab>("function");
	const [busy, setBusy] = useState(false);
	const [failure, setFailure] = useState<string | null>(null);
	const { selectedCueList, cueListBehavior, setCueListBehavior } = useCueListBehavior(draft, cueListObjects);
	useEffect(() => {
		if (failure && error && failure !== error) setFailure(error);
	}, [error, failure]);
	const [presentation, setPresentation] = useState<"label" | "icon" | "image">(
		() =>
			draft.presentation_image
				? "image"
				: draft.presentation_icon
					? "icon"
					: "label",
	);
	const selectedDynamicId =
		draft.target.type === "dynamic"
			? draft.target.assignment.dynamic_id
			: undefined;
	const selectedDynamic = selectedDynamicId
		? dynamics.find((dynamic) => dynamic.id === selectedDynamicId)
		: undefined;
	const dynamicScopeValid =
		draft.target.type !== "dynamic" ||
		selectedDynamic?.body.target_binding.type !== "targetless" ||
		(draft.target.assignment.target_scope?.type === "live_group"
			? Boolean(draft.target.assignment.target_scope.group_id)
			: (draft.target.assignment.target_scope?.targets.length ?? 0) > 0);
	const targetValid =
		family === "none" ||
		((draft.target.type !== "cue_list" || Boolean(draft.target.cue_list_id)) &&
			(draft.target.type !== "group" || Boolean(draft.target.group_id)) &&
			(draft.target.type !== "dynamic" ||
				Boolean(draft.target.assignment.dynamic_id)) &&
			dynamicScopeValid);
	const currentPayload = cleanPresentation(
		normalizePlaybackTopology(
			draft,
			draft.button_count ?? 3,
			Boolean(draft.has_fader),
		),
	);
	const initialPayload = cleanPresentation(initialDraft);
	const cueListBehaviorDirty = Boolean(
		selectedCueList &&
			cueListBehavior &&
			(cueListBehavior.auto_off_at_zero !==
				Boolean(selectedCueList.body.auto_off_at_zero) ||
				cueListBehavior.auto_off_flash_release !==
					Boolean(selectedCueList.body.auto_off_flash_release)),
	);
	const isDirty =
		family === "none"
			? !empty
			: family !== initialFamily ||
				!playbackDefinitionsEqual(currentPayload, initialPayload) ||
				cueListBehaviorDirty;
	const topology = `${draft.button_count ?? 3} button${draft.button_count === 1 ? "" : "s"} · ${draft.has_fader ? "fader" : "faderless"}`;
	const options = useMemo(() => playbackLayoutOptions(draft), [draft.target.type]);

	const apply = async () => {
		setBusy(true);
		setFailure(null);
		let succeeded =
			family === "none"
				? await clear(page, slot)
				: await save(
						page,
						slot,
						cleanPresentation(
							normalizePlaybackTopology(
								draft,
								draft.button_count ?? 3,
								Boolean(draft.has_fader),
							),
						),
					);
		if (
			succeeded &&
			selectedCueList &&
			cueListBehaviorDirty &&
			cueListBehavior
		) {
			succeeded = Boolean(
				await saveCueList(cueListWriteBasis(selectedCueList), {
					...selectedCueList.body,
					...cueListBehavior,
				}),
			);
		}
		setBusy(false);
		if (succeeded) onClose();
		else
			setFailure(
				error ??
					(family === "none"
						? "Playback could not be cleared."
						: "Playback configuration could not be saved."),
			);
	};
	const { chooseFamily, chooseSpecial } = playbackFamilyChoosers(
		draft, cueLists, groups, dynamics, setFamily, setDraft,
	);

	return <PlaybackConfigurationDialogView
		page={page} slot={slot} virtual={virtual} topology={topology}
		busy={busy} isDirty={isDirty} targetValid={targetValid}
		family={family} tab={tab} draft={draft} presentation={presentation}
		cueLists={cueLists} dynamics={dynamics} groups={groups} options={options}
		cueListBehavior={cueListBehavior} failure={failure}
		onClose={onClose} onApply={() => void apply()} onTabChange={setTab}
		onFamilyChange={chooseFamily} onSpecialChange={chooseSpecial}
		onPresentationChange={setPresentation} onDraftChange={setDraft}
		onCueListBehaviorChange={setCueListBehavior}
	/>;
}

type CueListBehavior = { auto_off_at_zero: boolean; auto_off_flash_release: boolean } | null;
type DialogViewProps = {
	page: number; slot: number; virtual: boolean; topology: string; busy: boolean;
	isDirty: boolean; targetValid: boolean; family: PlaybackFamily; tab: PlaybackTab;
	draft: PlaybackDefinition; presentation: "label" | "icon" | "image";
	cueLists: Array<{ id: string; name: string; number: number }>;
	dynamics: ReturnType<typeof useDynamics>; groups: ReturnType<typeof usePortableGroups>;
	options: ReturnType<typeof playbackLayoutOptions>; cueListBehavior: CueListBehavior;
	failure: string | null; onClose: () => void; onApply: () => void;
	onTabChange: (tab: PlaybackTab) => void; onFamilyChange: (family: PlaybackFamily) => void;
	onSpecialChange: (type: "programmer_fade" | "cue_fade" | "grand_master") => void;
	onPresentationChange: (value: "label" | "icon" | "image") => void;
	onDraftChange: (draft: PlaybackDefinition) => void;
	onCueListBehaviorChange: (value: CueListBehavior) => void;
};

function PlaybackConfigurationDialogView(props: DialogViewProps) {
	const { page, slot, virtual, topology, busy, isDirty, targetValid, family, tab,
		draft, presentation, cueLists, dynamics, groups, options, cueListBehavior,
		failure, onClose, onApply, onTabChange, onFamilyChange, onSpecialChange,
		onPresentationChange, onDraftChange, onCueListBehaviorChange } = props;
	return createPortal(
		<ModalRegistration onClose={onClose}>
			<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
				<section className="nested-modal playback-configuration-modal" role="dialog" aria-modal="true"
					aria-label="Playback Configuration" data-page={page} data-slot={slot} data-topology={topology}>
					<ModalTitleBar
						title={`Playback Configuration · ${page}.${slot}`}
						accept={{
							id: "apply",
							label: busy ? family === "none" ? "Clearing…" : "Applying…" : "Apply",
							variant: "primary",
							disabled: busy || !isDirty || !targetValid || (family !== "none" && !draft.name.trim()),
							onPress: onApply,
						}}
						onClose={onClose}
						closeLabel="Close playback configuration"
					/>
					<nav className="segmented-control playback-configuration-tabs">
						<Button className={tab === "function" ? "active" : ""} onClick={() => onTabChange("function")}>Function</Button>
						<Button className={tab === "behavior" ? "active" : ""} onClick={() => onTabChange("behavior")}>Behavior</Button>
						<Button className={tab === "layout" ? "active" : ""} onClick={() => onTabChange("layout")}>Layout</Button>
					</nav>
					<div className="playback-configuration-body">
						{tab === "function" && <PlaybackFunctionTab
							family={family} draft={draft} virtual={virtual} presentation={presentation}
							cueLists={cueLists} dynamics={dynamics} groups={groups}
							onFamilyChange={onFamilyChange} onSpecialChange={onSpecialChange}
							onPresentationChange={onPresentationChange} onDraftChange={onDraftChange}
						/>}
						{tab === "behavior" && <WindowScrollArea className="playback-tab-scroll">
							<div className="playback-tab-scroll-content">
								{family === "none" ? <InactivePlaybackDetail /> : <PlaybackBehaviorTab
									draft={draft} dynamics={dynamics} groups={groups} cueListBehavior={cueListBehavior}
									onCueListBehaviorChange={onCueListBehaviorChange} onDraftChange={onDraftChange}
								/>}
							</div>
						</WindowScrollArea>}
						{tab === "layout" && <WindowScrollArea className="playback-tab-scroll">
							<div className="playback-tab-scroll-content">
								{family === "none" ? <InactivePlaybackDetail /> : <PlaybackLayoutTab
									draft={draft} virtual={virtual} options={options} onDraftChange={onDraftChange}
								/>}
							</div>
						</WindowScrollArea>}
						{failure && <p role="alert" className="modal-error">{failure}</p>}
					</div>
				</section>
			</div>
		</ModalRegistration>,
		document.body,
	);
}

function usePlaybackConfigurationObjects() {
	const groups = usePortableGroups();
	const dynamics = useDynamics();
	const cueListObjects = useCueLists();
	const playbackDefinitions = usePlaybackDefinitions();
	const cueListNumbers = useMemo(() => {
		const numbers = new Map<string, number>();
		for (const { body } of [...playbackDefinitions].sort((left, right) => left.body.number - right.body.number)) {
			if (body.target.type === "cue_list" && !numbers.has(body.target.cue_list_id))
				numbers.set(body.target.cue_list_id, body.number);
		}
		return numbers;
	}, [playbackDefinitions]);
	const cueLists = useMemo(() => cueListObjects.map(({ body }, index) => ({
		id: body.id,
		name: body.name || body.id,
		number: cueListNumbers.get(body.id) ?? index + 1,
	})), [cueListNumbers, cueListObjects]);
	return { groups, dynamics, cueListObjects, cueLists };
}

function playbackFamilyChoosers(
	draft: PlaybackDefinition,
	cueLists: Array<{ id: string }>,
	groups: ReadonlyArray<{ id: string }>,
	dynamics: ReturnType<typeof useDynamics>,
	setFamily: (family: PlaybackFamily) => void,
	setDraft: (draft: PlaybackDefinition) => void,
) {
	const replaceTarget = (type: string) => {
		if (type !== draft.target.type) setDraft(withFunctionDefaults(
			draft, type, cueLists[0]?.id ?? "", groups[0]?.id ?? "", dynamics[0],
		));
	};
	const chooseFamily = (next: PlaybackFamily) => {
		setFamily(next);
		if (next === "none") return;
		replaceTarget(next === "special"
			? isSpecial(draft.target.type) ? draft.target.type : "programmer_fade"
			: next);
	};
	const chooseSpecial = (type: "programmer_fade" | "cue_fade" | "grand_master") => replaceTarget(type);
	return { chooseFamily, chooseSpecial };
}

function useCueListBehavior(draft: PlaybackDefinition, cueLists: ReturnType<typeof useCueLists>) {
	const selectedId = draft.target.type === "cue_list" ? draft.target.cue_list_id : undefined;
	const selectedCueList = selectedId ? cueLists.find(({ body }) => body.id === selectedId) : undefined;
	const behavior = () => selectedCueList ? {
		auto_off_at_zero: Boolean(selectedCueList.body.auto_off_at_zero),
		auto_off_flash_release: Boolean(selectedCueList.body.auto_off_flash_release),
	} : null;
	const [cueListBehavior, setCueListBehavior] = useState(behavior);
	useEffect(() => setCueListBehavior(behavior()), [selectedCueList?.id, selectedCueList?.revision]);
	return { selectedCueList, cueListBehavior, setCueListBehavior };
}

export function normalizePlaybackTopology(
	playback: PlaybackDefinition,
	fallbackButtons: number,
	fallbackFader: boolean,
): PlaybackDefinition {
	const buttonCount = Math.max(
		0,
		Math.min(3, playback.button_count ?? fallbackButtons),
	) as 0 | 1 | 2 | 3;
	const buttons = playback.buttons.map((action, index) =>
		index < buttonCount ? action : "none",
	) as PlaybackDefinition["buttons"];
	return {
		...playback,
		buttons,
		footprint: playback.footprint ?? { type: "normal" },
		button_count: buttonCount,
		has_fader: playback.has_fader ?? fallbackFader,
		color: playback.color ?? "#20c997",
		flash_release: playback.flash_release ?? "release_all",
		protect_from_swap: Boolean(playback.protect_from_swap),
	};
}

export function withFunctionDefaults(
	playback: PlaybackDefinition,
	type: string,
	cueListId: string,
	groupId: string,
	dynamic?: ReturnType<typeof useDynamics>[number],
): PlaybackDefinition {
	let target: PlaybackDefinition["target"];
	let buttons: PlaybackDefinition["buttons"];
	let fader: PlaybackDefinition["fader"];
	if (type === "cue_list") {
		target = { type, cue_list_id: cueListId };
		buttons = ["go_minus", "go", "flash"];
		fader = "master";
	} else if (type === "dynamic" && dynamic) {
		target = {
			type,
			assignment: {
				dynamic_id: dynamic.id,
				last_known_pool_number: dynamic.body.pool_number,
				embedded_fallback: dynamic.body,
				revision: 1,
				target_scope:
					dynamic.body.target_binding.type === "targetless" ? null : undefined,
				fader_mode: "size_and_master",
				priority: 0,
				activation_override: null,
				resume_policy: "follow_dynamic",
				local_speed_multiplier: { numerator: 1, denominator: 1 },
				learned_duration_millis: null,
				crossfade_non_intensity: false,
				auto_off_at_zero: false,
				auto_off_flash_release: false,
				auto_off_full_control: true,
			},
		};
		buttons = ["off", "pause", "flash"];
		fader = "master";
	} else if (type === "group") {
		target = { type, group_id: groupId };
		buttons = ["select", "select_dereferenced", "flash"];
		fader = "master";
	} else if (type === "speed_group") {
		target = { type, group: "A" };
		buttons = ["double", "half", "learn"];
		fader = "learned_percentage";
	} else if (type === "programmer_fade") {
		target = { type };
		buttons = ["double", "half", "off"];
		fader = "master";
	} else if (type === "cue_fade") {
		target = { type };
		buttons = ["double", "half", "off"];
		fader = "master";
	} else {
		target = { type: "grand_master" };
		buttons = ["blackout", "pause_dynamics", "flash"];
		fader = "master";
	}
	const footprint =
		playback.footprint?.type === "taller"
			? { type: "taller" as const, upper_button: "none" as const }
			: playback.footprint?.type === "wider"
				? { type: "wider" as const, right_buttons: buttons, right_fader: fader }
				: { type: "normal" as const };
	return normalizePlaybackTopology(
		{ ...playback, target, buttons, fader, footprint },
		playback.button_count ?? 3,
		Boolean(playback.has_fader),
	);
}

function cleanPresentation(playback: PlaybackDefinition): PlaybackDefinition {
	const presentation_icon = playback.presentation_icon?.trim() || undefined;
	const presentation_image = playback.presentation_image?.trim() || undefined;
	return {
		...playback,
		presentation_icon,
		presentation_image: presentation_icon ? undefined : presentation_image,
	};
}
function playbackDefinitionsEqual(
	left: PlaybackDefinition,
	right: PlaybackDefinition,
) {
	return JSON.stringify(left) === JSON.stringify(right);
}
