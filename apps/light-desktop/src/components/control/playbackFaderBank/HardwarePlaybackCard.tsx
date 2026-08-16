import type { VerticalTouchFaderAction } from "@tosklight/ui/faders";
import {
	HardwarePlaybackCardView,
	type PlaybackCardKind,
	type PlaybackCardSummary,
	type PlaybackCardViewModel,
} from "@tosklight/ui/playback";
import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import type { PlaybackRuntimeProjection } from "../../../api/types";
import { activateHardwareCard } from "./actions";
import type { PlaybackBankController } from "./controller";
import { ExpandedPlaybackControls } from "./ExpandedPlaybackControls";
import { playbackFaderDisplay } from "./feedback";
import { HardwareCueRows } from "./HardwareCueRows";
import { playbackRowUnits } from "./projection";
import {
	PlaybackAssignmentTarget,
	PlaybackCommandTargetBadge,
	PlaybackConfigurationTarget,
	PlaybackOffTarget,
} from "./SlotControls";
import type { PlaybackSlotProjection, PlaybackSnapshotActive } from "./types";

type HardwarePlaybackCardProps = {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
	active: PlaybackSnapshotActive | undefined;
	runtimeProjection: PlaybackRuntimeProjection | undefined;
	selected: boolean;
	kind: PlaybackCardKind;
	summary?: PlaybackCardSummary;
	color: string;
	hasFader: boolean;
	value: number;
	actions: VerticalTouchFaderAction[];
	className: string;
	cardStyle: CSSProperties | undefined;
	commandTarget: "record" | "set" | "off" | null;
	interceptPointer: (event: ReactPointerEvent<HTMLElement>) => void;
	interceptClick: (event: ReactMouseEvent<HTMLElement>) => void;
};

export function HardwarePlaybackCard({
	controller,
	slotData,
	active,
	runtimeProjection,
	selected,
	kind,
	summary,
	color,
	hasFader,
	value,
	actions,
	className,
	cardStyle,
	commandTarget,
	interceptPointer,
	interceptClick,
}: HardwarePlaybackCardProps) {
	const { playback, cue, group, slot, row, rowIndex } = slotData;
	const cueIndex = active?.enabled === false ? -1 : (active?.cue_index ?? -1);
	const display = playbackFaderDisplay(
		playback,
		active,
		value,
		runtimeProjection,
	);
	const model: PlaybackCardViewModel = {
		page: controller.activePageNumber ?? 1,
		slot,
		row: rowIndex,
		rowUnits: row ? playbackRowUnits(row, controller.hardware) : 1,
		name: playback?.name ?? "Empty",
		assigned: Boolean(playback),
		kind,
		selected,
		selectionPending: controller.selectionPending,
		className,
		style: cardStyle,
		color,
		hasFader,
		faderValue: value,
		faderLabel: playback?.name ?? `Playback ${slot}`,
		faderDisplay: display,
		summary,
		disabled: !playback || !controller.runtimeActions,
		hardwarePickup:
			active?.fader_pickup_required && active.fader_pickup_target != null
				? {
						physicalPosition: active.fader_position ?? 0,
						pickupTarget: active.fader_pickup_target,
					}
				: undefined,
		actions,
	};
	return (
		<HardwarePlaybackCardView
			model={model}
			slots={{
				overlays: (
					<>
						{commandTarget === "record" ? (
							<PlaybackCommandTargetBadge command="record" />
						) : null}
						<ExpandedPlaybackControls
							controller={controller}
							slotData={slotData}
						/>
						<PlaybackAssignmentTarget controller={controller} slot={slot} />
						<PlaybackConfigurationTarget
							controller={controller}
							playback={playback}
							slot={slot}
						/>
						<PlaybackOffTarget controller={controller} playback={playback} />
					</>
				),
			}}
			cueRows={
				cue ? (
					<HardwareCueRows
						cues={cue.cues}
						cueIndex={cueIndex}
						activatedAt={active?.activated_at}
						compact={controller.rowCount === 2}
						effectiveNextCueNumber={active?.effective_next_cue_number}
						effectiveNextIsLoaded={active?.effective_next_is_loaded}
					/>
				) : group ? (
					<div className="hardware-cue-list single">
						<div className="hardware-cue-row current">
							<span>GRP</span>
							<b>{group.body.name ?? `Group ${group.id}`}</b>
							<small>{value}% master</small>
						</div>
					</div>
				) : (
					<div className="hardware-cue-list single" />
				)
			}
			callbacks={{
				onPointerDownCapture: interceptPointer,
				onClickCapture: interceptClick,
				onActivate: (event) =>
					void activateHardwareCard(controller, event, playback, slot),
				onFaderChange: (next) =>
					playback &&
					void controller.runtimeActions?.poolPlaybackAction(
						playback.number,
						"master",
						{
							value: next / 100,
							surface: "physical",
						},
					),
			}}
		/>
	);
}
