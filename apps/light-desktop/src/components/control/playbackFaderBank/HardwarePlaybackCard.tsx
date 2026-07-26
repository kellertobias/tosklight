import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import {
	HardwarePlaybackFaderView,
	PlaybackActionButtons,
} from "@tosklight/ui/playback";
import type { PlaybackRuntimeProjection } from "../../../api/types";
import type { VerticalTouchFaderAction } from "@tosklight/ui/faders";
import { activateHardwareCard } from "./actions";
import type { PlaybackBankController } from "./controller";
import { playbackFaderDisplay } from "./feedback";
import { HardwareCueRows } from "./HardwareCueRows";
import { playbackRowUnits } from "./projection";
import {
	PlaybackAssignmentTarget,
	PlaybackConfigurationTarget,
	PlaybackRuntimeStatus,
} from "./SlotControls";
import type { PlaybackSlotProjection, PlaybackSnapshotActive } from "./types";

type HardwarePlaybackCardProps = {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
	active: PlaybackSnapshotActive | undefined;
	runtimeProjection: PlaybackRuntimeProjection | undefined;
	selected: boolean;
	hasFader: boolean;
	value: number;
	actions: VerticalTouchFaderAction[];
	className: string;
	cardStyle: CSSProperties | undefined;
	interceptPointer: (event: ReactPointerEvent<HTMLElement>) => void;
	interceptClick: (event: ReactMouseEvent<HTMLElement>) => void;
};

export function HardwarePlaybackCard({
	controller,
	slotData,
	active,
	runtimeProjection,
	selected,
	hasFader,
	value,
	actions,
	className,
	cardStyle,
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
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: The hardware card is a pointer selection surface whose actionable child controls retain their own keyboard semantics.
		<article
			data-page={controller.activePageNumber}
			data-playback-slot={slot}
			data-playback-row={rowIndex}
			data-row-units={row ? playbackRowUnits(row, controller.hardware) : 1}
			data-selected-playback={selected || undefined}
			data-selection-pending={controller.selectionPending || undefined}
			className={`hardware-playback-card ${className}`}
			style={cardStyle}
			onPointerDownCapture={interceptPointer}
			onClickCapture={interceptClick}
			onClick={(event) =>
				void activateHardwareCard(controller, event, playback, slot)
			}
		>
			<PlaybackAssignmentTarget controller={controller} slot={slot} />
			<PlaybackConfigurationTarget
				controller={controller}
				playback={playback}
				slot={slot}
			/>
			<header>
				<div
					className="playback-software-representation"
					style={{
						minWidth: 0,
						width: "100%",
						overflow: "hidden",
						padding: 0,
						textAlign: "left",
						textOverflow: "ellipsis",
						whiteSpace: "nowrap",
						userSelect: "none",
						cursor: "default",
					}}
				>
					<b>
						{slot} · {playback?.name ?? "Empty"}
					</b>
				</div>
				<strong>
					{controller.activePageNumber}.{slot}
				</strong>
			</header>
			<PlaybackRuntimeStatus active={active} />
			{cue ? (
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
			)}
			<div className="hardware-playback-controls">
				<footer>
					<PlaybackActionButtons actions={actions} />
				</footer>
				{hasFader && (
					<HardwarePlaybackFaderView
						ariaLabel={`Page ${controller.activePageNumber} playback ${slot} fader`}
						disabled={!playback || !controller.runtimeActions}
						display={display}
						value={value}
						pickup={
							active?.fader_pickup_required &&
							active.fader_pickup_target != null
								? {
										physicalPosition: active.fader_position ?? 0,
										pickupTarget: active.fader_pickup_target,
									}
								: undefined
						}
						onChange={(next) =>
							playback &&
							void controller.runtimeActions?.poolPlaybackAction(
								playback.number,
								"master",
								{
									value: next / 100,
									surface: "physical",
								},
							)
						}
					/>
				)}
			</div>
		</article>
	);
}
