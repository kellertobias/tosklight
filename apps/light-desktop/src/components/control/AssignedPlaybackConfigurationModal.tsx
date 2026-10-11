import { formatErrorDetails } from "@tosklight/ui";
import type { PlaybackDefinition } from "../../api/types";
import { usePlaybackTopologyActions } from "../../features/playbackTopology/PlaybackTopologyProvider";
import {
	PlaybackConfigurationDialog,
	type PlaybackConfigurationModalProps,
} from "./PlaybackConfigurationModal";
import type { PlaybackConfigurationState } from "./playbackFaderBank/types";

interface AssignedPlaybackConfigurationModalProps
	extends PlaybackConfigurationState,
		Pick<PlaybackConfigurationModalProps, "onClose"> {}

/** Shared surface wrapper using the portable, revision-checked topology boundary. */
export function AssignedPlaybackConfigurationModal({
	expectedPageRevision,
	expectedPageObjectId,
	fallbackButtons,
	...props
}: AssignedPlaybackConfigurationModalProps) {
	const topology = usePlaybackTopologyActions();
	const revisionBasis = {
		expectedPageRevision,
		expectedPageObjectId,
	};
	const save = async (
		page: number,
		slot: number,
		playback: PlaybackDefinition,
	) =>
		(await (props.empty ? topology?.assignPlayback : topology?.updatePlayback)?.({page, position: slot}, playback, revisionBasis)) != null;
	const clear = async (page: number, slot: number) =>
		(await topology?.clearPlayback({page, position: slot}, revisionBasis)) != null;
	return (
		<PlaybackConfigurationDialog
			{...props}
			fallbackButtons={fallbackButtons}
			save={save}
			clear={clear}
			error={topology?.error ? formatErrorDetails(topology.error) : undefined}
		/>
	);
}
