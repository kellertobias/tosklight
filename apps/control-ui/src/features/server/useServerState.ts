import { useRef, useState } from "react";
import { LightApiClient } from "../../api/LightApiClient";
import type {
	BootstrapSnapshot,
	CommandHistoryEntry,
	ConnectionStatus,
	DeskConfiguration,
	FixtureDefinition,
	FixtureProfile,
	MatterBridgeStatus,
	OutputRoute,
	PatchLayer,
	PatchSnapshot,
	PlaybackSnapshot,
	ScreenSnapshot,
	SessionResponse,
	ShowEntry,
	VersionedObject,
} from "../../api/types";
import type { CommandTargetMode } from "../../controlSurface/commandTarget";
import type {
	PendingCommandChoice,
	StoredDeskLayout,
	StoredStageLayout,
} from "./contracts";
import { useHighlightState } from "./useHighlightState";
import { useMediaServerState } from "./useMediaServerState";
import { useServerFeatureStores } from "./useServerFeatureStores";
import { useShowObjectsState } from "./useShowObjectsState";

export function useServerState() {
	const client = useRef(new LightApiClient()).current;
	const [status, setStatus] = useState<ConnectionStatus>("connecting");
	const [error, setError] = useState<string | null>(null);
	const [bootstrap, setBootstrap] = useState<BootstrapSnapshot | null>(null);
	const [session, setSession] = useState<SessionResponse | null>(null);
	const [connectionGeneration, setConnectionGeneration] = useState(0);
	const [deskLock, setDeskLock] = useState<
		import("../../api/types").DeskLockState | null
	>(null);
	const [patch, setPatch] = useState<PatchSnapshot | null>(null);
	const [outputRoutes, setOutputRoutes] = useState<
		VersionedObject<OutputRoute>[]
	>([]);
	const [patchLayers, setPatchLayers] = useState<VersionedObject<PatchLayer>[]>(
		[],
	);
	const [playbacks, setPlaybacks] = useState<PlaybackSnapshot | null>(null);
	const featureStores = useServerFeatureStores();
	const [screens, setScreens] = useState<ScreenSnapshot | null>(null);
	const [shows, setShows] = useState<ShowEntry[]>([]);
	const [configuration, setConfiguration] = useState<DeskConfiguration | null>(
		null,
	);
	const [matter, setMatter] = useState<MatterBridgeStatus | null>(null);
	const [fixtureLibrary, setFixtureLibrary] = useState<FixtureDefinition[]>([]);
	const [fixtureProfiles, setFixtureProfiles] = useState<FixtureProfile[]>([]);
	const [fixtureProfileWarnings, setFixtureProfileWarnings] = useState<
		string[]
	>([]);
	const media = useMediaServerState();
	const { showObjectsStore } = useShowObjectsState();
	const [cueObjects, setCueObjects] = useState<
		VersionedObject<import("../../api/types").CueList>[]
	>([]);
	const [deskLayout, setDeskLayout] =
		useState<VersionedObject<StoredDeskLayout> | null>(null);
	const [deskLayoutScope, setDeskLayoutScope] = useState<string | null>(null);
	const showObjectsRequest = useRef(0);
	const [stageLayout, setStageLayout] =
		useState<VersionedObject<StoredStageLayout> | null>(null);
	const [unresolvedMvrFixtures, setUnresolvedMvrFixtures] = useState<
		VersionedObject<Record<string, unknown>>[]
	>([]);
	const [commandTargetMode, setCommandTargetMode] =
		useState<CommandTargetMode>("FIXTURE");
	const commandTargetModeRef = useRef<CommandTargetMode>("FIXTURE");
	const [commandLine, setCommandLineState] = useState("FIXTURE");
	const [commandLinePristine, setCommandLinePristine] = useState(true);
	const [commandHistory, setCommandHistory] = useState<CommandHistoryEntry[]>(
		[],
	);
	const commandLineWrite = useRef<Promise<unknown>>(Promise.resolve());
	const commandLineEpoch = useRef(0);
	const [pendingCommandChoice, setPendingCommandChoice] =
		useState<PendingCommandChoice | null>(null);
	const [selectedFixtures, setSelectedFixtures] = useState<string[]>([]);
	const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
	const highlight = useHighlightState();
	const patchPreviewWrite = useRef<Promise<unknown>>(Promise.resolve());

	return {
		client,
		status,
		setStatus,
		error,
		setError,
		bootstrap,
		setBootstrap,
		session,
		setSession,
		connectionGeneration,
		setConnectionGeneration,
		deskLock,
		setDeskLock,
		patch,
		setPatch,
		outputRoutes,
		setOutputRoutes,
		patchLayers,
		setPatchLayers,
		playbacks,
		setPlaybacks,
		...featureStores,
		screens,
		setScreens,
		shows,
		setShows,
		configuration,
		setConfiguration,
		matter,
		setMatter,
		fixtureLibrary,
		setFixtureLibrary,
		fixtureProfiles,
		setFixtureProfiles,
		fixtureProfileWarnings,
		setFixtureProfileWarnings,
		...media,
		showObjectsStore,
		cueObjects,
		setCueObjects,
		deskLayout,
		setDeskLayout,
		deskLayoutScope,
		setDeskLayoutScope,
		showObjectsRequest,
		stageLayout,
		setStageLayout,
		unresolvedMvrFixtures,
		setUnresolvedMvrFixtures,
		commandTargetMode,
		setCommandTargetMode,
		commandTargetModeRef,
		commandLine,
		setCommandLineState,
		commandLinePristine,
		setCommandLinePristine,
		commandHistory,
		setCommandHistory,
		commandLineWrite,
		commandLineEpoch,
		pendingCommandChoice,
		setPendingCommandChoice,
		selectedFixtures,
		setSelectedFixtures,
		selectedGroupId,
		setSelectedGroupId,
		...highlight,
		patchPreviewWrite,
	};
}

export type ServerState = ReturnType<typeof useServerState>;
