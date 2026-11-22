import type { PlaylistTransformView } from "../../shared/api/generated/media-wire";

/** The ordinary layer frame used by playlist entries authored before per-step framing. */
export const DEFAULT_PLAYLIST_TRANSFORM: PlaylistTransformView = {
	scaleX: 1,
	scaleY: 1,
	positionX: 0,
	positionY: 0,
	rotation: 0,
};
