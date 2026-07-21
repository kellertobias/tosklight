import type { PlaybackDefinition } from "../../api/types";
import { usePlaybackTopologyActions } from "../../features/playbackTopology/PlaybackTopologyProvider";
import {
	PlaybackConfigurationDialog,
	type PlaybackConfigurationModalProps,
} from "./PlaybackConfigurationModal";
import type { PlaybackConfigurationState } from "./playbackFaderBank/types";

interface PhysicalPlaybackConfigurationModalProps
	extends PlaybackConfigurationState,
		Pick<PlaybackConfigurationModalProps, "onClose"> {}

/** Physical-bank wrapper using the portable, revision-checked topology boundary. */
export function PhysicalPlaybackConfigurationModal({
	expectedPageRevision,
	expectedPageObjectId,
	expectedPlaybackRevision,
	expectedPlaybackObjectId,
	fallbackButtons,
	...props
}: PhysicalPlaybackConfigurationModalProps) {
	const topology = usePlaybackTopologyActions();
	const revisionBasis = {
		expectedPageRevision,
		expectedPageObjectId,
		expectedPlaybackRevision,
		expectedPlaybackObjectId,
	};
	const save = async (
		page: number,
		slot: number,
		playback: PlaybackDefinition,
	) =>
		(await topology?.configureSlot(page, slot, playback, revisionBasis)) != null;
	const clear = async (page: number, slot: number) =>
		(await topology?.clearMappedPlayback(page, slot, revisionBasis)) != null;
	return (
		<PlaybackConfigurationDialog
			{...props}
			fallbackButtons={fallbackButtons}
			save={save}
			clear={clear}
			error={topology?.error?.message}
		/>
	);
}
