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
	SessionResponse,
	ShowEntry,
	VersionedObject,
} from "../../api/types";
import type { CommandTargetMode } from "../../controlSurface/commandTarget";
import type { FileCapabilities } from "../files/types";
import type { ScreenCapabilities } from "../screens/types";
import type {
	PendingCommandChoice,
	StoredDeskLayout,
	StoredStageLayout,
} from "./contracts";

export interface ServerCoreContext
	extends FileCapabilities,
		ScreenCapabilities {
	status: ConnectionStatus;
	error: string | null;
	dismissError: () => void;
	simulateError: (message: string | null) => void;
	readServerLogs: () => Promise<
		Array<{ revision: number; kind: string; payload: unknown }>
	>;
	bootstrap: BootstrapSnapshot | null;
	session: SessionResponse | null;
	createUser: (name: string) => Promise<void>;
	changeUser: (user: import("../../api/types").DeskUser) => Promise<void>;
	outputRoutes: VersionedObject<OutputRoute>[];
	patchLayers: VersionedObject<PatchLayer>[];
	shows: ShowEntry[];
	matter: MatterBridgeStatus | null;
	fixtureLibrary: FixtureDefinition[];
	fixtureProfiles: FixtureProfile[];
	fixtureProfileWarnings: string[];
	mediaServers: MediaServerFixture[];
	mediaPreviewUrls: Record<string, string>;
	cueObjects: VersionedObject<import("../../api/types").CueList>[];
	deskLayout: VersionedObject<StoredDeskLayout> | null;
	deskLayoutScope: string | null;
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
	refresh: () => Promise<void>;
	setCommandLine: (value: string, pristine?: boolean) => void;
	resetCommandLine: () => void;
	dismissCommandChoice: () => void;
	cancelCommandChoice: () => void;
	executeCommandLine: (
		value?: string,
		interaction?: {
			target: CommandTargetMode;
			pristine: boolean;
		},
	) => Promise<boolean>;
	updateControlDesk: (
		desk: import("../../api/types").ControlDesk,
	) => Promise<void>;
	selectControlDesk: (id: string) => void;
	removeClient: (deskId: string) => Promise<boolean>;
	switchUser: (name: string) => void;
	exportPaperwork: () => void;
	shutdownServer: () => Promise<boolean>;
	clearProgrammer: (sessionId: string) => Promise<void>;
	setDeskToken: (token: string) => void;
	setServerUrl: (url: string) => void;
}
