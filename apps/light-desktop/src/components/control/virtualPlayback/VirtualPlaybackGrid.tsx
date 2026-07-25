import type {
	CSSProperties,
	PointerEvent as ReactPointerEvent,
} from "react";
import { useEffect, useRef } from "react";
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
import { Button } from "../../common";

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
}

export function VirtualPlaybackGrid(props: VirtualPlaybackGridProps) {
	return (
		<div
			className="virtual-playback-grid"
			style={{
				gridTemplateColumns: `repeat(${props.columns}, minmax(0, 1fr))`,
				gridTemplateRows: `repeat(${props.rows}, minmax(0, 1fr))`,
			}}
		>
			{Array.from({ length: props.rows * props.columns }, (_, index) => (
				<VirtualPlaybackCell key={index} {...props} slot={index + 1} />
			))}
		</div>
	);
}

function VirtualPlaybackCell(
	props: VirtualPlaybackGridProps & { slot: number },
) {
	const available = validPlaybackSlot(props.slot);
	const number = available ? props.page?.slots[String(props.slot)] : undefined;
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
	const held = action === "flash" || action === "swap";
	const selectedForZone = props.selectedSlots.includes(props.slot);
	const invokeHeld = useHeldPlaybackAction(
		playback,
		held,
		props.runtimeActions,
	);
	const containingZones = props.zones.filter((zone) =>
		zone.slots.includes(props.slot),
	);
	const intercept = (event: ReactPointerEvent<HTMLButtonElement>) => {
		if (!available) return true;
		if (props.updateArmed || props.shiftArmed || event.shiftKey) {
			event.preventDefault();
			event.stopPropagation();
			return true;
		}
		if (!props.configurationArmed) return false;
		event.preventDefault();
		event.stopPropagation();
		props.onConfigure(playback, props.slot);
		return true;
	};
	const requestUpdate = () => {
		if (!playback || playback.target.type !== "cue_list") return;
		requestUpdateTarget(
			cueUpdateTarget(
				playback.target.cue_list_id,
				playback.number,
				currentCue
					? { id: currentCue.id, number: currentCue.number }
					: null,
			),
		);
	};
	return (
		<Button
			disabled={!available}
			aria-label={cellLabel(props.pageNumber, props.slot, available, playback)}
			aria-pressed={selectedForZone}
			data-exclusion-zones={containingZones
				.map((zone) => zone.name)
				.join(", ")}
			className={cellClassName({
				available,
				playback,
				running: runtime?.enabled === true,
				configurationArmed: props.configurationArmed,
				assignmentPending: props.assignmentPending,
				selectedForZone,
				containingZones,
				updateArmed: props.updateArmed,
			})}
			style={playbackStyle(playback)}
			onPointerDown={(event) => {
				if (intercept(event)) return;
				if (props.assignmentPending) {
					event.preventDefault();
					return;
				}
				if (held) {
					event.currentTarget.setPointerCapture?.(event.pointerId);
					invokeHeld(true);
				}
			}}
			onPointerUp={() => invokeHeld(false)}
			onPointerCancel={() => invokeHeld(false)}
			onLostPointerCapture={() => invokeHeld(false)}
			onClick={(event) => {
				if (!available) return;
				if (props.updateArmed) {
					event.preventDefault();
					requestUpdate();
					return;
				}
				if (props.shiftArmed || event.shiftKey) {
					event.preventDefault();
					props.onToggleZone(props.slot);
					return;
				}
				if (props.configurationArmed) {
					event.preventDefault();
					props.onConfigure(playback, props.slot);
					return;
				}
				if (props.assignmentPending) {
					props.onAssign(props.slot);
					return;
				}
				if (playback && !held && action !== "none")
					void props.runtimeActions?.poolPlaybackAction(
						playback.number,
						"button",
						{ button: 1, pressed: true, surface: "virtual" },
					);
			}}
		>
			<span>{playback?.presentation_icon ?? props.slot}</span>
			<b>{playback?.name ?? (available ? "Empty" : "Unavailable")}</b>
			<small>
				{cellDetail({
					available,
					selectedForZone,
					assignmentPending: props.assignmentPending,
					assignmentTarget: props.assignmentTarget,
					configurationArmed: props.configurationArmed,
					containingZones,
					playback,
					action,
					cueIndex: runtime?.cue_index,
				})}
			</small>
		</Button>
	);
}

function useHeldPlaybackAction(
	playback: PlaybackDefinition | null,
	held: boolean,
	actions: PlaybackRuntimeActions | null,
) {
	const request = useRef<HeldPlaybackRequest | null>(null);
	const release = () => {
		const active = request.current;
		request.current = null;
		if (!active) return;
		void active.pending
			.catch(() => null)
			.then(() => sendHeld(active, false))
			.catch(() => undefined);
	};
	useEffect(() => release, []);
	return (pressed: boolean) => {
		if (!pressed) return release();
		if (!playback || !held || !actions || request.current) return;
		const active: HeldPlaybackRequest = {
			number: playback.number,
			actions,
			pending: Promise.resolve(null),
		};
		active.pending = sendHeld(active, true);
		request.current = active;
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

function cellLabel(
	page: number,
	slot: number,
	available: boolean,
	playback: PlaybackDefinition | null,
) {
	if (!available) return `Virtual playback page ${page} cell ${slot} unavailable`;
	return `Virtual playback page ${page} cell ${slot}${playback ? ` ${playback.name}` : " empty"}`;
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

function playbackStyle(playback: PlaybackDefinition | null) {
	if (!playback) return undefined;
	const background = playback.presentation_image
		? `linear-gradient(#08101488,#081014cc),url(${JSON.stringify(playback.presentation_image)})`
		: undefined;
	return {
		"--playback-color": playback.color ?? "#20c997",
		backgroundImage: background,
	} as CSSProperties;
}

function cellClassName(input: {
	available: boolean;
	playback: PlaybackDefinition | null;
	running: boolean;
	configurationArmed: boolean;
	assignmentPending: boolean;
	selectedForZone: boolean;
	containingZones: readonly VirtualPlaybackZone[];
	updateArmed: boolean;
}) {
	return [
		"virtual-playback-cell",
		!input.available && "unavailable",
		input.playback && "playback-colored",
		input.running && "running",
		input.configurationArmed && "configuration-armed",
		input.assignmentPending && "assignment-pending",
		input.selectedForZone && "exclusion-selected",
		input.containingZones.length > 0 && "exclusion-member",
		input.updateArmed && "update-target",
	]
		.filter(Boolean)
		.join(" ");
}

function cellDetail(input: {
	available: boolean;
	selectedForZone: boolean;
	assignmentPending: boolean;
	assignmentTarget: number | null;
	configurationArmed: boolean;
	containingZones: readonly VirtualPlaybackZone[];
	playback: PlaybackDefinition | null;
	action: PlaybackDefinition["buttons"][number];
	cueIndex?: number;
}) {
	if (!input.available) return "Unavailable · Outside playback slots 1–127";
	if (input.selectedForZone) return "Selected for exclusion zone";
	if (input.assignmentPending)
		return `Assign Cuelist ${input.assignmentTarget}`;
	if (input.configurationArmed) return "Configure Playback";
	if (input.containingZones.length)
		return input.containingZones.map((zone) => zone.name).join(" · ");
	if (!input.playback) return "Unassigned";
	const action = input.action.replaceAll("_", " ").toUpperCase();
	return `${action}${input.cueIndex == null ? "" : ` · Cue ${input.cueIndex + 1}`}`;
}
