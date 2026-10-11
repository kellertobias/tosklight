import { useEffect } from "react";
import type { PlaybackConfigurationNotification } from "../../../api/playbackConfigurationModels";
import { useControlSurfaceTarget } from "../../../features/controlSurfaceInteraction/useControlSurfaceTarget";
import { openPlaybackConfiguration } from "./actions";
import type { PlaybackBankController } from "./controller";

export function usePlaybackBankConfiguration(
	controller: PlaybackBankController,
	interactionSurfaceId: string,
) {
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
			const playbackIdentity = intent.playback;
			const slotData = controller.slots.find(
				(candidate) => candidate.slot === playbackIdentity.slot,
			);

			return (
				controller.activePageNumber === playbackIdentity.pageNumber &&
				controller.playbackAddressing === playbackIdentity.addressing &&
				(controller.pageObject?.id ?? null) === playbackIdentity.pageObjectId &&
				(controller.pageObject?.revision ?? 0) ===
					playbackIdentity.pageObjectRevision
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
			if (slot == null) return;
			const slotData = controller.slots.find(
				(candidate) => candidate.slot === slot,
			);
			if (slotData)
				openPlaybackConfiguration(controller, slotData.playback, slot);
		},
	});
	useEffect(() => {
		const openRequested = (event: Event) => {
			const { address } = (
				event as CustomEvent<PlaybackConfigurationNotification>
			).detail;
			if (address.page !== controller.activePageNumber) return;
			const slotData = controller.slots.find(
				(candidate) => candidate.slot === address.position,
			);
			if (slotData)
				openPlaybackConfiguration(
					controller,
					slotData.playback,
					address.position,
				);
		};
		window.addEventListener("light:playback-configuration", openRequested);
		return () =>
			window.removeEventListener("light:playback-configuration", openRequested);
	}, [controller]);
}
