import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type PropsWithChildren } from "react";
import { LightApiClient, saveServerUrl } from "./LightApiClient";
import type { DeskModel } from "../types";
import type {
  BootstrapSnapshot,
  CommandHistoryEntry,
  ConnectionStatus,
  Cue,
  DeskConfiguration,
  FixtureDefinition,
  FixtureProfile,
  HighlightAction,
  HighlightState,
  MatterBridgeStatus,
  DmxSnapshot,
  MediaServerFixture,
  OutputRoute,
  PatchSnapshot,
  PatchLayer,
  PatchedFixture,
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
  VisualizationSnapshot,
} from "./types";
import { commandTargetAfterEnter, defaultCommandLine, type CommandTargetMode } from "../components/control/softwareKeypad";

export interface StoredDeskLayout {
  desks: DeskModel[];
  activeDeskId: string;
  windowSettings?: Partial<import("../types").WindowSettings>;
}

export interface CommandChoiceOption {
  id: string;
  label: string;
  command: string;
}

export interface PendingCommandChoice {
  type: "cue_move_copy";
  operation: "copy" | "move";
  command: string;
  options: CommandChoiceOption[];
  cancel_label: string;
}

export function deskLayoutScopeKey(showId: string | null | undefined, userId: string | null | undefined) {
  return showId && userId ? `${showId}:${userId}` : null;
}

export function cueOnlyRestoration(cues: Cue[]): { changes: Cue["changes"]; group_changes: NonNullable<Cue["group_changes"]> } {
  const cueOnly = cues.at(-1);
  if (!cueOnly?.cue_only) return { changes: [], group_changes: [] };
  const fixtureState = new Map<string, Cue["changes"][number]["value"]>();
  const groupState = new Map<string, NonNullable<Cue["group_changes"]>[number]["value"]>();
  for (const cue of cues.slice(0, -1)) {
    for (const change of cue.changes) {
      const key = `${change.fixture_id}\u0000${change.attribute}`;
      if (change.value == null) fixtureState.delete(key); else fixtureState.set(key, change.value);
    }
    for (const change of cue.group_changes ?? []) {
      const key = `${change.group_id}\u0000${change.attribute}`;
      if (change.value == null) groupState.delete(key); else groupState.set(key, change.value);
    }
  }
  const changes = cueOnly.changes
    .filter((change) => !change.automatic_restore)
    .map((change) => ({
      fixture_id: change.fixture_id,
      attribute: change.attribute,
      value: fixtureState.get(`${change.fixture_id}\u0000${change.attribute}`) ?? null,
      automatic_restore: true,
    }));
  const group_changes = (cueOnly.group_changes ?? [])
    .filter((change) => !change.automatic_restore)
    .map((change) => ({
      group_id: change.group_id,
      attribute: change.attribute,
      value: groupState.get(`${change.group_id}\u0000${change.attribute}`) ?? null,
      automatic_restore: true,
    }));
  return { changes, group_changes };
}
export interface StagePosition3d {
  x: number;
  y: number;
  z: number;
  rotationX: number;
  rotationY: number;
  rotationZ: number;
}
export interface StageAsset {
  id: string;
  name: string;
  format: "glb" | "stl" | "3mf" | "builtin";
  dataUrl?: string;
  builtinId?: import("../windows/builtInStageModels").BuiltInStageAssetId;
  position: StagePosition3d;
  scale: number;
}
export interface StoredStageLayout {
  version?: 2;
  positions: Record<string, { x: number; y: number; rotation: number }>;
  positions3d?: Record<string, StagePosition3d>;
  camera3d?: {
    position: [number, number, number];
    target: [number, number, number];
  };
  assets?: StageAsset[];
}

interface ServerContextValue {
  status: ConnectionStatus;
  error: string | null;
  dismissError: () => void;
  simulateError: (message: string | null) => void;
  readServerLogs: () => Promise<Array<{ revision: number; kind: string; payload: unknown }>>;
  fileRoots: () => Promise<import("./types").FileRoot[]>;
  fileEntries: (root: string, path?: string, hidden?: boolean) => Promise<import("./types").FileDirectory>;
  fileMetadata: (root: string, path: string) => Promise<import("./types").FileMetadata>;
  readFileNote: (root: string, path: string) => Promise<import("./types").FileNativeNote>;
  saveFileNote: (root: string, path: string, note: string) => Promise<import("./types").FileNativeNote>;
  readTextFile: (root: string, path: string) => Promise<import("./types").TextDocument>;
  saveTextFile: (root: string, path: string, text: string, revision: string | null) => Promise<import("./types").TextDocument>;
  fileOperation: (
    root: string,
    input: {
      operation: "create_file" | "create_folder" | "rename" | "copy" | "move" | "trash" | "delete";
      sources?: string[];
      destination?: string;
      destination_root_id?: string;
      name?: string;
      replace?: boolean;
      conflict?: import("./types").FileConflictChoice;
      apply_to_all?: boolean;
    },
  ) => Promise<import("./types").FileOperationResult>;
  fileContent: (root: string, path: string) => Promise<Blob>;
  fileStreamUrl: (root: string, path: string) => Promise<string>;
  fileThumbnail: (root: string, path: string, maxSize?: number) => Promise<Blob>;
  claimFileInput: (instanceId: string, action: import("./types").FileInputAction, origin: "pending" | "toolbar") => Promise<import("./types").FileInputContext>;
  releaseFileInput: (instanceId: string) => Promise<void>;
  bootstrap: BootstrapSnapshot | null;
  session: SessionResponse | null;
  deskLock: import("./types").DeskLockState | null;
  configureDeskLock: (input: { message: string; wallpaper: string | null; unlock_mode: "button" | "pin"; pin?: string }) => Promise<boolean>;
  lockDesk: () => Promise<void>;
  unlockDesk: (pin?: string) => Promise<boolean>;
  createUser: (name: string) => Promise<void>;
  changeUser: (user: import("./types").DeskUser) => Promise<void>;
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
  cueObjects: VersionedObject<import("./types").CueList>[];
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
  updateSettings: () => Promise<UpdateSettings | null>;
  saveUpdateSettings: (settings: UpdateSettings) => Promise<boolean>;
  previewUpdate: (target: UpdateTargetRequest, mode: UpdateMode) => Promise<UpdatePreview | null>;
  applyUpdate: (target: UpdateTargetRequest, mode: UpdateMode, expectedRevision?: number, expectedProgrammerRevision?: string) => Promise<UpdateResult | null>;
  updateTargets: (filter: UpdateTargetFilter) => Promise<UpdateMenuEntry[] | null>;
  refresh: () => Promise<void>;
  setCommandLine: (value: string, pristine?: boolean) => void;
  resetCommandLine: () => void;
  cancelCommandChoice: () => void;
  executeCommandLine: (value?: string) => Promise<boolean>;
  undoProgrammer: () => Promise<void>;
  setSelection: (fixtures: string[]) => Promise<void>;
  selectionGesture: (
    source:
      | { type: "fixture"; fixture_id: string }
      | { type: "live_group"; group_id: string }
      | { type: "dereferenced_group"; group_id: string },
    remove?: boolean,
  ) => Promise<void>;
  setProgrammer: (fixtureId: string, attribute: string, value: number) => Promise<void>;
  setProgrammerMany: (
    assignments: Array<{ fixtureId: string; attribute: string; value: number }>,
  ) => Promise<boolean>;
  setProgrammerValue: (
    fixtureId: string,
    attribute: string,
    value: import("./types").AttributeValue,
  ) => Promise<void>;
  controlFixtureAction: (fixtureId: string, actionId: string, active: boolean) => Promise<void>;
  generateFixturePresets: (
    fixtureIds: string[],
  ) => Promise<import("./types").GeneratedFixturePresetResult | null>;
  releaseProgrammer: (fixtureId: string, attribute: string) => Promise<void>;
  setGroupValue: (attribute: string, value: number) => Promise<void>;
  releaseGroupValue: (attribute: string) => Promise<void>;
  setPreloadGroupValue: (attribute: string, value: number) => Promise<void>;
  playbackAction: (cueListId: string, action: "go" | "back" | "pause" | "release") => Promise<void>;
  poolPlaybackAction: (
    number: number,
    action: "button" | "on" | "off" | "toggle" | "go" | "go-minus" | "go-to" | "load" | "fast-forward" | "fast-rewind" | "temp" | "temp-on" | "temp-off" | "swap" | "select" | "select-contents" | "select-dereferenced" | "learn" | "double" | "half" | "pause" | "blackout" | "pause-dynamics" | "flash" | "master" | "xfade-on" | "xfade-off",
    input?: {
      value?: number;
      pressed?: boolean;
      button?: number;
      cue_number?: number;
      surface?: "physical" | "virtual";
    },
  ) => Promise<void>;
  readVirtualPlaybackExclusionZones: () => Promise<import("./types").VirtualPlaybackExclusionSnapshot>;
  saveVirtualPlaybackExclusionZones: (surfaceId: string, zones: import("./types").VirtualPlaybackExclusionZone[]) => Promise<boolean>;
  setPlaybackPage: (page: number) => Promise<void>;
  savePlaybackPage: (page: import("./types").PlaybackPage) => Promise<boolean>;
  updateControlDesk: (desk: import("./types").ControlDesk) => Promise<void>;
  selectControlDesk: (id: string) => void;
  removeClient: (deskId: string) => Promise<boolean>;
  savePlaybackDefinition: (playback: import("./types").PlaybackDefinition) => Promise<void>;
  savePlaybackSlot: (page: number, slot: number, playback: import("./types").PlaybackDefinition) => Promise<boolean>;
  clearPlaybackSlot: (page: number, slot: number) => Promise<boolean>;
  saveCueList: (cueList: import("./types").CueList, revision: number) => Promise<boolean>;
  unassignPagePlayback: (page: number, slot: number) => Promise<boolean>;
  readDmx: () => Promise<DmxSnapshot>;
  readVisualization: (preload?: boolean) => Promise<VisualizationSnapshot>;
  setDmxOverride: (universe: number, address: number, value: number | null) => Promise<void>;
  saveOutputRoute: (id: string, route: OutputRoute, revision: number) => Promise<boolean>;
  deleteOutputRoute: (id: string, revision: number) => Promise<boolean>;
  createShow: (name: string) => Promise<void>;
  saveShowAs: (name: string) => Promise<boolean>;
  overwriteShow: (destinationId: string) => Promise<boolean>;
  initializeEmptyShow: () => Promise<boolean>;
  uploadShow: (file: File, overwrite?: boolean) => Promise<void>;
  openShow: (id: string, transition?: "hold_current" | "timed_fade" | "safe_blackout") => Promise<void>;
  openShowFile: (rootId: string, path: string, name: string) => Promise<boolean>;
  listShowRevisions: (id: string) => Promise<import("./types").ShowRevision[]>;
  saveShowRevision: (name: string) => Promise<import("./types").ShowRevision | null>;
  openShowRevision: (id: string, revision: number) => Promise<boolean>;
  rollbackShow: () => Promise<void>;
  downloadShow: (show: ShowEntry) => Promise<void>;
  previewMvr: (file: File, showId?: string) => Promise<import("./types").MvrImportPreview>;
  applyMvr: (
    token: string,
    input: {
      new_show?: { name: string; open_after_import: boolean };
      existing_show_id?: string;
      resolutions?: Record<string, { action: string; universe?: number; address?: number }>;
    },
  ) => Promise<import("./types").MvrApplyResult>;
  previewMvrExport: (showId: string) => Promise<import("./types").MvrExportPreview>;
  downloadMvr: (show: ShowEntry) => Promise<void>;
  saveConfiguration: (configuration: DeskConfiguration) => Promise<boolean>;
  setControlTiming: (input: Partial<Pick<DeskConfiguration, "speed_groups_bpm" | "programmer_fade_millis" | "sequence_master_fade_millis">>) => Promise<void>;
  speedGroup: (group: import("./types").SpeedGroupId) => Promise<import("./types").SpeedGroupSoundState>;
  updateSpeedGroup: (group: import("./types").SpeedGroupId, configuration: import("./types").SoundToLightConfig) => Promise<import("./types").SpeedGroupSoundState>;
  observeSpeedGroup: (group: import("./types").SpeedGroupId, observation: import("./types").SoundObservation) => Promise<import("./types").SpeedGroupSoundState>;
  speedGroupAction: (group: import("./types").SpeedGroupId, input: import("./types").SpeedGroupActionInput) => Promise<import("./types").SpeedGroupSoundState>;
  saveDeskLayout: (layout: StoredDeskLayout) => Promise<void>;
  saveStageLayout: (layout: StoredStageLayout) => Promise<void>;
  applyGroup: (id: string) => Promise<void>;
  selectGroup: (id: string, frozen?: boolean, rule?: Record<string, unknown>) => Promise<void>;
  selectionMacro: (rule: Record<string, unknown>) => Promise<void>;
  alignSelection: (attribute: string, mode: "left" | "right" | "center" | "out") => Promise<void>;
  preloadAction: (action: "enter" | "go" | "clear" | "release") => Promise<void>;
  storePreload: (
    input: {
      target: "preset" | "cue";
      target_id: string;
      cue_number?: number;
      name?: string;
      mode?: "merge" | "overwrite" | "add_missing_fixtures";
    },
    revision: number,
  ) => Promise<boolean>;
  storeDynamic: (speed: number, width: number, direction: string) => Promise<void>;
  storePlayback: (slot: number, cueListId?: string, pageNumber?: number) => Promise<void>;
  storeGroup: (id: string, name: string, mode?: "merge" | "overwrite") => Promise<void>;
  updateGroup: (id: string, update: Pick<StoredGroup, "name" | "color" | "icon">) => Promise<boolean>;
  setGroupMaster: (id: string, master: number) => Promise<void>;
  setGroupMasterFlash: (id: string, value: number) => Promise<void>;
  undoGroup: (id: string) => Promise<void>;
  refreshFrozenGroup: (id: string) => Promise<void>;
  detachDerivedGroup: (id: string) => Promise<void>;
  applyPreset: (id: string) => Promise<void>;
  storePreset: (id: string, name: string, mode: "merge" | "overwrite" | "add_missing_fixtures", family?: string) => Promise<void>;
  switchUser: (name: string) => void;
  exportPaperwork: () => void;
  shutdownServer: () => Promise<boolean>;
  clearProgrammer: (sessionId: string) => Promise<void>;
  clearProgrammerValues: () => Promise<void>;
  setMaster: (grandMaster?: number, blackout?: boolean) => Promise<void>;
  setDeskToken: (token: string) => void;
  setServerUrl: (url: string) => void;
  refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
  refreshMediaThumbnails: (fixtureId: string, elements: number[]) => Promise<void>;
  configureMediaServer: (fixtureId: string, ipAddress: string | null, port?: number) => Promise<void>;
  saveFixtureDefinition: (definition: FixtureDefinition) => Promise<boolean>;
  deleteFixtureDefinition: (id: string, revision: number) => Promise<void>;
  saveFixtureProfile: (profile: FixtureProfile, expectedRevision: number) => Promise<FixtureProfile>;
  deleteFixtureProfile: (id: string, revision: number) => Promise<void>;
  fixtureProfileRevisions: (id: string) => Promise<FixtureProfile[]>;
  saveFixtureProfileSourceGdtf: (id: string, revision: number, source: Uint8Array) => Promise<boolean>;
  importFixturePackage: (source: Uint8Array) => Promise<FixtureProfile>;
  exportFixturePackage: (id: string, revision: number) => Promise<Blob>;
  patchFixture: (input: { name: string; fixture_number: number; definition: FixtureDefinition; universe: number | null; address: number | null; split_patches?: import("./types").SplitPatch[]; layer_id?: string }) => Promise<string | null>;
  updatePatchedFixture: (fixtureId: string, changes: Partial<PatchedFixture>) => Promise<boolean>;
  savePatchLayer: (layer: PatchLayer) => Promise<boolean>;
}

