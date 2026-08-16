import type { VerticalTouchFaderAction } from "@tosklight/ui/faders";
import {
	type PlaybackCardKind,
	type PlaybackCardSummary,
	type PlaybackCardViewModel,
	TouchPlaybackCardView,
} from "@tosklight/ui/playback";
import type {
	CSSProperties,
	MouseEvent as ReactMouseEvent,
	PointerEvent as ReactPointerEvent,
} from "react";
import type { PlaybackRuntimeProjection } from "../../../api/types";
import type { PlaybackBankController } from "./controller";
import { ExpandedPlaybackControls } from "./ExpandedPlaybackControls";
import {
	playbackFaderDisplay,
	playbackFaderLabel,
	playbackFaderModeFeedback,
} from "./feedback";
import { playbackRowUnits } from "./projection";
import {
	PlaybackAssignmentTarget,
	PlaybackCommandTargetBadge,
	PlaybackConfigurationTarget,
	PlaybackOffTarget,
} from "./SlotControls";
import type { PlaybackSlotProjection, PlaybackSnapshotActive } from "./types";

type TouchPlaybackCardProps = {
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
	touchActions: VerticalTouchFaderAction[];
	className: string;
	cardStyle: CSSProperties | undefined;
	commandTarget: "record" | "set" | "off" | null;
	interceptPointer: (event: ReactPointerEvent<HTMLElement>) => void;
	interceptClick: (event: ReactMouseEvent<HTMLElement>) => void;
};

export function TouchPlaybackCard({
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
	touchActions,
	className,
	cardStyle,
	commandTarget,
	interceptPointer,
	interceptClick,
}: TouchPlaybackCardProps) {
	const { playback, slot, row, rowIndex } = slotData;
	const rowUnits = row ? playbackRowUnits(row, controller.hardware) : 1;
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
		rowUnits,
		name: playback?.name ?? "Empty",
		assigned: Boolean(playback),
		kind,
		selected,
		selectionPending: controller.selectionPending,
		className:
			`${className} ${rowUnits === 1 ? "playback-row-compact" : ""}`.trim(),
		style: cardStyle,
		color,
		hasFader,
		faderValue: value,
		faderLabel: playbackFaderLabel(playback),
		faderDisplay: display,
		faderMode: playbackFaderModeFeedback(playback, active),
		summary,
		disabled:
			controller.assignmentPending || !playback || !controller.runtimeActions,
		actions: touchActions,
	};
	return (
		<TouchPlaybackCardView
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
			callbacks={{
				onPointerDownCapture: interceptPointer,
				onClickCapture: interceptClick,
				onFaderChange: (next) =>
					playback &&
					void controller.runtimeActions?.poolPlaybackAction(
						playback.number,
						"master",
						{ value: next / 100, surface: "virtual" },
					),
			}}
		/>
	);
}
