import type { DiscoveredPeer } from "../../api/client/discovery";
import type { NetworkShowCatalog, ShowEntry } from "../../api/types";
import type { StoredDeskLayout } from "./contracts";

export interface ServerShowContext {
    setShowDescription: (id: string, description: string) => Promise<void>;
	createShow: (name: string) => Promise<void>;
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
	initializeEmptyShow: (baseShowId?: string) => Promise<boolean>;
	uploadShow: (file: File, overwrite?: boolean) => Promise<void>;
	openShow: (
		id: string,
		transition?: "hold_current" | "timed_fade" | "safe_blackout",
	) => Promise<boolean>;
	openCleanDefaultShow: () => Promise<boolean>;
	/** The Viz editors on the network that currently hold a document worth loading. */
	discoveredVisualizers: () => Promise<DiscoveredPeer[]>;
	/** Import and open the document one of them has open. */
	loadFromVisualizer: (instance: string) => Promise<boolean>;

	listShowRevisions: (
		id: string,
	) => Promise<import("../../api/types").ShowRevision[]>;
	saveShowRevision: (
		name: string,
	) => Promise<import("../../api/types").ShowRevision | null>;
	openShowRevision: (id: string, revision: number) => Promise<boolean>;
	rollbackShow: () => Promise<void>;
	downloadShow: (show: ShowEntry) => Promise<void>;
	previewMvr: (
		file: File,
		showId?: string,
	) => Promise<import("../../api/types").MvrImportPreview>;
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
	) => Promise<import("../../api/types").MvrApplyResult>;
	speedGroup: (
		group: import("../../api/types").SpeedGroupId,
	) => Promise<import("../../api/types").SpeedGroupSoundState>;
	updateSpeedGroup: (
		group: import("../../api/types").SpeedGroupId,
		configuration: import("../../api/types").SoundToLightConfig,
	) => Promise<import("../../api/types").SpeedGroupSoundState>;
	observeSpeedGroup: (
		group: import("../../api/types").SpeedGroupId,
		observation: import("../../api/types").SoundObservation,
	) => Promise<import("../../api/types").SpeedGroupSoundState>;
	speedGroupAction: (
		group: import("../../api/types").SpeedGroupId,
		input: import("../../api/types").SpeedGroupActionInput,
	) => Promise<import("../../api/types").SpeedGroupSoundState>;
	saveDeskLayout: (layout: StoredDeskLayout) => Promise<void>;
}
