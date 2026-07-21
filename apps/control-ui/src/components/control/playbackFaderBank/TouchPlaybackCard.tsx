import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import type { PlaybackRuntimeProjection } from "../../../api/types";
import { isSetContextClick } from "../../../disableContextMenu";
import {
	type VerticalTouchFaderAction,
	VerticalTouchFaderSurface,
} from "../VerticalTouchFader";
import { openPlaybackConfiguration } from "./actions";
import type { PlaybackBankController } from "./controller";
import {
	playbackFaderDisplay,
	playbackFaderLabel,
	playbackFaderModeFeedback,
} from "./feedback";
import { playbackRowUnits } from "./projection";
import {
	PlaybackActionButtons,
	PlaybackAssignmentTarget,
	PlaybackConfigurationTarget,
	PlaybackRepresentation,
	SingleFaderlessButton,
} from "./SlotControls";
import type { PlaybackSlotProjection, PlaybackSnapshotActive } from "./types";

type TouchPlaybackCardProps = {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
	active: PlaybackSnapshotActive | undefined;
	runtimeProjection: PlaybackRuntimeProjection | undefined;
	selected: boolean;
	hasFader: boolean;
	value: number;
	touchActions: VerticalTouchFaderAction[];
	className: string;
	cardStyle: CSSProperties | undefined;
	interceptPointer: (event: ReactPointerEvent<HTMLElement>) => void;
	interceptClick: (event: ReactMouseEvent<HTMLElement>) => void;
};

export function TouchPlaybackCard({
	controller,
	slotData,
	active,
	runtimeProjection,
	selected,
	hasFader,
	value,
	touchActions,
	className,
	cardStyle,
	interceptPointer,
	interceptClick,
}: TouchPlaybackCardProps) {
	const { playback, slot, row, rowIndex } = slotData;
	const singleFaderlessAction =
		!hasFader && touchActions.length === 1 ? touchActions[0] : null;
	const display = playbackFaderDisplay(
		playback,
		active,
		value,
		runtimeProjection,
	);
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: The touch card delegates keyboard semantics to its real child controls and uses article clicks only for the marked SET shortcut.
		<article
			data-set-click-target
			data-page={controller.activePageNumber}
			data-playback-slot={slot}
			data-playback-row={rowIndex}
			data-row-units={row ? playbackRowUnits(row, controller.hardware) : 1}
			data-selected-playback={selected || undefined}
			data-selection-pending={controller.selectionPending || undefined}
			className={className}
			style={cardStyle}
			onPointerDownCapture={interceptPointer}
			onClickCapture={interceptClick}
			onClick={(event) => {
				if (isSetContextClick(event.nativeEvent))
					openPlaybackConfiguration(controller, playback, slot);
			}}
		>
			<PlaybackAssignmentTarget controller={controller} slot={slot} />
			<PlaybackConfigurationTarget
				controller={controller}
				playback={playback}
				slot={slot}
			/>
			{!singleFaderlessAction && (
				<PlaybackRepresentation
					controller={controller}
					playback={playback}
					slot={slot}
				/>
			)}
			{hasFader && (
				<VerticalTouchFaderSurface
					hardware={controller.hardware}
					disabled={
						controller.assignmentPending ||
						!playback ||
						!controller.runtimeActions
					}
					label={playbackFaderLabel(playback)}
					value={value}
					accentColor={playback?.color}
					mode={playbackFaderModeFeedback(playback, active)}
					display={display}
					actions={touchActions}
					onChange={(next) =>
						playback &&
						void controller.runtimeActions?.poolPlaybackAction(
							playback.number,
							"master",
							{ value: next / 100, surface: "physical" },
						)
					}
				/>
			)}
			{singleFaderlessAction && (
				<SingleFaderlessButton
					action={singleFaderlessAction}
					slot={slot}
					playback={playback}
				/>
			)}
			{!hasFader && !singleFaderlessAction && touchActions.length > 0 && (
				<footer
					className={`faderless-playback-actions action-count-${touchActions.length}`}
					style={
						{
							"--playback-action-count": touchActions.length,
						} as CSSProperties
					}
				>
					<PlaybackActionButtons actions={touchActions} />
				</footer>
			)}
		</article>
	);
}