const ServerContext = createContext<ServerContextValue | null>(null);

export function ServerProvider({ children }: PropsWithChildren) {
  const client = useRef(new LightApiClient()).current;
  const [status, setStatus] = useState<ConnectionStatus>("connecting");
  const [error, setError] = useState<string | null>(null);
  const [bootstrap, setBootstrap] = useState<BootstrapSnapshot | null>(null);
  const [session, setSession] = useState<SessionResponse | null>(null);
  const [deskLock, setDeskLock] = useState<import("./types").DeskLockState | null>(null);
  const [patch, setPatch] = useState<PatchSnapshot | null>(null);
  const [outputRoutes, setOutputRoutes] = useState<VersionedObject<OutputRoute>[]>([]);
  const [patchLayers, setPatchLayers] = useState<VersionedObject<PatchLayer>[]>([]);
  const [playbacks, setPlaybacks] = useState<PlaybackSnapshot | null>(null);
  const [screens, setScreens] = useState<ScreenSnapshot | null>(null);
  const [shows, setShows] = useState<ShowEntry[]>([]);
  const [configuration, setConfiguration] = useState<DeskConfiguration | null>(null);
  const [matter, setMatter] = useState<MatterBridgeStatus | null>(null);
  const [fixtureLibrary, setFixtureLibrary] = useState<FixtureDefinition[]>([]);
  const [fixtureProfiles, setFixtureProfiles] = useState<FixtureProfile[]>([]);
  const [fixtureProfileWarnings, setFixtureProfileWarnings] = useState<string[]>([]);
  const [mediaServers, setMediaServers] = useState<MediaServerFixture[]>([]);
  const [mediaPreviewUrls, setMediaPreviewUrls] = useState<Record<string, string>>({});
  const mediaPreviewUrlsRef = useRef<Record<string, string>>({});
  const [groups, setGroups] = useState<VersionedObject<StoredGroup>[]>([]);
  const [presets, setPresets] = useState<VersionedObject<StoredPreset>[]>([]);
  const [cueObjects, setCueObjects] = useState<VersionedObject<import("./types").CueList>[]>([]);
  const [deskLayout, setDeskLayout] = useState<VersionedObject<StoredDeskLayout> | null>(null);
  const [deskLayoutScope, setDeskLayoutScope] = useState<string | null>(null);
  const showObjectsRequest = useRef(0);
  const [stageLayout, setStageLayout] = useState<VersionedObject<StoredStageLayout> | null>(null);
  const [unresolvedMvrFixtures, setUnresolvedMvrFixtures] = useState<VersionedObject<Record<string, unknown>>[]>([]);
  const [commandTargetMode, setCommandTargetMode] = useState<CommandTargetMode>("FIXTURE");
  const commandTargetModeRef = useRef<CommandTargetMode>("FIXTURE");
  const [commandLine, setCommandLineState] = useState("FIXTURE");
  const [commandLinePristine, setCommandLinePristine] = useState(true);
  const [commandHistory, setCommandHistory] = useState<CommandHistoryEntry[]>([]);
  const commandLineWrite = useRef<Promise<unknown>>(Promise.resolve());
  const commandLineEpoch = useRef(0);
  const [pendingCommandChoice, setPendingCommandChoice] = useState<PendingCommandChoice | null>(null);
  const [selectedFixtures, setSelectedFixtures] = useState<string[]>([]);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  const [highlight, setHighlight] = useState<HighlightState | null>(null);
  const [highlightError, setHighlightError] = useState<string | null>(null);
  const highlightEpoch = useRef(0);
  const highlightWrite = useRef<Promise<unknown>>(Promise.resolve());
  const highlightErrorSticky = useRef(false);
  useEffect(
    () => () => {
      for (const url of Object.values(mediaPreviewUrlsRef.current)) URL.revokeObjectURL(url);
    },
    [],
  );
  useEffect(() => {
    if (!session) return;
    let cancelled = false;
    const refreshLock = () =>
      void client
        .deskLock()
        .then((value) => {
          if (!cancelled) setDeskLock(value);
        })
        .catch(() => undefined);
    refreshLock();
    const timer = window.setInterval(refreshLock, 500);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [client, session]);
  useEffect(() => {
    for (const url of Object.values(mediaPreviewUrlsRef.current)) URL.revokeObjectURL(url);
    mediaPreviewUrlsRef.current = {};
    setMediaPreviewUrls({});
  }, [bootstrap?.active_show?.id]);

  useEffect(() => {
    if (!session) {
      highlightEpoch.current += 1;
      highlightErrorSticky.current = false;
      setHighlight(null);
      setHighlightError(null);
      return;
    }
    let cancelled = false;
    const load = () => {
      const request = ++highlightEpoch.current;
      void highlightWrite.current
        .catch(() => undefined)
        .then(() => client.highlight())
        .then((next) => {
          if (cancelled || request !== highlightEpoch.current) return;
          setHighlight(next);
          if (!highlightErrorSticky.current) setHighlightError(null);
        })
        .catch((reason) => {
          if (!cancelled && request === highlightEpoch.current) setHighlightError(reason instanceof Error ? reason.message : String(reason));
        });
    };
    load();
    // WebSocket events provide the normal fast path. This slow refresh keeps a secondary
    // device authoritative after reconnects or when an older server omits the event.
    const timer = window.setInterval(load, 2_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [client, session]);

  const loadShowObjects = useCallback(
    async (showId: string | null, userId: string | null) => {
      const request = ++showObjectsRequest.current;
      const scope = deskLayoutScopeKey(showId, userId);
      setDeskLayoutScope((loaded) => loaded === scope ? loaded : null);
      if (!showId) {
        if (request !== showObjectsRequest.current) return;
        setGroups([]);
        setPresets([]);
        setCueObjects([]);
        setOutputRoutes([]);
        setDeskLayout(null);
        setStageLayout(null);
        setUnresolvedMvrFixtures([]);
        setDeskLayoutScope(null);
        return;
      }
      const [nextGroups, nextPresets, nextCueObjects, nextOutputRoutes, layouts, stageLayouts, nextPatchLayers, unresolvedMvr] = await Promise.all([
        client.objects<StoredGroup>(showId, "group"),
        client.objects<StoredPreset>(showId, "preset"),
        client.objects<import("./types").CueList>(showId, "cue_list"),
        client.objects<OutputRoute>(showId, "route"),
        userId ? client.objects<StoredDeskLayout>(showId, "user_layout") : Promise.resolve([]),
        client.objects<StoredStageLayout>(showId, "stage_layout"),
        client.objects<PatchLayer>(showId, "patch_layer"),
        client.objects<Record<string, unknown>>(showId, "unresolved_mvr_fixture"),
      ]);
      if (request !== showObjectsRequest.current) return;
      setGroups(nextGroups);
      setPresets(nextPresets);
      setCueObjects(nextCueObjects);
      setOutputRoutes(nextOutputRoutes);
      setDeskLayout(layouts.find((item) => item.id === userId) ?? null);
      setDeskLayoutScope(scope);
      setStageLayout(stageLayouts.find((item) => item.id === "main") ?? null);
      setPatchLayers(
        nextPatchLayers.length
          ? nextPatchLayers
          : [
              {
                kind: "patch_layer",
                id: "default",
                revision: 0,
                updated_at: "",
                body: { id: "default", name: "Default", order: 0 },
              },
            ],
      );
      setUnresolvedMvrFixtures(unresolvedMvr);
    },
    [client],
  );

  const refresh = useCallback(async () => {
    const nextBootstrap = await client.bootstrap();
    setBootstrap(nextBootstrap);
    setPatch(await client.patch());
    if (client.currentSession) setPlaybacks(await client.playbacks());
    setShows(await client.shows());
    const nextConfiguration = await client.configuration();
    setConfiguration(nextConfiguration.configuration);
    setMatter(nextConfiguration.matter);
    setFixtureLibrary(await client.fixtureLibrary());
    setFixtureProfiles(await client.fixtureProfiles().catch(() => []));
    setFixtureProfileWarnings(await client.fixtureProfileWarnings().catch(() => []));
    if (client.currentSession) setMediaServers((await client.mediaServers()).fixtures);
    await loadShowObjects(nextBootstrap.active_show?.id ?? null, client.currentSession?.user.id ?? null);
  }, [client, loadShowObjects]);

  useEffect(() => {
    if (!session || !configuration?.matter_enabled) return;
    let cancelled = false;
    const poll = () => void client.matterStatus().then((next) => { if (!cancelled) setMatter(next); }).catch(() => undefined);
    poll();
    const timer = window.setInterval(poll, 1_000);
    return () => { cancelled = true; window.clearInterval(timer); };
  }, [client, session, configuration?.matter_enabled]);

  useEffect(() => {
    let cancelled = false;
    let unsubscribe = () => {};
    let retryTimer: number | undefined;
    const retry = () => {
      if (cancelled) return;
      window.clearTimeout(retryTimer);
      setStatus("connecting");
      retryTimer = window.setTimeout(() => void start(), 1_500);
    };
    const start = async () => {
      try {
        unsubscribe();
        client.disconnectEvents();
        const initial = await client.bootstrap();
        if (cancelled) return;
        setBootstrap(initial);
        const enabled = initial.users.filter((user) => user.enabled);
        if (!enabled.length) throw new Error("No enabled desk user is configured");
        const remembered = localStorage.getItem("light.operator");
        const user = enabled.find((candidate) => candidate.name === remembered) ?? enabled.find((candidate) => candidate.name === "Operator") ?? enabled[0];
        const screenWindow = new URLSearchParams(window.location.search).has("screen");
        const restored = screenWindow ? (JSON.parse(localStorage.getItem("light.primary-session") ?? "null") as SessionResponse | null) : null;
        const nextSession = restored ?? (await client.login(user.name));
        if (restored) client.restoreSession(restored);
        const nextDeskLock = await client.deskLock();
        localStorage.setItem("light.operator", user.name);
        let effectiveBootstrap = initial;
        if (!initial.active_show && !nextDeskLock.locked) {
          const library = await client.shows();
          const defaultShow = library.find((show) => show.name === "Default Stage Show") ?? (await client.createShow("Default Stage Show"));
          await client.openShow(defaultShow.id, "hold_current");
          effectiveBootstrap = await client.bootstrap();
          setBootstrap(effectiveBootstrap);
        }
        const [nextPatch, nextPlaybacks, programmers, nextShows, nextConfiguration, nextMedia, nextFixtureLibrary, nextFixtureProfiles, nextFixtureProfileWarnings, nextScreens] = await Promise.all([client.patch(), client.playbacks(), client.programmers(), client.shows(), client.configuration(), client.mediaServers(), client.fixtureLibrary(), client.fixtureProfiles().catch(() => []), client.fixtureProfileWarnings().catch(() => []), client.screens()]);
        if (cancelled) return;
        setSession(nextSession);
        setCommandHistory(await client.commandHistory());
        setDeskLock(nextDeskLock);
        setPatch(nextPatch);
        setPlaybacks(nextPlaybacks);
        setShows(nextShows);
        setConfiguration(nextConfiguration.configuration);
        setMatter(nextConfiguration.matter);
        setMediaServers(nextMedia.fixtures);
        setFixtureLibrary(nextFixtureLibrary);
        setFixtureProfiles(nextFixtureProfiles);
        setFixtureProfileWarnings(nextFixtureProfileWarnings);
        setScreens(nextScreens);
        await loadShowObjects(effectiveBootstrap.active_show_error ? null : (effectiveBootstrap.active_show?.id ?? null), nextSession.user.id);
        const ownProgrammer = programmers.find((programmer) => programmer.session_id === nextSession.session_id);
        const restoredCommand = ownProgrammer?.command_line?.trim() || commandTargetModeRef.current;
        const restoredTarget = restoredCommand === "GROUP" ? "GROUP" : restoredCommand === "FIXTURE" ? "FIXTURE" : commandTargetModeRef.current;
        commandTargetModeRef.current = restoredTarget;
        setCommandTargetMode(restoredTarget);
        setCommandLineState(restoredCommand);
        setCommandLinePristine(restoredCommand === restoredTarget);
        setSelectedFixtures(ownProgrammer?.selected ?? []);
        unsubscribe = client.onEvent((event) => {
          if (event.kind === "desk_lock_changed")
            void client
              .deskLock()
              .then(setDeskLock)
              .catch(() => undefined);
          if (event.kind === "desk_action" && (event.payload as { action?: string; session_id?: string; desk_id?: string })?.action) {
            const action = event.payload as { action: string; session_id?: string; desk_id?: string };
            if ((!action.session_id && !action.desk_id) || action.session_id === nextSession.session_id || action.desk_id === nextSession.desk.id)
              window.dispatchEvent(new CustomEvent("light:desk-action", { detail: action.action }));
          }
          if (event.kind === "file_input_action") {
            const action = event.payload as { action?: string; instance_id?: string; session_id?: string };
            if (action.action && action.instance_id && action.session_id === nextSession.session_id)
              window.dispatchEvent(new CustomEvent("light:file-manager-input", { detail: action }));
          }
          if (event.kind === "file_operation_completed")
            window.dispatchEvent(new CustomEvent("light:file-operation", { detail: event.payload }));
          if (event.kind === "command_history"
            && (event.payload as { desk_id?: string }).desk_id === nextSession.desk.id)
            void client.commandHistory().then(setCommandHistory).catch(() => undefined);
          if (event.kind === "group_configuration_requested") {
            const request = event.payload as { group_id?: string; desk_id?: string };
            if (request.group_id && request.desk_id === nextSession.desk.id)
              window.dispatchEvent(new CustomEvent("light:group-configuration", { detail: request.group_id }));
          }
          if (["update_armed", "update_target_requested", "update_target_rejected", "update_targets_requested", "update_settings_requested"].includes(event.kind)) {
            const request = event.payload as { armed?: boolean; desk_id?: string; session_id?: string; target?: UpdateTargetRequest; error?: string };
            if (request.desk_id === nextSession.desk.id) {
              if (event.kind === "update_armed") window.dispatchEvent(new CustomEvent("light:update-armed", { detail: request.armed ?? true }));
              if (event.kind === "update_target_requested" && request.target) window.dispatchEvent(new CustomEvent("light:update-target", { detail: request.target }));
              if (event.kind === "update_target_rejected") window.dispatchEvent(new CustomEvent("light:command-error", { detail: request.error ?? "This playback is not a recordable Update target." }));
              if (event.kind === "update_targets_requested") window.dispatchEvent(new Event("light:update-target-menu"));
              if (event.kind === "update_settings_requested") window.dispatchEvent(new Event("light:update-settings"));
            }
          }
          if (event.kind === "highlight_changed") {
            const request = ++highlightEpoch.current;
            void highlightWrite.current
              .catch(() => undefined)
              .then(() => client.highlight())
              .then((next) => {
                if (request !== highlightEpoch.current) return;
                setHighlight(next);
                if (!highlightErrorSticky.current) setHighlightError(null);
              })
              .catch(() => undefined);
          }
          if (["playback_changed", "playback_page_changed", "show_opened", "show_object_changed", "preload_stored"].includes(event.kind)) {
            void client
              .playbacks()
              .then(setPlaybacks)
              .catch(() => undefined);
          }
          if (["server_configuration_changed", "speed_group_command", "speed_group_action"].includes(event.kind)) {
            void client
              .configuration()
              .then((next) => { setConfiguration(next.configuration); setMatter(next.matter); })
              .catch(() => undefined);
            void client
              .playbacks()
              .then(setPlaybacks)
              .catch(() => undefined);
          }
          if (["screen_configuration_changed", "screen_page_changed", "playback_page_changed", "show_opened"].includes(event.kind))
            void client
              .screens()
              .then(setScreens)
              .catch(() => undefined);
          if (["show_opened", "show_renamed", "show_rolled_back", "server_configuration_changed", "session_started", "session_disconnected", "client_removed", "programmer_changed", "programmer_cleared", "hardware_connection_changed"].includes(event.kind)) {
            const requestedCommandLineEpoch = commandLineEpoch.current;
            // Read the shared desk command line only after every local key/reset write that was
            // already queued when this event arrived. In particular, programmer.execute can
            // emit programmer_changed before its response lets Enter enqueue the reset target;
            // the epoch check below rejects that earlier read. An event arriving after the
            // reset waits for its write here, so it cannot restore the just-executed command.
            void commandLineWrite.current
              .catch(() => undefined)
              .then(() => client.bootstrap())
              .then((next) => {
                setBootstrap(next);
                const own = next.active_programmers.find((programmer) => programmer.session_id === nextSession.session_id);
                if (own) {
                  // A programmer event can be delivered before the command execution response.
                  // Do not let that event's older bootstrap snapshot restore a command after Enter
                  // has already reset it locally. Events without an intervening local edit still
                  // apply, preserving the shared command line for another session on this desk.
                  if (requestedCommandLineEpoch === commandLineEpoch.current) {
                    const restoredCommand = own.command_line?.trim() || commandTargetModeRef.current;
                    setCommandLineState(restoredCommand);
                    setCommandLinePristine(restoredCommand === commandTargetModeRef.current);
                  }
                  setSelectedFixtures(own.selected ?? []);
                }
                void loadShowObjects(next.active_show?.id ?? null, nextSession.user.id);
              })
              .catch(() => undefined);
          }
          if (["show_opened", "show_object_changed"].includes(event.kind)) {
            void client
              .patch()
              .then(setPatch)
              .catch(() => undefined);
          }
          if (["fixture_library_changed", "fixture_profile_changed"].includes(event.kind)) {
            void client
              .fixtureLibrary()
              .then(setFixtureLibrary)
              .catch(() => undefined);
            void client
              .fixtureProfiles()
              .then(setFixtureProfiles)
              .catch(() => undefined);
            void client
              .fixtureProfileWarnings()
              .then(setFixtureProfileWarnings)
              .catch(() => undefined);
          }
          if (["show_uploaded", "show_deleted", "show_opened", "show_renamed", "show_rolled_back"].includes(event.kind))
            void client
              .shows()
              .then(setShows)
              .catch(() => undefined);
          if (["show_opened", "media_thumbnails_refreshed", "media_preview_refreshed", "media_server_offline"].includes(event.kind))
            void client
              .mediaServers()
              .then((next) => setMediaServers(next.fixtures))
              .catch(() => undefined);
          if (["show_object_changed", "preset_stored", "preload_stored"].includes(event.kind))
            void client
              .bootstrap()
              .then((next) => loadShowObjects(next.active_show?.id ?? null, nextSession.user.id))
              .catch(() => undefined);
          if (["show_opened", "show_object_changed"].includes(event.kind))
            void client
              .programmers()
              .then((states) => {
                const own = states.find((programmer) => programmer.session_id === nextSession.session_id);
                if (own) setSelectedFixtures(own.selected);
              })
              .catch(() => undefined);
        });
        await client.connectEvents(retry);
        if (!cancelled) setStatus("connected");
      } catch (reason) {
        if (cancelled) return;
        setError(reason instanceof Error ? reason.message : String(reason));
        setStatus(reason instanceof TypeError ? "offline" : "error");
        retry();
      }
    };
    void start();
    return () => {
      cancelled = true;
      window.clearTimeout(retryTimer);
      unsubscribe();
      client.disconnectEvents();
      void client.closeSession();
    };
  }, [client, loadShowObjects]);

  const persistCommandLine = useCallback(
    (value: string) => {
      const write = commandLineWrite.current.catch(() => undefined).then(() => client.setCommandLine(value));
      commandLineWrite.current = write;
      return write;
    },
    [client],
  );
  const setCommandLine = useCallback(
    (value: string, pristine = false) => {
      const next = value.trim() ? value : commandTargetModeRef.current;
      commandLineEpoch.current += 1;
      setCommandLineState(next);
      setCommandLinePristine(pristine || !value.trim());
      void persistCommandLine(next).catch((reason) => setError(String(reason)));
    },
    [persistCommandLine],
  );
  const resetCommandLine = useCallback(() => setCommandLine("", true), [setCommandLine]);
  const cancelCommandChoice = useCallback(() => {
    setPendingCommandChoice(null);
    resetCommandLine();
  }, [resetCommandLine]);
  const fileRoots = useCallback(() => client.fileRoots(), [client]);
  const fileEntries = useCallback(
    (root: string, path?: string, hidden?: boolean) => client.fileEntries(root, path, hidden),
    [client],
  );

  const value = useMemo<ServerContextValue>(
    () => ({
      status,
      error,
      dismissError: () => setError(null),
      simulateError: (message) => setError(message),
      readServerLogs: () => client.auditEvents(),
      fileRoots,
      fileEntries,
      fileMetadata: (root, path) => client.fileMetadata(root, path),
      readFileNote: (root, path) => client.readFileNote(root, path),
      saveFileNote: (root, path, note) => client.saveFileNote(root, path, note),
      readTextFile: (root, path) => client.readTextFile(root, path),
      saveTextFile: (root, path, text, revision) => client.saveTextFile(root, path, text, revision),
      fileOperation: (root, input) => client.fileOperation(root, input),
      fileContent: (root, path) => client.fileContent(root, path),
      fileStreamUrl: (root, path) => client.fileStreamUrl(root, path),
      fileThumbnail: (root, path, maxSize) => client.fileThumbnail(root, path, maxSize),
      claimFileInput: (instanceId, action, origin) => client.claimFileInput(instanceId, action, origin),
      releaseFileInput: (instanceId) => client.releaseFileInput(instanceId),
      bootstrap,
      session,
      deskLock,
      configureDeskLock: async (input) => {
        try {
          setDeskLock(await client.configureDeskLock(input));
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      lockDesk: async () => {
        try {
          setDeskLock(await client.lockDesk());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      unlockDesk: async (pin) => {
        try {
          setDeskLock(await client.unlockDesk(pin));
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      createUser: async (name) => {
        try {
          setError(null);
          const user = await client.createUser(name);
          setBootstrap(await client.bootstrap());
          await client.closeSession();
          localStorage.setItem("light.operator", user.name);
          window.location.reload();
        } catch (caught) {
          setError(caught instanceof Error ? caught.message : String(caught));
        }
      },
      changeUser: async (user) => {
        localStorage.setItem("light.operator", user.name);
        await client.closeSession();
        window.location.reload();
      },
      patch,
      outputRoutes,
      patchLayers,
      playbacks,
      screens,
      saveScreen: async (screen) => {
        try {
          await client.putScreen(screen);
          setScreens(await client.screens());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      deleteScreen: async (id) => {
        try {
          await client.deleteScreen(id);
          setScreens(await client.screens());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setScreenPage: async (id, page) => {
        try {
          await client.setScreenPage(id, page);
          setScreens(await client.screens());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      shows,
      configuration,
      matter,
      fixtureLibrary,
      fixtureProfiles,
      fixtureProfileWarnings,
      mediaServers,
      mediaPreviewUrls,
      groups,
      presets,
      cueObjects,
      deskLayout,
      deskLayoutScope,
      stageLayout,
      unresolvedMvrFixtures,
      commandLine,
      commandTargetMode,
      commandLinePristine,
      commandHistory,
      pendingCommandChoice,
      selectedFixtures,
      selectedGroupId,
      highlight,
      highlightError,
      dismissHighlightError: () => {
        highlightErrorSticky.current = false;
        setHighlightError(null);
      },
      highlightAction: async (action) => {
        const request = ++highlightEpoch.current;
        highlightErrorSticky.current = false;
        setHighlightError(null);
        try {
          const write = client.highlightAction(action);
          highlightWrite.current = write.catch(() => undefined);
          const next = await write;
          if (request === highlightEpoch.current) {
            setHighlight(next);
            highlightErrorSticky.current = false;
            setHighlightError(null);
          }
          return true;
        } catch (reason) {
          const raw = reason instanceof Error ? reason.message : String(reason);
          let message = raw;
          try {
            const parsed = JSON.parse(raw) as { error?: string; message?: string };
            message = parsed.error ?? parsed.message ?? raw;
          } catch {
            // The server may already have returned a plain operator-facing message.
          }
          if (/409|ownership|owned by|another (?:user|operator)/i.test(message)) {
            const owner = highlight?.owner_user_name?.trim();
            message = `Highlight is controlled by ${owner || "another operator"}. ${message}`;
          }
          highlightErrorSticky.current = true;
          setHighlightError(message);
          return false;
        }
      },
      updateSettings: async () => {
        try {
          const settings = await client.updateSettings();
          setError(null);
          return settings;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      saveUpdateSettings: async (settings) => {
        try {
          await client.saveUpdateSettings(settings);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      previewUpdate: async (target, mode) => {
        try {
          const preview = await client.previewUpdate(target, mode);
          setError(null);
          return preview;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      applyUpdate: async (target, mode, expectedRevision, expectedProgrammerRevision) => {
        try {
          const result = await client.applyUpdate(target, mode, expectedRevision, expectedProgrammerRevision);
          await refresh();
          setError(null);
          return result;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      updateTargets: async (filter) => {
        try {
          const entries = await client.updateTargets(filter);
          setError(null);
          return entries;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      refresh,
      setCommandLine,
      resetCommandLine,
      cancelCommandChoice,
      executeCommandLine: async (value = commandLine) => {
        try {
          // Invalidate event refreshes that began while the command was being assembled. A
          // successful execution increments this again when it installs the reset target.
          commandLineEpoch.current += 1;
          // Key presses update the authoritative desk command line. Preserve their order so an
          // older in-flight key cannot arrive after Enter and restore a command that already ran.
          await commandLineWrite.current;
          const toggledTarget = commandTargetAfterEnter(value, commandTargetModeRef.current, commandLinePristine);
          if (toggledTarget) {
            const nextTarget = toggledTarget;
            commandTargetModeRef.current = nextTarget;
            setCommandTargetMode(nextTarget);
            commandLineEpoch.current += 1;
            setCommandLineState(nextTarget);
            setCommandLinePristine(true);
            await client.setCommandTarget(nextTarget);
            await persistCommandLine(nextTarget);
            setError(null);
            return true;
          }
          const result = (await client.executeCommandLine(value)) as
            | {
                programmer?: {
                  selected?: string[];
                  selection_expression?: {
                    type?: string;
                    group_id?: string;
                  } | null;
                };
                pending_choice?: PendingCommandChoice;
              }
            | undefined;
          if (result?.pending_choice) {
            setPendingCommandChoice(result.pending_choice);
            setError(null);
            return true;
          }
          if (result?.programmer?.selected) {
            setSelectedFixtures(result.programmer.selected);
            setSelectedGroupId(result.programmer.selection_expression?.type === "live_group" ? (result.programmer.selection_expression.group_id ?? null) : null);
          }
          setPendingCommandChoice(null);
          const target = defaultCommandLine(commandTargetModeRef.current);
          commandLineEpoch.current += 1;
          setCommandLineState(target);
          setCommandLinePristine(true);
          await persistCommandLine(target);
          setError(null);
          return true;
        } catch (reason) {
          const message = reason instanceof Error ? reason.message : String(reason);
          setError(message);
          window.dispatchEvent(new CustomEvent("light:command-error", { detail: message }));
          return false;
        }
      },
      undoProgrammer: async () => {
        try {
          await client.undoProgrammer();
          await refresh();
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setSelection: async (fixtures) => {
        const previous = selectedFixtures;
        setSelectedFixtures(fixtures);
        setSelectedGroupId(null);
        try {
          await client.setSelection(fixtures);
          setError(null);
        } catch (reason) {
          setSelectedFixtures(previous);
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      selectionGesture: async (source, remove = false) => {
        const previousFixtures = selectedFixtures;
        const previousGroup = selectedGroupId;
        try {
          const result = (await client.selectionGesture(source, remove)) as {
            programmer?: {
              selected?: string[];
              selection_expression?: {
                type?: string;
                items?: Array<{ type?: string; group_id?: string }>;
              } | null;
            };
          };
          const programmer = result.programmer;
          setSelectedFixtures(programmer?.selected ?? []);
          const items = programmer?.selection_expression?.type === "sources"
            ? (programmer.selection_expression.items ?? [])
            : [];
          const only = items.length === 1 ? items[0] : null;
          setSelectedGroupId(only?.type === "live_group" ? (only.group_id ?? null) : null);
          setError(null);
        } catch (reason) {
          setSelectedFixtures(previousFixtures);
          setSelectedGroupId(previousGroup);
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setProgrammer: async (fixtureId, attribute, level) => {
        try {
          await client.setProgrammer(fixtureId, attribute, level);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setProgrammerMany: async (assignments) => {
        try {
          await client.setProgrammerMany(assignments);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      setProgrammerValue: async (fixtureId, attribute, value) => {
        try {
          await client.setProgrammerValue(fixtureId, attribute, value);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      controlFixtureAction: async (fixtureId, actionId, active) => {
        try {
          await client.controlFixtureAction(fixtureId, actionId, active);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      generateFixturePresets: async (fixtureIds) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before generating presets");
          const result = await client.generateFixturePresets(fixtureIds);
          setPresets(await client.objects<StoredPreset>(bootstrap.active_show.id, "preset"));
          setError(null);
          return result;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      releaseProgrammer: async (fixtureId, attribute) => {
        try {
          await client.releaseProgrammer(fixtureId, attribute);
          setBootstrap(await client.bootstrap());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setGroupValue: async (attribute, level) => {
        try {
          if (!selectedGroupId) throw new Error("Select a live group before setting group-relative values");
          await client.setGroupProgrammer(selectedGroupId, attribute, level);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      releaseGroupValue: async (attribute) => {
        try {
          if (!selectedGroupId) throw new Error("Select a live group before releasing group-relative values");
          await client.releaseGroupProgrammer(selectedGroupId, attribute);
          setBootstrap(await client.bootstrap());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setPreloadGroupValue: async (attribute, level) => {
        try {
          if (!selectedGroupId) throw new Error("Select a live group before setting group-relative preload values");
          await client.setPreloadGroup(selectedGroupId, attribute, level);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      playbackAction: async (cueListId, action) => {
        try {
          await client.playbackAction(cueListId, action);
          setPlaybacks(await client.playbacks());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      poolPlaybackAction: async (number, action, input = {}) => {
        try {
          await client.poolPlaybackAction(number, action, input);
          setPlaybacks(await client.playbacks());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      readVirtualPlaybackExclusionZones: () => client.virtualPlaybackExclusionZones(),
      saveVirtualPlaybackExclusionZones: async (surfaceId, zones) => {
        try {
          await client.saveVirtualPlaybackExclusionZones(surfaceId, zones);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      setPlaybackPage: async (page) => {
        if (!playbacks?.desk) return;
        try {
          await client.setPlaybackPage(playbacks.desk.id, page);
          setPlaybacks(await client.playbacks());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      savePlaybackPage: async (page) => {
        if (!bootstrap?.active_show) return false;
        try {
          const pages = await client.objects<import("./types").PlaybackPage>(bootstrap.active_show.id, "playback_page");
          const existing = pages.find((item) => item.body.number === page.number);
          if (!existing) {
            for (const loadedPage of playbacks?.pages ?? []) {
              if (loadedPage.number === page.number || pages.some((item) => item.body.number === loadedPage.number)) continue;
              await client.putObject(bootstrap.active_show.id, "playback_page", String(loadedPage.number), loadedPage, 0);
            }
          }
          await client.putObject(bootstrap.active_show.id, "playback_page", String(page.number), page, existing?.revision ?? 0);
          setPlaybacks(await client.playbacks());
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      updateControlDesk: async (desk) => {
        try {
          const updated = await client.updateControlDesk(desk);
          setSession((current) => (current ? { ...current, desk: updated } : current));
          setBootstrap(await client.bootstrap());
          setPlaybacks((current) => (current ? { ...current, desk: updated } : current));
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      selectControlDesk: (id) => {
        localStorage.setItem("light.control-desk", id);
        window.location.reload();
      },
      removeClient: async (deskId) => {
        try {
          await client.removeClient(deskId);
          setBootstrap(await client.bootstrap());
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      savePlaybackDefinition: async (playback) => {
        if (!bootstrap?.active_show) return;
        try {
          const objects = await client.objects<import("./types").PlaybackDefinition>(bootstrap.active_show.id, "playback");
          const existing = objects.find((item) => item.body.number === playback.number);
          await client.putObject(bootstrap.active_show.id, "playback", String(playback.number), playback, existing?.revision ?? 0);
          await refresh();
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      savePlaybackSlot: async (page, slot, playback) => {
        if (!bootstrap?.active_show) return false;
        try {
          const [pages, definitions] = await Promise.all([
            client.objects<import("./types").PlaybackPage>(bootstrap.active_show.id, "playback_page"),
            client.objects<import("./types").PlaybackDefinition>(bootstrap.active_show.id, "playback"),
          ]);
          const pageObject = pages.find((item) => item.body.number === page);
          const mappedNumber = pageObject?.body.slots[String(slot)];
          const playbackObject = mappedNumber == null ? undefined : definitions.find((item) => item.body.number === mappedNumber);
          await client.savePlaybackSlot(page, slot, playback, playbackObject?.revision ?? 0, pageObject?.revision ?? 0);
          setPlaybacks(await client.playbacks());
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      clearPlaybackSlot: async (page, slot) => {
        if (!bootstrap?.active_show) return false;
        try {
          const [pages, definitions] = await Promise.all([
            client.objects<import("./types").PlaybackPage>(bootstrap.active_show.id, "playback_page"),
            client.objects<import("./types").PlaybackDefinition>(bootstrap.active_show.id, "playback"),
          ]);
          const pageObject = pages.find((item) => item.body.number === page);
          const mappedNumber = pageObject?.body.slots[String(slot)];
          if (!pageObject || mappedNumber == null) return true;
          const playbackObject = definitions.find((item) => item.body.number === mappedNumber);
          if (!playbackObject) return false;
          await client.clearPlaybackSlot(page, slot, playbackObject.revision, pageObject.revision);
          setPlaybacks(await client.playbacks());
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      saveCueList: async (cueList, revision) => {
        if (!bootstrap?.active_show) return false;
        try {
          await client.putObject(bootstrap.active_show.id, "cue_list", cueList.id, cueList, revision);
          await refresh();
          await loadShowObjects(bootstrap.active_show.id, session?.user.id ?? null);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      unassignPagePlayback: async (pageNumber, slot) => {
        if (!bootstrap?.active_show) return false;
        try {
          const pages = await client.objects<import("./types").PlaybackPage>(bootstrap.active_show.id, "playback_page");
          const page = pages.find((item) => item.body.number === pageNumber);
          if (!page || page.body.slots[String(slot)] == null) return true;
          const playbackNumber = page.body.slots[String(slot)];
          await client.poolPlaybackAction(playbackNumber, "off").catch(() => undefined);
          const slots = { ...page.body.slots };
          delete slots[String(slot)];
          await client.putObject(bootstrap.active_show.id, "playback_page", page.id, { ...page.body, slots }, page.revision);
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      readDmx: () => client.dmx(),
      readVisualization: (preload = false) => client.visualization(preload),
      setDmxOverride: async (universe, address, rawValue) => {
        try {
          await client.setDmxOverride(universe, address, rawValue);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveOutputRoute: async (id, route, revision) => {
        if (!bootstrap?.active_show) return false;
        try {
          await client.putObject(bootstrap.active_show.id, "route", id, route, revision);
          setPatch(await client.patch());
          setOutputRoutes(await client.objects<OutputRoute>(bootstrap.active_show.id, "route"));
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      deleteOutputRoute: async (id, revision) => {
        if (!bootstrap?.active_show) return false;
        try {
          await client.deleteObject(bootstrap.active_show.id, "route", id, revision);
          setPatch(await client.patch());
          setOutputRoutes(await client.objects<OutputRoute>(bootstrap.active_show.id, "route"));
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      createShow: async (name) => {
        try {
          await client.createShow(name);
          setShows(await client.shows());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveShowAs: async (name) => {
        try {
          let created: ShowEntry;
          let shouldOpen = true;
          if (bootstrap?.active_show && /^New Empty Show(?: [1-9]\d*)?$/.test(bootstrap.active_show.name)) {
            created = await client.renameShow(bootstrap.active_show.id, name);
            shouldOpen = false;
          } else if (bootstrap?.active_show) {
            const blob = await client.downloadShow(bootstrap.active_show.id);
            const bytes = new Uint8Array(await blob.arrayBuffer());
            let binary = "";
            for (const byte of bytes) binary += String.fromCharCode(byte);
            created = await client.createShow(name, btoa(binary), false);
          } else created = await client.createShow(name);
          if (shouldOpen) await client.openShow(created.id, "hold_current");
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      overwriteShow: async (destinationId) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before choosing an overwrite destination");
          if (bootstrap.active_show.id === destinationId) throw new Error("The active show is already that destination");
          const destination = shows.find((show) => show.id === destinationId);
          if (!destination) throw new Error("The overwrite destination is no longer available");
          await client.overwriteShow(bootstrap.active_show.id, destination.id);
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      initializeEmptyShow: async () => {
        try {
          const names = new Set(shows.map((show) => show.name.toLowerCase()));
          let name = "New Empty Show";
          for (let suffix = 2; names.has(name.toLowerCase()); suffix += 1) name = `New Empty Show ${suffix}`;
          const created = await client.createShow(name);
          await client.openShow(created.id, "hold_current");
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      uploadShow: async (file, overwrite = false) => {
        try {
          const bytes = new Uint8Array(await file.arrayBuffer());
          let binary = "";
          for (const byte of bytes) binary += String.fromCharCode(byte);
          await client.createShow(file.name.replace(/\.show$/i, ""), btoa(binary), overwrite);
          setShows(await client.shows());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      openShow: async (id, transition = "safe_blackout") => {
        try {
          await client.openShow(id, transition);
          await refresh();
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      openShowFile: async (rootId, path, name) => {
        try {
          const showName = name.replace(/\.show$/i, "");
          let entry = rootId === "shows"
            ? shows.find((show) => show.name.localeCompare(showName, undefined, { sensitivity: "accent" }) === 0)
            : undefined;
          if (!entry) {
            const blob = await client.fileContent(rootId, path);
            const bytes = new Uint8Array(await blob.arrayBuffer());
            let binary = "";
            for (const byte of bytes) binary += String.fromCharCode(byte);
            entry = await client.createShow(showName, btoa(binary), false);
          }
          await client.openShow(entry.id, "safe_blackout");
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      listShowRevisions: async (id) => {
        try {
          const revisions = await client.showRevisions(id);
          setError(null);
          return revisions;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return [];
        }
      },
      saveShowRevision: async (name) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before saving a named revision");
          const revision = await client.saveShowRevision(bootstrap.active_show.id, name);
          setError(null);
          return revision;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      openShowRevision: async (id, revision) => {
        try {
          await client.openShowRevision(id, revision);
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      rollbackShow: async () => {
        try {
          await client.rollbackShow();
          await refresh();
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      downloadShow: async (show) => {
        try {
          const blob = await client.downloadShow(show.id);
          const url = URL.createObjectURL(blob);
          const anchor = document.createElement("a");
          anchor.href = url;
          anchor.download = `${show.name}.show`;
          anchor.click();
          URL.revokeObjectURL(url);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      previewMvr: (file, showId) => client.previewMvr(file, showId),
      applyMvr: async (token, input) => {
        try {
          const result = await client.applyMvr(token, input);
          await refresh();
          setShows(await client.shows());
          setError(null);
          return result;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          throw reason;
        }
      },
      previewMvrExport: (showId) => client.mvrExportPreview(showId),
      downloadMvr: async (show) => {
        try {
          const blob = await client.downloadMvr(show.id);
          const url = URL.createObjectURL(blob);
          const anchor = document.createElement("a");
          anchor.href = url;
          anchor.download = `${show.name}.mvr`;
          anchor.click();
          URL.revokeObjectURL(url);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveConfiguration: async (next) => {
        try {
          const result = await client.updateConfiguration(next);
          setConfiguration(result.configuration);
          setMatter(result.matter);
          setError(null);
          return result.requires_restart;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      setControlTiming: async (input) => {
        if (!configuration) return;
        try {
          const result = await client.updateConfiguration({
            ...configuration,
            ...input,
          });
          setConfiguration(result.configuration);
          setMatter(result.matter);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      speedGroup: (group) => client.speedGroup(group),
      updateSpeedGroup: async (group, next) => {
        try {
          const result = await client.updateSpeedGroup(group, next);
          setError(null);
          return result;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          throw reason;
        }
      },
      observeSpeedGroup: (group, observation) => client.observeSpeedGroup(group, observation),
      speedGroupAction: async (group, input) => {
        try {
          const result = await client.speedGroupAction(group, input);
          setError(null);
          return result;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          throw reason;
        }
      },
      saveDeskLayout: async (layout) => {
        try {
          if (!bootstrap?.active_show || !session) throw new Error("Open a show before saving a Desktop layout");
          const revision = deskLayout?.revision ?? 0;
          await client.putObject(bootstrap.active_show.id, "user_layout", session.user.id, layout, revision);
          const layouts = await client.objects<StoredDeskLayout>(bootstrap.active_show.id, "user_layout");
          setDeskLayout(layouts.find((item) => item.id === session.user.id) ?? null);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveStageLayout: async (layout) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before saving stage positions");
          await client.putObject(bootstrap.active_show.id, "stage_layout", "main", layout, stageLayout?.revision ?? 0);
          const layouts = await client.objects<StoredStageLayout>(bootstrap.active_show.id, "stage_layout");
          setStageLayout(layouts.find((item) => item.id === "main") ?? null);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      applyGroup: async (id) => {
        try {
          const result = (await client.selectGroup(id)) as {
            programmer?: { selected?: string[] };
          };
          setSelectedFixtures(result.programmer?.selected ?? []);
          setSelectedGroupId(id);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      selectGroup: async (id, frozen = false, rule = { type: "all" }) => {
        try {
          const result = (await client.selectGroup(id, frozen, rule)) as {
            programmer?: { selected?: string[] };
          };
          const selected = result.programmer?.selected ?? [];
          setSelectedFixtures(selected);
          setSelectedGroupId(frozen ? null : id);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      selectionMacro: async (rule) => {
        try {
          const result = (await client.selectionMacro(rule)) as {
            programmer?: { selected?: string[] };
          };
          setSelectedFixtures(result.programmer?.selected ?? []);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      alignSelection: async (attribute, mode) => {
        try {
          await client.align(attribute, mode);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      preloadAction: async (action) => {
        try {
          await client.preload(action);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      storePreload: async (input, revision) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before storing preload data");
          await client.storePreload(bootstrap.active_show.id, input, revision);
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      storeDynamic: async (speed, width, direction) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before storing a dynamic");
          const target = cueObjects[0];
          if (!target) throw new Error("Create a Cuelist before storing a dynamic");
          const body = structuredClone(target.body) as {
            cues?: Array<{ phasers?: unknown[] }>;
          };
          const cue = body.cues?.[0];
          if (!cue) throw new Error("The Cuelist needs at least one Cue");
          const phasers = (cue.phasers ??= []);
          phasers.push({
            fixture_ids: selectedGroupId ? [] : selectedFixtures,
            group_ids: selectedGroupId ? [selectedGroupId] : [],
            attribute: "intensity",
            phaser: {
              mode: "relative",
              steps: [
                { position: 0, value: 0, curve_to_next: "sine" },
                { position: 0.5, value: 1, curve_to_next: "sine" },
              ],
              cycles_per_minute: speed,
              phase_start_degrees: direction === "Reverse" ? 360 : 0,
              phase_end_degrees: direction === "Reverse" ? 0 : 360,
              width: width / 100,
            },
          });
          await client.putObject(bootstrap.active_show.id, "cue_list", target.id, body, target.revision);
          await refresh();
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      storePlayback: async (slot, cueListId, explicitPageNumber) => {
        try {
          if (!bootstrap?.active_show || !session) throw new Error("Open a show before storing a Cue");
          const programmers = await client.programmers();
          const programmer = programmers.find((item) => item.session_id === session.session_id);
          if (!programmer) throw new Error("The current programmer is unavailable");
          const recordingBlind = programmer.blind && programmer.preload_capture_programmer !== false;
          const recordValues = recordingBlind ? programmer.preload_pending : programmer.values;
          const recordGroupValues = recordingBlind ? programmer.preload_group_pending : programmer.group_values;
          const objects = await client.objects<import("./types").CueList>(bootstrap.active_show.id, "cue_list");
          const existing = cueListId ? objects.find((item) => item.id === cueListId) : undefined;
          const id = existing?.id ?? crypto.randomUUID();
          const current = existing?.body ?? {
            id,
            name: `Cuelist ${slot + 1}`,
            priority: 50,
            mode: "sequence" as const,
            looped: false,
            intensity_priority_mode: "htp" as const,
            wrap_mode: "off" as const,
            restart_mode: "first_cue" as const,
            force_cue_timing: false,
            disable_cue_timing: false,
            chaser_step_millis: 1000,
            chaser_xfade_millis: 0,
            speed_group: null,
            speed_multiplier: 1,
            cues: [],
          };
          const active = playbacks?.active.find((item) => item.cue_list_id === id);
          const mergeActive = localStorage.getItem("light.store-merge-active-cue") === "true" && active && current.cues[active.cue_index];
          const cueNumber = mergeActive ? current.cues[active.cue_index].number : current.cues.length ? Math.max(...current.cues.map((cue) => cue.number)) + 1 : 1;
          const changes = (
            recordValues as Array<{
              fixture_id: string;
              attribute: string;
              value: import("./types").AttributeValue;
              fade_millis?: number;
              delay_millis?: number;
            }>
          ).map((value) => ({
            fixture_id: value.fixture_id,
            attribute: value.attribute,
            value: value.value,
            ...(value.fade_millis == null ? {} : { fade_millis: value.fade_millis }),
            ...(value.delay_millis == null ? {} : { delay_millis: value.delay_millis }),
          }));
          const group_changes = Object.entries(recordGroupValues ?? {}).flatMap(([group_id, attributes]) =>
            Object.entries(attributes).map(([attribute, value]) => ({
              group_id,
              attribute,
              value: value.value as import("./types").AttributeValue,
              ...(value.fade_millis == null ? {} : { fade_millis: value.fade_millis }),
              ...(value.delay_millis == null ? {} : { delay_millis: value.delay_millis }),
            })),
          );
          const previousCue = mergeActive ? current.cues[active.cue_index] : null;
          const cueOnly = localStorage.getItem("light.store-cue-only") === "true";
          const mergeChanges = <
            T extends {
              fixture_id?: string;
              group_id?: string;
              attribute: string;
            },
          >(
            previous: T[],
            incoming: T[],
          ) => [...previous.filter((old) => !incoming.some((next) => next.attribute === old.attribute && next.fixture_id === old.fixture_id && next.group_id === old.group_id)), ...incoming];
          const restoration = mergeActive ? { changes: [], group_changes: [] } : cueOnlyRestoration(current.cues);
          const cue = {
            number: cueNumber,
            name: previousCue?.name ?? `Cue ${cueNumber}`,
            cue_only: mergeActive ? previousCue?.cue_only ?? cueOnly : cueOnly,
            fade_millis: previousCue?.fade_millis ?? 0,
            delay_millis: previousCue?.delay_millis ?? 0,
            trigger: previousCue?.trigger ?? { type: "manual" },
            changes: mergeActive ? mergeChanges(previousCue?.changes ?? [], changes) : mergeChanges(restoration.changes, changes),
            group_changes: mergeActive ? mergeChanges(previousCue?.group_changes ?? [], group_changes) : mergeChanges(restoration.group_changes, group_changes),
            phasers: previousCue?.phasers ?? [],
          };
          const cues = mergeActive ? current.cues.map((existingCue, index) => (index === active.cue_index ? cue : existingCue)) : [...current.cues, cue];
          await client.putObject(bootstrap.active_show.id, "cue_list", id, { ...current, cues }, existing?.revision ?? 0);
          const playbackObjects = await client.objects<import("./types").PlaybackDefinition>(bootstrap.active_show.id, "playback");
          let playbackObject = playbackObjects.find((item) => item.body.target.type === "cue_list" && item.body.target.cue_list_id === id);
          if (!playbackObject) {
            const used = new Set(playbackObjects.map((item) => item.body.number));
            const number = Array.from({ length: 1000 }, (_, index) => index + 1).find((candidate) => !used.has(candidate));
            if (!number) throw new Error("The Cuelist Pool is full");
            const body: import("./types").PlaybackDefinition = {
              number,
              name: current.name,
              target: { type: "cue_list", cue_list_id: id },
              buttons: ["go", "go_minus", "flash"],
              fader: "master",
              go_activates: true,
              auto_off: true,
              xfade_millis: 0,
            };
            await client.putObject(bootstrap.active_show.id, "playback", String(number), body, 0);
            playbackObject = {
              kind: "playback",
              id: String(number),
              body,
              revision: 1,
              updated_at: "",
            };
          }
          const pageNumber = explicitPageNumber ?? playbacks?.active_page ?? 1;
          const pages = await client.objects<import("./types").PlaybackPage>(bootstrap.active_show.id, "playback_page");
          const pageObject = pages.find((item) => item.body.number === pageNumber);
          const page = pageObject?.body ?? {
            number: pageNumber,
            name: pageNumber === 1 ? "Main" : `Page ${pageNumber}`,
            slots: {},
          };
          await client.putObject(
            bootstrap.active_show.id,
            "playback_page",
            String(pageNumber),
            {
              ...page,
              slots: { ...page.slots, [slot + 1]: playbackObject.body.number },
            },
            pageObject?.revision ?? 0,
          );
          await refresh();
          if (!recordingBlind) {
            await client.poolPlaybackAction(playbackObject.body.number, "go-to", { cue_number: cueNumber });
            await refresh();
          }
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      storeGroup: async (id, name, mode = "overwrite") => {
        try {
          if (!bootstrap?.active_show || !session) throw new Error("Open a show before storing groups");
          const existing = groups.find((item) => item.id === id);
          const programmers = await client.programmers();
          const programmer = programmers.find((item) => item.session_id === session.session_id);
          const expression = programmer?.selection_expression;
          const derived_from =
            expression?.type === "live_group" && expression.group_id
              ? {
                  source_group_id: expression.group_id,
                  rule: expression.rule ?? { type: "all" },
                }
              : (existing?.body.derived_from ?? null);
          const frozen_from =
            expression?.type === "frozen_group" && expression.group_id
              ? {
                  source_group_id: expression.group_id,
                  source_revision: expression.source_revision ?? 0,
                  captured_at: new Date().toISOString(),
                }
              : (existing?.body.frozen_from ?? null);
          const numericId = Number(id);
          const scoped = Object.fromEntries(Object.entries(programmer?.group_values?.[id] ?? {}).map(([attribute, value]) => [attribute, value.value]));
          const programming = {
            ...(existing?.body.programming ?? {}),
            ...scoped,
          };
          const body: StoredGroup = {
            ...existing?.body,
            name,
            fixtures: mode === "merge" ? [...new Set([...(existing?.body.fixtures ?? []), ...selectedFixtures])] : selectedFixtures,
            master: existing?.body.master ?? 1,
            playback_fader: existing?.body.playback_fader ?? (numericId >= 1 && numericId <= 8 ? numericId : null),
            programming,
            derived_from,
            frozen_from,
          };
          await client.putObject(bootstrap.active_show.id, "group", id, body, existing?.revision ?? 0);
          setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group"));
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      updateGroup: async (id, update) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before editing a group");
          const existing = groups.find((item) => item.id === id);
          if (!existing) throw new Error(`Group ${id} does not exist`);
          const name = update.name?.trim();
          if (!name) throw new Error("Group name is required");
          const body: StoredGroup = {
            ...existing.body,
            name,
            color: update.color || undefined,
            icon: update.icon || undefined,
          };
          await client.putObject(bootstrap.active_show.id, "group", id, body, existing.revision);
          setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group"));
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      setGroupMaster: async (id, master) => {
        try {
          await client.setGroupMaster(id, master);
          setGroups((current) => current.map((group) => (group.id === id ? { ...group, body: { ...group.body, master } } : group)));
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setGroupMasterFlash: async (id, value) => {
        try {
          await client.setGroupMasterFlash(id, value);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      undoGroup: async (id) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before undoing a group change");
          const existing = groups.find((item) => item.id === id);
          if (!existing) throw new Error("Group does not exist");
          await client.undoObject(bootstrap.active_show.id, "group", id, existing.revision);
          setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group"));
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      refreshFrozenGroup: async (id) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before refreshing a frozen group");
          const existing = groups.find((item) => item.id === id);
          const sourceId = existing?.body.frozen_from?.source_group_id;
          if (!existing || !sourceId) throw new Error("Group is not a frozen group");
          const result = (await client.selectGroup(sourceId, true)) as {
            programmer?: { selected?: string[] };
          };
          const fixtures = result.programmer?.selected ?? [];
          await client.putObject(
            bootstrap.active_show.id,
            "group",
            id,
            {
              ...existing.body,
              fixtures,
              frozen_from: {
                source_group_id: sourceId,
                source_revision: bootstrap.active_show.revision,
                captured_at: new Date().toISOString(),
              },
            },
            existing.revision,
          );
          setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group"));
          setSelectedFixtures(fixtures);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      detachDerivedGroup: async (id) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before detaching a derived group");
          const existing = groups.find((item) => item.id === id);
          if (!existing?.body.derived_from) throw new Error("Group is not derived");
          await client.putObject(bootstrap.active_show.id, "group", id, { ...existing.body, derived_from: null }, existing.revision);
          setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group"));
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      applyPreset: async (id) => {
        try {
          const result = (await client.applyPreset(id)) as { programmer?: { selected?: string[] } } | undefined;
          if (result?.programmer?.selected) setSelectedFixtures(result.programmer.selected);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      storePreset: async (id, name, mode, family = "All") => {
        try {
          if (!bootstrap?.active_show || !session) throw new Error("Open a show before storing presets");
          const programmers = await client.programmers();
          const programmer = programmers.find((item) => item.session_id === session.session_id);
          if (!programmer) throw new Error("The current programmer is unavailable");
          const values: Record<string, Record<string, unknown>> = {};
          const group_values: Record<string, Record<string, unknown>> = Object.fromEntries(Object.entries(programmer.group_values ?? {}).map(([group, attributes]) => [group, Object.fromEntries(Object.entries(attributes).map(([attribute, value]) => [attribute, value.value]))]));
          const includesAttribute = (attribute: string) => {
            const normalized = family.toLowerCase();
            if (normalized === "all") return true;
            if (normalized === "intensity" || normalized === "dimmer") return attribute === "intensity" || attribute === "dimmer";
            if (normalized === "color") return attribute.startsWith("color.") || attribute === "color";
            if (normalized === "position") return attribute === "pan" || attribute === "tilt" || attribute.startsWith("position.");
            if (normalized === "beam") return /^(beam\.|gobo|prism|iris|focus|zoom|frost|shaper\.)/.test(attribute);
            return attribute === normalized || attribute.startsWith(`${normalized}.`);
          };
          for (const raw of programmer.values) {
            const value = raw as {
              fixture_id: string;
              attribute: string;
              value: unknown;
            };
            if (includesAttribute(value.attribute)) (values[value.fixture_id] ??= {})[value.attribute] = value.value;
          }
          for (const [group, attributes] of Object.entries(group_values)) for (const attribute of Object.keys(attributes)) if (!includesAttribute(attribute)) delete attributes[attribute];
          const existing = presets.find((item) => item.id === id);
          await client.storePreset(bootstrap.active_show.id, id, { name, values, group_values, family }, mode, existing?.revision ?? 0);
          setPresets(await client.objects<StoredPreset>(bootstrap.active_show.id, "preset"));
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      switchUser: (name) => {
        localStorage.setItem("light.operator", name);
        location.reload();
      },
      exportPaperwork: () => {
        const payload = {
          generated_at: new Date().toISOString(),
          show: bootstrap?.active_show,
          patch,
          cue_lists: playbacks?.cue_lists,
          groups: groups.map((item) => item.body),
          presets: presets.map((item) => ({
            id: item.id,
            name: item.body.name,
            fixtures: Object.keys(item.body.values).length,
          })),
        };
        const blob = new Blob([JSON.stringify(payload, null, 2)], {
          type: "application/json",
        });
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = `${bootstrap?.active_show?.name ?? "show"}-paperwork.json`;
        anchor.click();
        URL.revokeObjectURL(url);
      },
      shutdownServer: async () => {
        try {
          await client.shutdown();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      clearProgrammer: async (sessionId) => {
        try {
          await client.clearProgrammer(sessionId);
          if (sessionId === session?.session_id) {
            setSelectedFixtures([]);
            setSelectedGroupId(null);
            setCommandLineState(commandTargetModeRef.current);
            setCommandLinePristine(true);
          }
          setBootstrap(await client.bootstrap());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      clearProgrammerValues: async () => {
        try {
          await client.clearProgrammerValues();
          setBootstrap(await client.bootstrap());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setMaster: async (grandMaster, blackout) => {
        try {
          await client.setMaster({ grand_master: grandMaster, blackout });
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setDeskToken: (token) => {
        client.setDeskToken(token);
        location.reload();
      },
      setServerUrl: (url) => {
        try {
          saveServerUrl(url);
          location.reload();
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      refreshMediaPreview: async (fixtureId, source = 0) => {
        try {
          await client.refreshMediaPreview(fixtureId, source);
          const blob = await client.mediaPreview(fixtureId, source);
          const url = URL.createObjectURL(blob);
          setMediaPreviewUrls((current) => {
            const previous = current[fixtureId];
            if (previous) URL.revokeObjectURL(previous);
            const next = { ...current, [fixtureId]: url };
            mediaPreviewUrlsRef.current = next;
            return next;
          });
          setMediaServers((await client.mediaServers()).fixtures);
          setError(null);
          return true;
        } catch (reason) {
          setMediaServers((await client.mediaServers().catch(() => ({ fixtures: mediaServers }))).fixtures);
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      refreshMediaThumbnails: async (fixtureId, elements) => {
        try {
          await client.refreshMediaThumbnails(fixtureId, elements);
          setMediaServers((await client.mediaServers()).fixtures);
          setError(null);
        } catch (reason) {
          setMediaServers((await client.mediaServers().catch(() => ({ fixtures: mediaServers }))).fixtures);
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      configureMediaServer: async (fixtureId, ipAddress, port = 4811) => {
        try {
          if (!bootstrap?.active_show) throw new Error("Open a show before configuring media servers");
          const fixtures = await client.objects<import("./types").PatchedFixture>(bootstrap.active_show.id, "patched_fixture");
          const object = fixtures.find((candidate) => candidate.body.fixture_id === fixtureId);
          if (!object) throw new Error("Patched fixture object was not found");
          const direct_control = ipAddress ? { protocol: "citp" as const, ip_address: ipAddress, port } : null;
          await client.putObject(bootstrap.active_show.id, "patched_fixture", object.id, { ...object.body, direct_control }, object.revision);
          setPatch(await client.patch());
          setMediaServers((await client.mediaServers()).fixtures);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveFixtureDefinition: async (definition) => {
        try {
          await client.putFixtureDefinition(definition);
          setFixtureLibrary(await client.fixtureLibrary());
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      deleteFixtureDefinition: async (id, revision) => {
        try {
          await client.deleteFixtureDefinition(id, revision);
          setFixtureLibrary(await client.fixtureLibrary());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveFixtureProfile: async (profile, expectedRevision) => {
        try {
          const saved = await client.putFixtureProfile(profile, expectedRevision);
          setFixtureProfiles(await client.fixtureProfiles());
          setFixtureProfileWarnings(await client.fixtureProfileWarnings());
          setFixtureLibrary(await client.fixtureLibrary());
          setError(null);
          return saved;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          throw reason;
        }
      },
      deleteFixtureProfile: async (id, revision) => {
        try {
          await client.deleteFixtureProfile(id, revision);
          setFixtureProfiles(await client.fixtureProfiles());
          setFixtureProfileWarnings(await client.fixtureProfileWarnings());
          setFixtureLibrary(await client.fixtureLibrary());
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      fixtureProfileRevisions: (id) => client.fixtureProfileRevisions(id),
      saveFixtureProfileSourceGdtf: async (id, revision, source) => {
        try {
          await client.putFixtureProfileSourceGdtf(id, revision, source);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      importFixturePackage: async (source) => {
        try {
          const imported = await client.importFixturePackage(source);
          setFixtureProfiles(await client.fixtureProfiles());
          setFixtureProfileWarnings(await client.fixtureProfileWarnings());
          setFixtureLibrary(await client.fixtureLibrary());
          setError(null);
          return imported;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          throw reason;
        }
      },
      exportFixturePackage: (id, revision) => client.exportFixturePackage(id, revision),
      patchFixture: async (input) => {
        try {
          if (!bootstrap?.active_show) throw new Error("No active show is available");
          if ((input.universe == null) !== (input.address == null)) throw new Error("Universe and address must both be set or both be empty");
          if (input.universe != null && input.address != null && (input.universe < 1 || input.address < 1 || input.address + input.definition.footprint - 1 > 512)) throw new Error("The fixture must fit within universe addresses 1–512");
          const fixture_id = crypto.randomUUID();
          const body = {
            fixture_id,
            fixture_number: input.fixture_number,
            name: input.name,
            definition: input.definition,
            universe: input.universe,
            address: input.address,
            split_patches: input.split_patches ?? [],
            highlight_overrides: {},
            layer_id: input.layer_id ?? "default",
            direct_control: null,
            location: { x: 0, y: 0, z: 0 },
            rotation: { x: 0, y: 0, z: 0 },
            logical_heads: [],
            multipatch: [],
          };
          await client.putObject(bootstrap.active_show.id, "patched_fixture", fixture_id, body, 0);
          await refresh();
          setError(null);
          return fixture_id;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return null;
        }
      },
      updatePatchedFixture: async (fixtureId, changes) => {
        try {
          if (!bootstrap?.active_show) throw new Error("No active show is available");
          const objects = await client.objects<PatchedFixture>(bootstrap.active_show.id, "patched_fixture");
          const object = objects.find((candidate) => candidate.id === fixtureId);
          if (!object) throw new Error("Patched fixture object was not found");
          await client.putObject(bootstrap.active_show.id, "patched_fixture", fixtureId, { ...object.body, ...changes }, object.revision);
          await refresh();
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
      savePatchLayer: async (layer) => {
        try {
          if (!bootstrap?.active_show) throw new Error("No active show is available");
          const existing = patchLayers.find((item) => item.id === layer.id);
          await client.putObject(bootstrap.active_show.id, "patch_layer", layer.id, layer, existing?.revision ?? 0);
          await loadShowObjects(bootstrap.active_show.id, session?.user.id ?? null);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
        }
      },
    }),
    [
      status,
      error,
      bootstrap,
      session,
      deskLock,
      patch,
      outputRoutes,
      playbacks,
      shows,
      configuration,
      matter,
      fixtureLibrary,
      fixtureProfiles,
      fixtureProfileWarnings,
      mediaServers,
      mediaPreviewUrls,
      groups,
      presets,
      cueObjects,
      deskLayout,
      deskLayoutScope,
      stageLayout,
      unresolvedMvrFixtures,
      commandLine,
      commandTargetMode,
      commandLinePristine,
      commandHistory,
      pendingCommandChoice,
      selectedFixtures,
      selectedGroupId,
      highlight,
      highlightError,
      refresh,
      setCommandLine,
      resetCommandLine,
      cancelCommandChoice,
      fileRoots,
      fileEntries,
      client,
    ],
  );

  return <ServerContext.Provider value={value}>{children}</ServerContext.Provider>;
}

export function useServer() {
  const context = useContext(ServerContext);
  if (!context) throw new Error("useServer must be used inside ServerProvider");
  return context;
}
