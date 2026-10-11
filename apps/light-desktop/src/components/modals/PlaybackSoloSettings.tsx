import { useEffect, useSyncExternalStore } from "react";
import { usePlaybackSoloEditor } from "../../features/playbackSolo/PlaybackSoloEditorContext";
import { usePlaybackSolo } from "../../features/playbackSolo/PlaybackSoloContext";
import { PlaybackSoloRegionManager } from "./PlaybackSoloRegionManager";

/** Every editor operates the cells of one visible Playback surface. */
export function PlaybackSoloSettings({ surfaceId, onClose }: {
	surfaceId: string;
	onClose?: () => void;
}) {
	const editor = usePlaybackSoloEditor(surfaceId);
	const capability = usePlaybackSolo();
	useEffect(() => capability.activate(), [capability.authorityGeneration]);
	const regions = useSyncExternalStore(
		capability.subscribe,
		capability.getRegions,
		capability.getRegions,
	);
	if (!editor) return <p role="status">Opening this surface’s Playback grid…</p>;
	return <PlaybackSoloRegionManager key={capability.authorityGeneration}
		regions={regions} capability={capability}
		editor={editor} onClose={onClose} />;
}
