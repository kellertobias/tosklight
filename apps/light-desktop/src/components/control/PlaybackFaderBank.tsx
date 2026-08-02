import { memo, useEffect, useId, useState } from "react";
import { routeControlSurfaceIntentWithFeedback } from "../../features/controlSurfaceInteraction/registry";
import { useControlSurfaceTarget } from "../../features/controlSurfaceInteraction/useControlSurfaceTarget";
import { PhysicalPlaybackConfigurationModal } from "./PhysicalPlaybackConfigurationModal";
import { openPlaybackConfiguration } from "./playbackFaderBank/actions";
import {
	type PlaybackFaderBankProps,
	usePlaybackBankController,
} from "./playbackFaderBank/controller";
import { PlaybackSlot } from "./playbackFaderBank/PlaybackSlot";

export const PlaybackFaderBank = memo<PlaybackFaderBankProps>(
	function PlaybackFaderBank(props: PlaybackFaderBankProps = {}) {
		const controller = usePlaybackBankController(props);
		const interactionSurfaceId = useId();
		useControlSurfaceTarget({
			id: `playback-bank:${interactionSurfaceId}`,
			priority: 300,
			accepts: (intent) => {
				if (
					intent.type === "configure_playback" &&
					intent.surfaceId === interactionSurfaceId
				)
					return true;
				if (intent.type !== "open_playback_settings") return false;
				const slotData = controller.slots.find(
					(candidate) => candidate.slot === intent.playback.slot,
				);
				const playbackObject = controller.topology.playbacks.find(
					(candidate) => candidate.body.number === slotData?.playback?.number,
				);
				return (
					controller.activePageNumber === intent.playback.pageNumber &&
					controller.playbackAddressing === intent.playback.addressing &&
					(controller.pageObject?.id ?? null) ===
						intent.playback.pageObjectId &&
					(controller.pageObject?.revision ?? 0) ===
						intent.playback.pageObjectRevision &&
					(playbackObject?.id ?? null) === intent.playback.playbackObjectId &&
					(playbackObject?.revision ?? 0) ===
						intent.playback.playbackObjectRevision
				);
			},
			handle: (intent) => {
				if (
					intent.type !== "configure_playback" &&
					intent.type !== "open_playback_settings"
				)
					return;
				const slot =
					intent.type === "configure_playback"
						? intent.slot
						: intent.playback.slot;
				const slotData = controller.slots.find(
					(candidate) => candidate.slot === slot,
				);
				if (slotData)
					openPlaybackConfiguration(controller, slotData.playback, slot);
			},
		});
		// Once the grid has rendered for a ready topology, transient projection refetches
		// (hardware connect, layout changes) must not tear the slot elements down — only a
		// topology-scope reset returns the bank to its loading placeholder.
		const [rendered, setRendered] = useState(false);
		useEffect(() => {
			if (controller.authorityReady) setRendered(true);
			else if (!controller.topology.ready) setRendered(false);
		}, [controller.authorityReady, controller.topology.ready]);
		const showGrid =
			controller.authorityReady || (rendered && controller.topology.ready);
		if (!showGrid)
			return (
				<div
					className="playback-fader-bank playback-authority-status"
					role={controller.authorityError ? "alert" : "status"}
				>
					{controller.authorityError?.message ?? "Loading Playbacks…"}
				</div>
			);
		return (
			<>
				{/* biome-ignore lint/a11y/noStaticElementInteractions: This container routes the pointer-only context menu to the exact typed playback target. */}
				<div
					className={`playback-fader-bank ${controller.hardware ? "hardware-layout" : "touch-layout"}`}
					onContextMenu={(event) => {
						event.preventDefault();
						const card = (event.target as Element).closest<HTMLElement>(
							"[data-playback-slot]",
						);
						const slot = Number(card?.dataset.playbackSlot);
						if (!Number.isInteger(slot)) return;
						routeControlSurfaceIntentWithFeedback({
							type: "configure_playback",
							source: "context_menu",
							surfaceId: interactionSurfaceId,
							slot,
						});
					}}
					style={{
						gridTemplateColumns: `repeat(${controller.columns}, minmax(0, 1fr))`,
						gridTemplateRows: controller.rowTracks,
					}}
				>
					{controller.slots.map((slotData) => (
						<PlaybackSlot
							controller={controller}
							slotData={slotData}
							key={`${slotData.slot}-${slotData.playback?.number ?? "empty"}`}
						/>
					))}
				</div>
				{controller.configuration && (
					<PhysicalPlaybackConfigurationModal
						{...controller.configuration}
						onClose={() => controller.setConfiguration(null)}
					/>
				)}
			</>
		);
	},
	equalPlaybackFaderBankProps,
);

function equalPlaybackFaderBankProps(
	left: PlaybackFaderBankProps,
	right: PlaybackFaderBankProps,
) {
	return (
		(left.pageNumber ?? null) === (right.pageNumber ?? null) &&
		(left.firstSlot ?? 1) === (right.firstSlot ?? 1) &&
		(left.count ?? null) === (right.count ?? null) &&
		(left.rows ?? null) === (right.rows ?? null) &&
		(left.buttons ?? null) === (right.buttons ?? null) &&
		Boolean(left.hardwareConnected) === Boolean(right.hardwareConnected) &&
		equalPlaybackLayout(left.playbackLayout, right.playbackLayout)
	);
}

function equalPlaybackLayout(
	left: PlaybackFaderBankProps["playbackLayout"],
	right: PlaybackFaderBankProps["playbackLayout"],
) {
	if (left === right) return true;
	if (!left || !right || left.playbacks_per_row !== right.playbacks_per_row)
		return false;
	return (
		left.rows.length === right.rows.length &&
		left.rows.every((row, index) => {
			const candidate = right.rows[index];
			if (!candidate) return false;
			return (
				row.first_playback_slot === candidate.first_playback_slot &&
				row.has_fader === candidate.has_fader &&
				row.button_count === candidate.button_count
			);
		})
	);
}

export type { PlaybackFaderBankProps } from "./playbackFaderBank/controller";
export {
	emptyConfiguration,
	playbackButtonLabel,
} from "./playbackFaderBank/feedback";
export { playbackRowUnits } from "./playbackFaderBank/projection";
