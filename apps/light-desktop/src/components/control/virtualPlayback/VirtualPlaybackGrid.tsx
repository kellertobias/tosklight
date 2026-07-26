import { useEffect, useRef } from "react";
import {
	VirtualPlaybackGridView,
	type VirtualPlaybackBoxViewModel,
} from "@tosklight/ui/playback";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
} from "../../../api/types";
import type { PlaybackProjection } from "../../../features/playbackRuntime/contracts";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import {
	cueUpdateTarget,
	requestUpdateTarget,
} from "../updateWorkflow";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../../features/poolPresentation/poolPresentation";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";

export const MAX_PLAYBACK_SLOT = 127;

interface VirtualPlaybackGridProps {
	pageNumber: number;
	page: PlaybackPage | undefined;
	rows: number;
	columns: number;
	playbacks: ReadonlyMap<number, PlaybackDefinition>;
	cueLists: ReadonlyMap<string, CueList>;
	runtimes: ReadonlyMap<number, PlaybackProjection | undefined>;
	runtimeActions: PlaybackRuntimeActions | null;
	zones: readonly VirtualPlaybackZone[];
	selectedSlots: readonly number[];
	configurationArmed: boolean;
	assignmentPending: boolean;
	assignmentTarget: number | null;
	updateArmed: boolean;
	shiftArmed: boolean;
	onConfigure(playback: PlaybackDefinition | null, slot: number): void;
	onAssign(slot: number): void;
	onToggleZone(slot: number): void;
	paneId?: string;
}

export function VirtualPlaybackGrid(props: VirtualPlaybackGridProps) {
	const heldActions = useHeldPlaybackActions(props.runtimeActions);
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const surfaceKey = poolSurfaceKey(showId, "cuelist", props.paneId);
	const boxes = Array.from(
		{ length: props.rows * props.columns },
		(_, position) =>
			boxViewModel(
				props,
				position + 1,
				position,
				poolPresentation,
				showId,
				surfaceKey,
			),
	);
	const playbackAt = (slot: number) => {
		const number = props.page?.slots[String(slot)];
		return number == null ? null : (props.playbacks.get(number) ?? null);
	};
	return (
		<VirtualPlaybackGridView
			page={props.pageNumber}
			rows={props.rows}
			columns={props.columns}
			boxes={boxes}
			callbacks={{
				onClick: (slot, _position, interaction) => {
					if (!props.shiftArmed && !interaction.shiftKey) return false;
					props.onToggleZone(slot);
					return true;
				},
				onAction: (slot) => {
					const playback = playbackAt(slot);
					const action = playback?.buttons[0] ?? "none";
					if (playback && action !== "none")
						void props.runtimeActions?.poolPlaybackAction(
							playback.number,
							"button",
							{ button: 1, pressed: true, surface: "virtual" },
						);
				},
				onActionPress: (slot) => {
					const playback = playbackAt(slot);
					if (playback) heldActions.press(slot, playback);
				},
				onActionRelease: (slot) => heldActions.release(slot),
				onConfigure: (slot) => props.onConfigure(playbackAt(slot), slot),
				onAssign: props.onAssign,
				onUpdate: (slot) => requestPlaybackUpdate(props, slot),
				onZoneSelection: props.onToggleZone,
			}}
		/>
	);
}

