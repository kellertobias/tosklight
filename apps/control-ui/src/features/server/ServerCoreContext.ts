import type {
	BootstrapSnapshot,
	CommandHistoryEntry,
	ConnectionStatus,
	DeskConfiguration,
	FixtureDefinition,
	FixtureProfile,
	HighlightAction,
	HighlightState,
	MatterBridgeStatus,
	MediaServerFixture,
	OutputRoute,
	PatchLayer,
	PatchSnapshot,
	PlaybackSnapshot,
	ScreenConfiguration,
	ScreenSnapshot,
	SessionResponse,
	ShowEntry,
	StoredGroup,
	StoredPreset,
	UpdateMenuEntry,
	UpdateMode,
	UpdatePreview,
	UpdateResult,
	UpdateSettings,
	UpdateTargetFilter,
	UpdateTargetRequest,
	VersionedObject,
} from "../../api/types";
import type { CommandTargetMode } from "../../controlSurface/commandTarget";
import type {
	PendingCommandChoice,
	StoredDeskLayout,
	StoredStageLayout,
} from "./contracts";

export interface ServerCoreContext {
	status: ConnectionStatus;
	error: string | null;
	dismissError: () => void;
	simulateError: (message: string | null) => void;
	readServerLogs: () => Promise<
		Array<{ revision: number; kind: string; payload: unknown }>
	>;
	fileRoots: () => Promise<import("../../api/types").FileRoot[]>;
	fileEntries: (
		root: string,
		path?: string,
		hidden?: boolean,
	) => Promise<import("../../api/types").FileDirectory>;
	fileMetadata: (
		root: string,
		path: string,
	) => Promise<import("../../api/types").FileMetadata>;
	readFileNote: (
		root: string,
		path: string,
	) => Promise<import("../../api/types").FileNativeNote>;
	saveFileNote: (
		root: string,
		path: string,
		note: string,
	) => Promise<import("../../api/types").FileNativeNote>;
	readTextFile: (
		root: string,
		path: string,
	) => Promise<import("../../api/types").TextDocument>;
	saveTextFile: (
		root: string,
		path: string,
		text: string,
		revision: string | null,
	) => Promise<import("../../api/types").TextDocument>;
	fileOperation: (
		root: string,
		input: {
			operation:
				| "create_file"
				| "create_folder"
				| "rename"
				| "copy"
				| "move"
				| "trash"
				| "delete";
			sources?: string[];
			destination?: string;
			destination_root_id?: string;
			name?: string;
			replace?: boolean;
			conflict?: import("../../api/types").FileConflictChoice;
			apply_to_all?: boolean;
		},
	) => Promise<import("../../api/types").FileOperationResult>;
	fileContent: (root: string, path: string) => Promise<Blob>;
	fileStreamUrl: (root: string, path: string) => Promise<string>;
	fileThumbnail: (
		root: string,
		path: string,
		maxSize?: number,
	) => Promise<Blob>;
	claimFileInput: (
		instanceId: string,
		action: import("../../api/types").FileInputAction,
		origin: "pending" | "toolbar",
	) => Promise<import("../../api/types").FileInputContext>;
	releaseFileInput: (instanceId: string) => Promise<void>;
	bootstrap: BootstrapSnapshot | null;
	session: SessionResponse | null;
	deskLock: import("../../api/types").DeskLockState | null;
	configureDeskLock: (input: {
		message: string;
		wallpaper: string | null;
		unlock_mode: "button" | "pin";
		pin?: string;
	}) => Promise<boolean>;
	lockDesk: () => Promise<void>;
	unlockDesk: (pin?: string) => Promise<boolean>;
	createUser: (name: string) => Promise<void>;
	changeUser: (user: import("../../api/types").DeskUser) => Promise<void>;
	patch: PatchSnapshot | null;
	outputRoutes: VersionedObject<OutputRoute>[];
	patchLayers: VersionedObject<PatchLayer>[];
	playbacks: PlaybackSnapshot | null;
	screens: ScreenSnapshot | null;
	saveScreen: (screen: ScreenConfiguration) => Promise<void>;
	deleteScreen: (id: string) => Promise<void>;
	setScreenPage: (id: string, page: number) => Promise<void>;
	shows: ShowEntry[];
	configuration: DeskConfiguration | null;
	matter: MatterBridgeStatus | null;
	fixtureLibrary: FixtureDefinition[];
	fixtureProfiles: FixtureProfile[];
	fixtureProfileWarnings: string[];
	mediaServers: MediaServerFixture[];
	mediaPreviewUrls: Record<string, string>;
	groups: VersionedObject<StoredGroup>[];
	presets: VersionedObject<StoredPreset>[];
	cueObjects: VersionedObject<import("../../api/types").CueList>[];
	deskLayout: VersionedObject<StoredDeskLayout> | null;
	deskLayoutScope: string | null;
	stageLayout: VersionedObject<StoredStageLayout> | null;
	unresolvedMvrFixtures: VersionedObject<Record<string, unknown>>[];
	commandLine: string;
	commandTargetMode: CommandTargetMode;
	commandLinePristine: boolean;
	commandHistory: CommandHistoryEntry[];
	pendingCommandChoice: PendingCommandChoice | null;
	selectedFixtures: string[];
	selectedGroupId: string | null;
	highlight: HighlightState | null;
	highlightError: string | null;
	dismissHighlightError: () => void;
	highlightAction: (action: HighlightAction) => Promise<boolean>;
	setPatchPreviewHighlight: (
		active: boolean,
		fixtureIds?: string[],
	) => Promise<boolean>;
	updateSettings: () => Promise<UpdateSettings | null>;
	saveUpdateSettings: (settings: UpdateSettings) => Promise<boolean>;
	previewUpdate: (
		target: UpdateTargetRequest,
		mode: UpdateMode,
	) => Promise<UpdatePreview | null>;
	applyUpdate: (
		target: UpdateTargetRequest,
		mode: UpdateMode,
		expectedRevision?: number,
		expectedProgrammerRevision?: string,
	) => Promise<UpdateResult | null>;
	updateTargets: (
		filter: UpdateTargetFilter,
	) => Promise<UpdateMenuEntry[] | null>;
	refresh: () => Promise<void>;
	setCommandLine: (value: string, pristine?: boolean) => void;
	resetCommandLine: () => void;
	cancelCommandChoice: () => void;
	executeCommandLine: (value?: string) => Promise<boolean>;
	updateControlDesk: (
		desk: import("../../api/types").ControlDesk,
	) => Promise<void>;
	selectControlDesk: (id: string) => void;
	removeClient: (deskId: string) => Promise<boolean>;
	switchUser: (name: string) => void;
	exportPaperwork: () => void;
	shutdownServer: () => Promise<boolean>;
	clearProgrammer: (sessionId: string) => Promise<void>;
	clearProgrammerValues: () => Promise<void>;
	setMaster: (grandMaster?: number, blackout?: boolean) => Promise<void>;
	setDeskToken: (token: string) => void;
	setServerUrl: (url: string) => void;
}
