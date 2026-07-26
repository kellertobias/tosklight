import type {
	PlaybackCardKind,
	PlaybackCardSummary,
} from "@tosklight/ui/playback";
import { PLAYBACK_CARD_DEFAULT_COLORS } from "@tosklight/ui/playback";
import { type CSSProperties, useEffect } from "react";
import type {
	Cue,
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../../api/types";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";
import { legacyPlaybackRuntime } from "../../../features/playbackRuntime/legacy";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../../features/poolPresentation/poolPresentation";
import { formatSpeedGroupBpm } from "../speedGroupFormatting";
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
	const kind = playbackKind(playback?.target.type);
	const playbackColor = resolvePlaybackColor(kind, playback?.color);
	useEffect(
		() => () => controller.heldActions.releaseSlot(slot),
		[controller.heldActions, playback, slot],
	);
	const currentCue =
		cue && active && active.cue_index >= 0 ? cue.cues[active.cue_index] : null;
	const baseSummary: PlaybackCardSummary | undefined = playback
		? playbackSummary({
				target: playback.target.type,
				currentCue,
				groupFixtureCount: group?.body.fixtures.length,
				projection: runtimeProjection,
				value,
			})
		: undefined;
	const summary =
		baseSummary && active?.loaded_cue_number != null
			? { ...baseSummary, detail: "LOADED" }
			: baseSummary;
	const { actions, faderActions } = buildPlaybackActions({
		controller,
		playback,
		active,
		selected,
		slot,
		currentCue,
		buttonCount,
		color: playbackColor,
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
					...(active?.enabled !== false && active ? (["active"] as const) : []),
					...(selected ? (["selected"] as const) : []),
					...(!playback ? (["empty"] as const) : []),
					...(controller.state.storeArmed ? (["record-target"] as const) : []),
					...(controller.state.storeArmed ? (["store-target"] as const) : []),
					...(controller.state.updateArmed ? (["update-target"] as const) : []),
				],
			})
		: null;
	const className = `${playback ? "playback-colored" : ""} ${presentation?.className ?? ""} ${active?.enabled !== false && active ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${active?.swap_active ? "swap-active" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${controller.assignmentPending ? "assignment-pending" : ""} ${controller.state.storeArmed ? "store-target" : ""} ${controller.state.updateArmed ? "update-target" : ""}`;
	const cardStyle = playback
		? ({
				"--playback-color": playbackColor,
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
				kind={kind}
				summary={summary}
				color={playbackColor}
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
			kind={kind}
			summary={summary}
			color={playbackColor}
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

function resolvePlaybackColor(kind: PlaybackCardKind, color?: string | null) {
	const normalized = color?.trim();
	if (normalized && normalized.toLowerCase() !== "#20c997") return normalized;
	return kind === "empty" ? "#66717a" : PLAYBACK_CARD_DEFAULT_COLORS[kind];
}

function playbackKind(
	target: PlaybackDefinition["target"]["type"] | undefined,
): PlaybackCardKind {
	if (target === "cue_list") return "cue-list";
	if (target === "group") return "group-master";
	if (target === "speed_group") return "speed-group";
	if (
		target === "grand_master" ||
		target === "programmer_fade" ||
		target === "cue_fade"
	)
		return "special-master";
	return "empty";
}

function playbackSummary({
	target,
	currentCue,
	groupFixtureCount,
	projection,
	value,
}: {
	target: PlaybackDefinition["target"]["type"];
	currentCue: Cue | null;
	groupFixtureCount?: number;
	projection: PlaybackRuntimeProjection | undefined;
	value: number;
}): PlaybackCardSummary {
	if (target === "cue_list")
		return {
			label: currentCue
				? `${currentCue.number} · ${currentCue.name || `Cue ${currentCue.number}`}`
				: "No active cue",
			detail: currentCue
				? `${(currentCue.fade_millis / 1_000).toFixed(1)}s`
				: undefined,
			progress:
				projection?.target === "cue_list"
					? projection.runtime?.manual_xfade_progress
					: 0,
		};
	if (target === "group")
		return {
			label: `${groupFixtureCount ?? 0} Fixture${groupFixtureCount === 1 ? "" : "s"}`,
			detail: `${Math.round(value)}%`,
		};
	if (target === "speed_group" && projection?.target === "speed_group") {
		const runtime = projection.runtime;
		return {
			label: `${formatSpeedGroupBpm(runtime.effective_bpm)} BPM`,
			detail: runtime.paused ? "PAUSED" : runtime.source.replaceAll("_", " "),
			beat: {
				count: 4,
				active: Math.min(3, Math.floor(runtime.beat_phase * 4)),
			},
		};
	}
	if (target === "programmer_fade")
		return { label: "Programmer fade", detail: `${Math.round(value)}%` };
	if (target === "cue_fade")
		return { label: "Cue fade", detail: `${Math.round(value)}%` };
	if (target === "grand_master")
		return { label: "Grand master", detail: `${Math.round(value)}%` };
	return { label: "Unavailable" };
}
