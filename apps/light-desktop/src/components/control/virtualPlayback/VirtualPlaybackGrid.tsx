import {
	type VirtualPlaybackBoxViewModel,
	type VirtualPlaybackExclusionFence,
	VirtualPlaybackGridView,
} from "@tosklight/ui/playback";
import { useEffect, useRef } from "react";
import type {
	CueList,
	PlaybackDefinition,
	PlaybackPage,
} from "../../../api/types";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";
import type { PlaybackRuntimeActions } from "../../../features/playbackRuntime/actionWriter";
import type { PlaybackProjection } from "../../../features/playbackRuntime/contracts";
import {
	identityKey,
	virtualPlaybackIdentity,
} from "../../../features/playbackRuntime/contracts";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../../features/poolPresentation/poolPresentation";
import type { VirtualPlaybackZone } from "../../../features/virtualPlaybackZones/contracts";
import { cueUpdateTarget, requestUpdateTarget } from "../updateWorkflow";

export const MAX_VIRTUAL_PLAYBACK_CELLS = 8_998;

interface VirtualPlaybackGridProps {
	pageNumber: number;
	page: PlaybackPage | undefined;
	rows: number;
	columns: number;
	playbacks: ReadonlyMap<number, PlaybackDefinition>;
	cueLists: ReadonlyMap<string, CueList>;
	runtimes: ReadonlyMap<string, PlaybackProjection | undefined>;
	runtimeActions: PlaybackRuntimeActions | null;
	zones: readonly VirtualPlaybackZone[];
	selectedSlots: readonly number[];
	configurationArmed: boolean;
	updateArmed: boolean;
	shiftArmed: boolean;
	onConfigure(playback: PlaybackDefinition | null, slot: number): void;
	onToggleZone(slot: number): void;
	paneId?: string;
}

export function VirtualPlaybackGrid(props: VirtualPlaybackGridProps) {
	const heldActions = useHeldPlaybackActions(props.runtimeActions);
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const surfaceKey = poolSurfaceKey(showId, "cuelist", props.paneId);
	const playbackAt = (slot: number) => {
		const number = virtualPlaybackNumber(slot);
		return props.page?.virtual_playbacks?.[String(number)] ?? null;
	};
	return (
		<VirtualPlaybackGridView
			page={props.pageNumber}
			rows={props.rows}
			columns={props.columns}
			boxAt={(position) =>
				boxViewModel(
					props,
					position + 1,
					position,
					poolPresentation,
					showId,
					surfaceKey,
				)
			}
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
						void props.runtimeActions?.virtualPlaybackAction(
							props.pageNumber,
							playback.number,
							"button",
							{ button: 1, pressed: true, surface: "virtual" },
						);
				},
				onActionPress: (slot) => {
					const playback = playbackAt(slot);
					if (playback) heldActions.press(slot, props.pageNumber, playback);
				},
				onActionRelease: (slot) => heldActions.release(slot),
				onConfigure: (slot) => props.onConfigure(playbackAt(slot), slot),
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
	const number = virtualPlaybackNumber(slot);
	const playback = available
		? (props.page?.virtual_playbacks?.[String(number)] ?? null)
		: null;
	const projection = playback
		? props.runtimes.get(
				identityKey(virtualPlaybackIdentity(props.pageNumber, playback.number)),
			)
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
	const exclusionFence = exclusionFenceForSlot(
		props.zones,
		slot,
		props.columns,
		props.rows * props.columns,
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
					...(props.updateArmed ? (["update-target"] as const) : []),
				],
			})
		: undefined;
	return {
		slot,
		position,
		availability: !available ? "unavailable" : playback ? "assigned" : "empty",
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
		updateTarget: props.updateArmed,
		exclusionMember: containingZones.length > 0,
		exclusionZones: containingZones.map((zone) => zone.name),
		exclusionFence,
		exclusionSelected: props.selectedSlots.includes(slot),
		selectingExclusionZone: props.shiftArmed,
		poolPresentation: presentation,
	};
}

export function exclusionFenceForSlot(
	zones: readonly Pick<VirtualPlaybackZone, "slots">[],
	slot: number,
	columns: number,
	cellCount: number,
): VirtualPlaybackExclusionFence | undefined {
	const containingZones = zones.filter((zone) => zone.slots.includes(slot));
	if (containingZones.length === 0 || columns < 1 || cellCount < 1)
		return undefined;
	const column = (slot - 1) % columns;
	const sharesZoneWith = (neighbor: number) =>
		neighbor >= 1 &&
		neighbor <= cellCount &&
		containingZones.some((zone) => zone.slots.includes(neighbor));
	return {
		top: slot <= columns || !sharesZoneWith(slot - columns),
		right: column === columns - 1 || !sharesZoneWith(slot + 1),
		bottom: slot + columns > cellCount || !sharesZoneWith(slot + columns),
		left: column === 0 || !sharesZoneWith(slot - 1),
	};
}

function requestPlaybackUpdate(props: VirtualPlaybackGridProps, slot: number) {
	const number = virtualPlaybackNumber(slot);
	const playback = props.page?.virtual_playbacks?.[String(number)] ?? null;
	if (!playback || playback.target.type !== "cue_list") return;
	const projection = props.runtimes.get(
		identityKey(virtualPlaybackIdentity(props.pageNumber, playback.number)),
	);
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

function useHeldPlaybackActions(actions: PlaybackRuntimeActions | null) {
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
		press(slot: number, page: number, playback: PlaybackDefinition) {
			if (!actions || requests.current.has(slot)) return;
			const active: HeldPlaybackRequest = {
				page,
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
	page: number;
	number: number;
	actions: PlaybackRuntimeActions;
	pending: Promise<unknown>;
}

function sendHeld(
	request: { page: number; number: number; actions: PlaybackRuntimeActions },
	pressed: boolean,
) {
	return request.actions.virtualPlaybackAction(
		request.page,
		request.number,
		"button",
		{
			button: 1,
			pressed,
			surface: "virtual",
		},
	);
}

export function validPlaybackSlot(slot: number) {
	return (
		Number.isSafeInteger(slot) &&
		slot >= 1 &&
		slot <= MAX_VIRTUAL_PLAYBACK_CELLS
	);
}

export function virtualPlaybackNumber(slot: number) {
	return 1_000 + slot;
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
