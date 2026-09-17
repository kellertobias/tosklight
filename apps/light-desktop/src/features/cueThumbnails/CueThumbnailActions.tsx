import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import type {
	CueMediaPreviewApiClient,
	CueMediaPreviewEntry,
	CueMediaPreviewImage,
} from "../../api/client/cueMediaPreviews";
import type {
	CueThumbnailApiClient,
	CueThumbnailEntry,
	CueThumbnailUpload,
} from "../../api/client/cueThumbnails";

/**
 * Access to the show's stored cue preview pictures.
 *
 * `canStore` is false on a desk that may not write the show — a secondary or read-only session
 * still reads and displays stored previews, it just never uploads a redraw.
 */
interface CueThumbnailActions {
	available: boolean;
	canStore: boolean;
	index(): Promise<CueThumbnailEntry[]>;
	imageUrl(cueId: string): Promise<string>;
	store(uploads: CueThumbnailUpload[]): Promise<void>;
	/** Cues the addressed Media Server pictures; empty when this desk cannot ask. */
	mediaIndex(): Promise<CueMediaPreviewEntry[]>;
	mediaImage(
		cueId: string,
		previewKey: string,
		size: { width: number; height: number },
	): Promise<CueMediaPreviewImage>;
}

const CueThumbnailActionsContext = createContext<CueThumbnailActions | null>(
	null,
);

export function CueThumbnailActionsProvider({
	children,
	client,
	mediaClient,
	showId,
	canWrite,
}: PropsWithChildren<{
	client: CueThumbnailApiClient;
	mediaClient?: CueMediaPreviewApiClient;
	showId: string | null;
	canWrite: boolean;
}>) {
	const actions = useMemo<CueThumbnailActions>(
		() => ({
			available: Boolean(showId),
			canStore: canWrite && Boolean(showId),
			index: () => (showId ? client.index(showId) : Promise.resolve([])),
			imageUrl: (cueId) =>
				showId
					? client.imageUrl(showId, cueId)
					: Promise.reject(new Error("no active show")),
			store: async (uploads) => {
				if (!showId || !canWrite || !uploads.length) return;
				await client.store(showId, uploads);
			},
			mediaIndex: () =>
				showId && mediaClient
					? mediaClient.index(showId)
					: Promise.resolve([]),
			mediaImage: (cueId, previewKey, size) =>
				showId && mediaClient
					? mediaClient.image(showId, cueId, previewKey, size)
					: Promise.reject(new Error("no active show")),
		}),
		[canWrite, client, mediaClient, showId],
	);
	return (
		<CueThumbnailActionsContext.Provider value={actions}>
			{children}
		</CueThumbnailActionsContext.Provider>
	);
}

export function useCueThumbnailActions(): CueThumbnailActions | null {
	return useContext(CueThumbnailActionsContext);
}
