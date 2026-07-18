import type { CSSProperties } from "react";
import type { PlaybackBankController } from "./controller";
import { playbackFaderValue } from "./feedback";
import { HardwarePlaybackCard } from "./HardwarePlaybackCard";
import { buildPlaybackActions, createSlotInterceptors } from "./slotActions";
import { TouchPlaybackCard } from "./TouchPlaybackCard";
import type { PlaybackSlotProjection } from "./types";

export function PlaybackSlot({
	controller,
	slotData,
}: {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
}) {
	const { playback, cue, group, slot, row } = slotData;
	const active = playback
		? controller.server.playbacks?.active.find(
				(item) => item.playback_number === playback.number,
			)
		: undefined;
	const selected =
		playback?.number === controller.server.playbacks?.selected_playback;
	const configuredButtons =
		row?.button_count ??
		(controller.hardware
			? 3
			: (controller.buttons ?? controller.server.playbacks?.desk.buttons ?? 3));
	const buttonCount = playback
		? Math.min(configuredButtons, playback.button_count ?? configuredButtons)
		: configuredButtons;
	const hasFader = (row?.has_fader ?? true) && (playback?.has_fader ?? true);
	const value = playbackFaderValue(
		playback,
		active,
		group?.body.master,
		controller.server.configuration,
		controller.server.playbacks?.authoritative_controls,
		1,
	);
	const currentCue =
		cue && active && active.cue_index >= 0 ? cue.cues[active.cue_index] : null;
	const { actions, faderActions } = buildPlaybackActions({
		controller,
		playback,
		active,
		selected,
		slot,
		currentCue,
		buttonCount,
	});
	const touchActions = faderActions.filter(
		(_, button) => actions[button] !== "none",
	);
	const { interceptPointer, interceptClick } = createSlotInterceptors(
		controller,
		playback,
		slot,
		currentCue,
	);
	const className = `${playback ? "playback-colored" : ""} ${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${active?.fader_pickup_required ? "pickup-required" : ""} ${active?.swap_active ? "swap-active" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${controller.assignmentPending ? "assignment-pending" : ""} ${controller.state.storeArmed ? "store-target" : ""} ${controller.state.updateArmed ? "update-target" : ""}`;
	const cardStyle = playback
		? ({
				"--playback-color": playback.color ?? "#20c997",
			} as CSSProperties)
		: undefined;
	if (controller.hardware)
		return (
			<HardwarePlaybackCard
				controller={controller}
				slotData={slotData}
				active={active}
				selected={selected}
				hasFader={hasFader}
				value={value}
				actions={faderActions}
				className={className}
				cardStyle={cardStyle}
				interceptPointer={interceptPointer}
				interceptClick={interceptClick}
			/>
		);
	return (
		<TouchPlaybackCard
			controller={controller}
			slotData={slotData}
			active={active}
			selected={selected}
			hasFader={hasFader}
			value={value}
			touchActions={touchActions}
			className={className}
			cardStyle={cardStyle}
			interceptPointer={interceptPointer}
			interceptClick={interceptClick}
		/>
	);
}
