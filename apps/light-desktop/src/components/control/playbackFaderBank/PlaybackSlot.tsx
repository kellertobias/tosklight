import type {
	PlaybackCardKind,
	PlaybackCardSummary,
} from "@tosklight/ui/playback";
import { PLAYBACK_CARD_DEFAULT_COLORS } from "@tosklight/ui/playback";
import { type CSSProperties, useState } from "react";
import { samePlaybackAddress } from "../../../api/playbackAddress";
import type {
	Cue,
	PlaybackDefinition,
	PlaybackRuntimeProjection,
} from "../../../api/types";
import { useActiveShowId } from "../../../features/deskSnapshot/DeskSnapshotState";
import { playbackControlRuntime } from "../../../features/playbackRuntime/controlRuntime";
import {
	poolSurfaceKey,
	resolveConfiguredPoolPresentation,
	usePoolPresentationConfiguration,
} from "../../../features/poolPresentation/poolPresentation";
import { playbackHasRuntimeError } from "../playbackRuntimeError";
import { formatSpeedGroupBpm } from "../speedGroupFormatting";
import { isPlaybackSetClickArmed } from "./actions";
import type { PlaybackBankController } from "./controller";
import { dynamicPlaybackSummary, playbackFaderValue } from "./feedback";
import { HardwarePlaybackCard } from "./HardwarePlaybackCard";
import { playbackCellGridStyle } from "./projection";
import { buildPlaybackActions, createSlotInterceptors } from "./slotActions";
import { TouchPlaybackCard } from "./TouchPlaybackCard";
import { playbackTileActionLabel } from "./playbackTileActionLabel";
import { timecodePlaybackSummary } from "./timecodePlaybackSummary";
import type { PlaybackSlotProjection } from "./types";

function playbackCommandTarget(controller: PlaybackBankController) {
	if (controller.state.storeArmed) return "record" as const;
	if (
		controller.assignmentPending ||
		controller.setInteractionArmed ||
		controller.groupAssignmentPending ||
		isPlaybackSetClickArmed(controller)
	)
		return "set" as const;
	if (controller.offPending) return "off" as const;
	return null;
}

function playbackCardStyle(
	cell: PlaybackSlotProjection,
	color: string,
	presentationStyle?: CSSProperties,
): CSSProperties {
	const footprint = cell.footprint;
	return {
		...playbackCellGridStyle(cell),
		...(cell.playback ? { "--playback-color": color } : {}),
		...(footprint
			? {
					gridColumn: `${footprint.columnStart} / span ${footprint.columnSpan}`,
					gridRow: `${footprint.rowStart} / span ${footprint.rowSpan}`,
				}
			: {}),
		...presentationStyle,
	} as CSSProperties;
}

function isSelectedPlayback(
	controller: PlaybackBankController,
	playback: PlaybackSlotProjection["playback"],
) {
	return Boolean(
		playback &&
			controller.playbackDesk?.selected_playback &&
			samePlaybackAddress(
				playback.address,
				controller.playbackDesk.selected_playback,
			),
	);
}

interface PlaybackSlotProps {
	controller: PlaybackBankController;
	slotData: PlaybackSlotProjection;
	soloClasses?: string;
	soloMember?: boolean;
}

export function PlaybackSlot({
	controller: bankController,
	slotData,
	soloClasses = "",
	soloMember = false,
}: PlaybackSlotProps) {
	const [controlSourceId] = useState(() => `software:${crypto.randomUUID()}`);
	const controller = { ...bankController, controlSourceId };
	const { playback, cue, group, slot, row } = slotData;
	const poolPresentation = usePoolPresentationConfiguration();
	const showId = useActiveShowId() ?? "unresolved";
	const runtimeProjection = playback
		? controller.runtimeProjections.get(playback.address.position)
		: undefined;
	const active = playbackControlRuntime(runtimeProjection);
	const selected = isSelectedPlayback(controller, playback);
	const configuredButtons =
		row?.button_count ?? (controller.hardware ? 3 : (controller.buttons ?? 3));
	const buttonCount = playback
		? Math.min(configuredButtons, playback.button_count ?? configuredButtons)
		: configuredButtons;
	const hasFader =
		row?.presentation !== "buttons" && (playback?.has_fader ?? true);
	const value = playbackFaderValue(
		playback,
		active,
		runtimeProjection,
		controller.hardware,
	);
	const kind = playbackKind(playback?.target.type);
	const playbackColor = resolvePlaybackColor(kind, playback?.color);
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
	const { summary, running: transportRunning, error: transportError } = timecodePlaybackSummary(
		playback, baseSummary, controller.timecodeRuntime, active?.loaded_cue_number,
	);
	const { actions, faderActions, primaryAction } = buildPlaybackActions({
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
		cue,
		slot,
		currentCue,
		buttonCount,
	);
	const commandTarget = playbackCommandTarget(controller);
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
	const className = `${soloClasses} ${transportError || playbackHasRuntimeError(playback, runtimeProjection, cue !== null) ? "playback-runtime-error" : ""} ${playback ? "playback-colored" : ""} ${presentation?.className ?? ""} ${(active?.enabled !== false && active) || transportRunning ? "running" : ""} ${active?.loaded_cue_number != null ? "loaded" : ""} ${active?.swap_active ? "swap-active" : ""} ${selected ? "selected" : ""} ${!playback ? "empty" : ""} ${controller.assignmentPending ? "assignment-pending" : ""} ${controller.state.storeArmed ? "store-target" : ""} ${controller.state.updateArmed ? "update-target" : ""} ${commandTarget ? `playback-command-target command-target-${commandTarget}` : ""}`;
	const cardStyle = playbackCardStyle(
		slotData,
		playbackColor,
		presentation?.style,
	);
	if (controller.hardware && row?.presentation !== "push_encoder")
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
				commandTarget={commandTarget}
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
			primaryAction={primaryAction}
			primaryActionLabel={playbackTileActionLabel(primaryAction, soloMember)}
			className={className}
			cardStyle={cardStyle}
			commandTarget={commandTarget}
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
	if (target === "macro") return "macro";
	if (target === "timecode") return "timecode";
	if (target === "dynamic") return "dynamic";
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
	if (target === "macro") return { label: "Ready", detail: "GO runs Macro" };
	if (target === "timecode") return { label: "Loading transport…" };
	if (target === "group")
		return {
			label: `${groupFixtureCount ?? 0} Fixture${groupFixtureCount === 1 ? "" : "s"}`,
			detail: value === 0 ? undefined : `${Math.round(value)}%`,
		};
	if (target === "dynamic" && projection?.target === "dynamic") {
		return dynamicPlaybackSummary(projection);
	}
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
