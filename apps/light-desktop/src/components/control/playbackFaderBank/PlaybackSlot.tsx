import { type CSSProperties, useEffect } from "react";
import { legacyPlaybackRuntime } from "../../../features/playbackRuntime/legacy";
import type { PlaybackBankController } from "./controller";
import { playbackFaderValue } from "./feedback";
import { HardwarePlaybackCard } from "./HardwarePlaybackCard";
import { buildPlaybackActions, createSlotInterceptors } from "./slotActions";
import { TouchPlaybackCard } from "./TouchPlaybackCard";
import type { PlaybackSlotProjection } from "./types";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../../features/poolPresentation/poolPresentation";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";

export function PlaybackSlot({
	controller,
	slotData,
}: {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
}) {
	const { playback, cue, slot, row } = slotData;
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const runtimeProjection = playback
		? controller.runtimeProjections.get(playback.number)
		: undefined;
	const active = legacyPlaybackRuntime(runtimeProjection);
	const selected =
		playback?.number === controller.playbackDesk?.selected_playback;
	const configuredButtons =
		row?.button_count ?? (controller.hardware ? 3 : (controller.buttons ?? 3));
	const buttonCount = playback
		? Math.min(configuredButtons, playback.button_count ?? configuredButtons)
		: configuredButtons;
	const hasFader = (row?.has_fader ?? true) && (playback?.has_fader ?? true);
	const value = playbackFaderValue(playback, active, runtimeProjection);
	useEffect(
		() => () => controller.heldActions.releaseSlot(slot),
		[controller.heldActions, playback, slot],
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
				surfaceKey: poolSurfaceKey(showId, representedType),
				objectType: representedType,
				itemColorKey: representedId,
				itemColor: playback?.color,
				states: [
					...(active?.enabled !== false && active
						? (["active"] as const)
						: []),
					...(selected ? (["selected"] as const) : []),
					...(!playback ? (["empty"] as const) : []),
					...(controller.state.storeArmed
						? (["record-target"] as const)
						: []),
					...(controller.state.storeArmed
						? (["store-target"] as const)
						: []),
					...(controller.state.updateArmed
						? (["update-target"] as const)
						: []),
				],
			})
		: null;
	const className = `${playback ? "playback-colored" : ""} ${presentation?.className ?? ""} ${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${active?.swap_active ? "swap-active" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${controller.assignmentPending ? "assignment-pending" : ""} ${controller.state.storeArmed ? "store-target" : ""} ${controller.state.updateArmed ? "update-target" : ""}`;
	const cardStyle = playback
		? ({
				"--playback-color":
					presentation?.color ?? playback.color ?? "#20c997",
				...presentation?.style,
			} as CSSProperties)
		: undefined;
	if (controller.hardware)
		return (
			<HardwarePlaybackCard
				controller={controller}
				slotData={slotData}
				active={active}
				runtimeProjection={runtimeProjection}
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
			runtimeProjection={runtimeProjection}
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