function boxViewModel(
	props: VirtualPlaybackGridProps,
	slot: number,
	position: number,
	poolPresentation: ReturnType<typeof usePoolPresentationConfiguration>,
	showId: string,
	surfaceKey: string,
): VirtualPlaybackBoxViewModel {
	const available = validPlaybackSlot(slot);
	const number = available ? props.page?.slots[String(slot)] : undefined;
	const playback = number == null ? null : (props.playbacks.get(number) ?? null);
	const projection = playback
		? props.runtimes.get(playback.number)
		: undefined;
	const runtime = projection?.target === "cue_list" ? projection.runtime : null;
	const cueList =
		playback?.target.type === "cue_list"
			? props.cueLists.get(playback.target.cue_list_id)
			: undefined;
	const currentCue = currentCueFrom(cueList, runtime?.current ?? null);
	const action = playback?.buttons[0] ?? "none";
	const containingZones = props.zones.filter((zone) =>
		zone.slots.includes(slot),
	);
	const representedType =
		playback?.target.type === "group"
			? "group"
			: playback?.target.type === "cue_list"
				? "cuelist"
				: null;
	const representedId =
		playback?.target.type === "group"
			? playback.target.group_id
			: playback?.target.type === "cue_list"
				? playback.target.cue_list_id
				: undefined;
	const presentation = representedType
		? resolveConfiguredPoolPresentation(poolPresentation, {
				showId,
				surfaceKey,
				objectType: representedType,
				itemColorKey: representedId,
				itemColor: playback?.color,
				states: [
					...(!available || !playback ? (["empty"] as const) : []),
					...(!available ? (["disabled"] as const) : []),
					...(runtime?.enabled === true ? (["active"] as const) : []),
					...(props.assignmentPending ? (["record-target"] as const) : []),
					...(props.assignmentPending ? (["store-target"] as const) : []),
					...(props.updateArmed ? (["update-target"] as const) : []),
				],
			})
		: undefined;
	return {
		slot,
		position,
		availability: !available
			? "unavailable"
			: playback
				? "assigned"
				: "empty",
		label: playback?.name,
		icon: playback?.presentation_icon,
		color: playback?.color,
		backgroundImage: playback?.presentation_image,
		actionLabel:
			playback && action !== "none"
				? action.replaceAll("_", " ").toUpperCase()
				: undefined,
		heldAction: action === "flash" || action === "swap",
		running: runtime?.enabled === true,
		currentCue:
			runtime?.cue_index == null
				? undefined
				: `Cue ${currentCue?.number ?? runtime.cue_index + 1}`,
		configurationTarget: props.configurationArmed,
		assignmentTarget: props.assignmentPending,
		updateTarget: props.updateArmed,
		exclusionMember: containingZones.length > 0,
		exclusionZones: containingZones.map((zone) => zone.name),
		exclusionSelected: props.selectedSlots.includes(slot),
		selectingExclusionZone: props.shiftArmed,
		poolPresentation: presentation,
	};
}

function requestPlaybackUpdate(
	props: VirtualPlaybackGridProps,
	slot: number,
) {
	const number = props.page?.slots[String(slot)];
	const playback = number == null ? null : (props.playbacks.get(number) ?? null);
	if (!playback || playback.target.type !== "cue_list") return;
	const projection = props.runtimes.get(playback.number);
	const runtime = projection?.target === "cue_list" ? projection.runtime : null;
	const cueList = props.cueLists.get(playback.target.cue_list_id);
	const currentCue = currentCueFrom(cueList, runtime?.current ?? null);
	requestUpdateTarget(
		cueUpdateTarget(
			playback.target.cue_list_id,
			playback.number,
			currentCue ? { id: currentCue.id, number: currentCue.number } : null,
		),
	);
}

function useHeldPlaybackActions(
	actions: PlaybackRuntimeActions | null,
) {
	const requests = useRef(new Map<number, HeldPlaybackRequest>());
	const releaseSlot = (slot: number) => {
		const active = requests.current.get(slot);
		requests.current.delete(slot);
		if (!active) return;
		void active.pending
			.catch(() => null)
			.then(() => sendHeld(active, false))
			.catch(() => undefined);
	};
	useEffect(
		() => () => {
			for (const slot of [...requests.current.keys()]) releaseSlot(slot);
		},
		[],
	);
	return {
		press(slot: number, playback: PlaybackDefinition) {
			if (!actions || requests.current.has(slot)) return;
		const active: HeldPlaybackRequest = {
			number: playback.number,
			actions,
			pending: Promise.resolve(null),
		};
		active.pending = sendHeld(active, true);
			requests.current.set(slot, active);
		},
		release(slot: number) {
			releaseSlot(slot);
		},
	};
}

interface HeldPlaybackRequest {
	number: number;
	actions: PlaybackRuntimeActions;
	pending: Promise<unknown>;
}

function sendHeld(
	request: { number: number; actions: PlaybackRuntimeActions },
	pressed: boolean,
) {
	return request.actions.poolPlaybackAction(request.number, "button", {
		button: 1,
		pressed,
		surface: "virtual",
	});
}

export function validPlaybackSlot(slot: number) {
	return Number.isSafeInteger(slot) && slot >= 1 && slot <= MAX_PLAYBACK_SLOT;
}

function currentCueFrom(
	cueList: Pick<CueList, "cues"> | undefined,
	reference: { id: string; number: number } | null,
) {
	if (!cueList || !reference) return null;
	return cueList.cues.find(
		(cue): cue is typeof cue & { id: string } =>
			Boolean(cue.id) &&
			(cue.id === reference.id || cue.number === reference.number),
	);
}
