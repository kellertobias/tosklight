import { createContext, type PropsWithChildren, useContext } from "react";
import type { MediaServerInspection } from "../../api/client/mediaOutput";
import type { MatterBridgeStatus, MediaServerFixture } from "../../api/types";

/**
 * Scoped media-server and Matter-bridge desk state for the setup surfaces: matched media
 * fixtures, their preview URLs, the refresh actions, and the bridge status.
 */
export interface MediaServersState {
	mediaServers: MediaServerFixture[];
	mediaPreviewUrls: Record<string, string>;
	refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
	refreshMediaThumbnails: (
		fixtureId: string,
		folder: number,
		elements: number[],
	) => Promise<void>;
	inspectMediaServer: (fixtureId: string) => Promise<MediaServerInspection>;
	applyMediaLibrarySelection: (
		fixtureId: string,
		input: {
			expected_library_revision: string;
			layer_fixture_id: string;
			kind: "content" | "mask";
			folder: number;
			file: number;
		},
	) => Promise<unknown>;
	mediaThumbnail: (
		fixtureId: string,
		folder: number,
		element: number,
	) => Promise<Blob>;
	matter: MatterBridgeStatus | null;
}

const MediaServersContext = createContext<MediaServersState | null>(null);

export function MediaServersProvider({
	children,
	media,
}: PropsWithChildren<{ media: MediaServersState }>) {
	return (
		<MediaServersContext.Provider value={media}>
			{children}
		</MediaServersContext.Provider>
	);
}

/** Media-server desk state, or null outside a mounted desk boundary. */
export function useMediaServers(): MediaServersState | null {
	return useContext(MediaServersContext);
}
