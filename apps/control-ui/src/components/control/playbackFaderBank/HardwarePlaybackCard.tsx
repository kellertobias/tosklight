import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import { Input } from "../../common";
import type { VerticalTouchFaderAction } from "../VerticalTouchFader";
import { activateHardwareCard } from "./actions";
import type { PlaybackBankController } from "./controller";
import { playbackFaderDisplay } from "./feedback";
import { HardwareCueRows } from "./HardwareCueRows";
import { playbackRowUnits } from "./projection";
import {
	PlaybackActionButtons,
	PlaybackAssignmentTarget,
	PlaybackConfigurationTarget,
} from "./SlotControls";
import type { PlaybackSlotProjection, PlaybackSnapshotActive } from "./types";

type HardwarePlaybackCardProps = {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
	active: PlaybackSnapshotActive | undefined;
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
	const headerPage =
		controller.page?.number ??
		controller.pageNumber ??
		controller.state.playbackPage + 1;
	const display = playbackFaderDisplay(
		playback,
		active,
		value,
		controller.server.configuration,
		controller.server.playbacks?.authoritative_controls,
		controller.state.blackout,
	);
	return (
		// biome-ignore lint/a11y/useKeyWithClickEvents: The hardware card is a pointer selection surface whose actionable child controls retain their own keyboard semantics.
		<article
			data-set-click-target
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
					{headerPage}.{slot}
				</strong>
			</header>
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
						<i />
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
					// biome-ignore lint/a11y/noLabelWithoutControl: Input renders the native range control inside this label.
					<label
						className="hardware-fader"
						style={
							{
								"--hardware-fader-level": `${value}%`,
							} as CSSProperties
						}
					>
						<i />
						<b>{display}</b>
						<Input
							aria-label={`Page ${controller.activePageNumber} playback ${slot} fader`}
							type="range"
							min="0"
							max="100"
							step="0.1"
							value={value}
							onInput={(event) =>
								playback &&
								void controller.server.poolPlaybackAction(
									playback.number,
									"master",
									{
										value: Number(event.currentTarget.value) / 100,
										surface: "physical",
									},
								)
							}
						/>
					</label>
				)}
			</div>
		</article>
	);
}
