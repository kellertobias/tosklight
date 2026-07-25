import type { PlaybackDefinition } from "../../api/types";
import { usePlaybackTopologyActions } from "../../features/playbackTopology/PlaybackTopologyProvider";
import {
	PlaybackConfigurationDialog,
	type PlaybackConfigurationModalProps,
} from "./PlaybackConfigurationModal";

interface VirtualPlaybackConfigurationModalProps
	extends PlaybackConfigurationModalProps {
	expectedPageRevision: number;
	expectedPageObjectId: string | null;
	expectedPlaybackRevision: number;
	expectedPlaybackObjectId: string | null;
}

/** Virtual-only wrapper using the portable v2 topology action boundary. */
export function VirtualPlaybackConfigurationModal(
	props: VirtualPlaybackConfigurationModalProps,
) {
	const topology = usePlaybackTopologyActions();
	const revisionBasis = {
		expectedPageRevision: props.expectedPageRevision,
		expectedPageObjectId: props.expectedPageObjectId,
		expectedPlaybackRevision: props.expectedPlaybackRevision,
		expectedPlaybackObjectId: props.expectedPlaybackObjectId,
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
			virtual
			fallbackButtons={1}
			save={save}
			clear={clear}
			error={topology?.error?.message}
		/>
	);
}
