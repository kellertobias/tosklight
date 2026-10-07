import { createContext, type PropsWithChildren, useContext } from "react";
import type { DiscoveredPeer } from "../../api/client/discovery";
import type {
	MvrApplyResult,
	MvrImportPreview,
	NetworkShowCatalog,
	ShowEntry,
	ShowRevision,
} from "../../api/types";

/**
 * Scoped Show lifecycle for the setup and recovery surfaces: the stored show list plus
 * open/save/overwrite/upload/download, named revisions, MVR transfer, desk users, and
 * server shutdown.
 */
export interface ShowLifecycleActions {
    setShowDescription: (id: string, description: string) => Promise<void>;
	shows: ShowEntry[];
	openShow: (
		id: string,
		transition?: "hold_current" | "timed_fade" | "safe_blackout",
	) => Promise<boolean>;
	openCleanDefaultShow: () => Promise<boolean>;
	discoveredVisualizers: () => Promise<DiscoveredPeer[]>;
	loadFromVisualizer: (instance: string) => Promise<boolean>;
	initializeEmptyShow: (baseShowId?: string) => Promise<boolean>;
    networkSaveFolders: (instance: string, rootId: string, path: string) => Promise<import("../../api/client/shows").ShowSaveFolders>;
    saveShowCopy: (name: string, target: import("../../api/client/shows").ShowSaveTarget, baseShow: boolean) => Promise<ShowEntry>;
    exportMvrFile: (name: string, target: import("../../api/client/shows").ShowSaveTarget) => Promise<import("../../api/client/shows").ExportedMvrFile>;
    networkShows: () => Promise<NetworkShowCatalog>;
    importRemoteShow: (instance: string, showId: string | null, revision: number | null, open: boolean) => Promise<ShowEntry | null>;
    prepareShowRevision: (id: string, revision: number) => Promise<ShowEntry | null>;
    prepareShowFile: (root: string, path: string, name: string) => Promise<ShowEntry | null>;
    openShowFile: (root: string, path: string, name: string) => Promise<boolean>;
	saveShowAs: (name: string, options?: { baseShow?: boolean; latest?: boolean }) => Promise<boolean>;
	overwriteShow: (destinationId: string) => Promise<boolean>;
	uploadShow: (file: File, overwrite?: boolean) => Promise<void>;
	downloadShow: (show: ShowEntry) => Promise<void>;
	listShowRevisions: (id: string) => Promise<ShowRevision[]>;
	saveShowRevision: (name: string) => Promise<ShowRevision | null>;
	openShowRevision: (id: string, revision: number) => Promise<boolean>;
	previewMvr: (file: File, showId?: string) => Promise<MvrImportPreview>;
	applyMvr: (
		token: string,
		input: {
			new_show?: { name: string; open_after_import: boolean };
			existing_show_id?: string;
			resolutions?: Record<
				string,
				{ action: string; universe?: number; address?: number }
			>;
		},
	) => Promise<MvrApplyResult>;
	shutdownServer: () => Promise<boolean>;
}

const ShowLifecycleContext = createContext<ShowLifecycleActions | null>(null);

export function ShowLifecycleProvider({
	children,
	lifecycle,
}: PropsWithChildren<{ lifecycle: ShowLifecycleActions }>) {
	return (
		<ShowLifecycleContext.Provider value={lifecycle}>
			{children}
		</ShowLifecycleContext.Provider>
	);
}

/** Show lifecycle state and actions, or null outside a mounted desk boundary. */
export function useShowLifecycle(): ShowLifecycleActions | null {
	return useContext(ShowLifecycleContext);
}
