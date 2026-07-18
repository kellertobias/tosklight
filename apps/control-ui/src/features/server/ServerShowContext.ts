import type { DeskConfiguration, ShowEntry } from "../../api/types";
import type { StoredDeskLayout, StoredStageLayout } from "./contracts";

export interface ServerShowContext {
	createShow: (name: string) => Promise<void>;
	saveShowAs: (name: string) => Promise<boolean>;
	overwriteShow: (destinationId: string) => Promise<boolean>;
	initializeEmptyShow: () => Promise<boolean>;
	uploadShow: (file: File, overwrite?: boolean) => Promise<void>;
	openShow: (
		id: string,
		transition?: "hold_current" | "timed_fade" | "safe_blackout",
	) => Promise<void>;
	openCleanDefaultShow: () => Promise<boolean>;
	openShowFile: (
		rootId: string,
		path: string,
		name: string,
	) => Promise<boolean>;
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
	previewMvrExport: (
		showId: string,
	) => Promise<import("../../api/types").MvrExportPreview>;
	downloadMvr: (show: ShowEntry) => Promise<void>;
	saveConfiguration: (configuration: DeskConfiguration) => Promise<boolean>;
	setControlTiming: (
		input: Partial<
			Pick<
				DeskConfiguration,
				| "speed_groups_bpm"
				| "programmer_fade_millis"
				| "sequence_master_fade_millis"
			>
		>,
	) => Promise<void>;
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
	saveStageLayout: (layout: StoredStageLayout) => Promise<void>;
}
