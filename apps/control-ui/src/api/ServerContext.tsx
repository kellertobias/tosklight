import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PropsWithChildren,
} from "react";
import { LightApiClient, saveServerUrl } from "./LightApiClient";
import type { DeskModel } from "../types";
import type {
  BootstrapSnapshot,
  ConnectionStatus,
  DeskConfiguration,
  FixtureDefinition,
  DmxSnapshot,
  MediaServerFixture,
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
  VersionedObject,
  VisualizationSnapshot,
} from "./types";

export interface StoredDeskLayout {
  desks: DeskModel[];
  activeDeskId: string;
  windowSettings?: Partial<import("../types").WindowSettings>;
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
  bootstrap: BootstrapSnapshot | null;
  session: SessionResponse | null;
  showDirty: boolean;
  createUser: (name: string) => Promise<void>;
  changeUser: (user: import("./types").DeskUser) => Promise<void>;
  patch: PatchSnapshot | null;
  patchLayers: VersionedObject<PatchLayer>[];
  playbacks: PlaybackSnapshot | null;
  screens: ScreenSnapshot | null;
  saveScreen: (screen: ScreenConfiguration) => Promise<void>;
  deleteScreen: (id: string) => Promise<void>;
  setScreenPage: (id: string, page: number) => Promise<void>;
  shows: ShowEntry[];
  configuration: DeskConfiguration | null;
  fixtureLibrary: FixtureDefinition[];
  mediaServers: MediaServerFixture[];
  mediaPreviewUrls: Record<string, string>;
  groups: VersionedObject<StoredGroup>[];
  presets: VersionedObject<StoredPreset>[];
  cueObjects: VersionedObject<Record<string, unknown>>[];
  deskLayout: VersionedObject<StoredDeskLayout> | null;
  stageLayout: VersionedObject<StoredStageLayout> | null;
  unresolvedMvrFixtures: VersionedObject<Record<string, unknown>>[];
  commandLine: string;
  selectedFixtures: string[];
  selectedGroupId: string | null;
  refresh: () => Promise<void>;
  setCommandLine: (value: string) => void;
  executeCommandLine: () => Promise<boolean>;
  setSelection: (fixtures: string[]) => Promise<void>;
  setProgrammer: (
    fixtureId: string,
    attribute: string,
    value: number,
  ) => Promise<void>;
  setGroupValue: (attribute: string, value: number) => Promise<void>;
  setPreloadGroupValue: (attribute: string, value: number) => Promise<void>;
  playbackAction: (
    cueListId: string,
    action: "go" | "back" | "pause" | "release",
  ) => Promise<void>;
  poolPlaybackAction: (number: number, action: "on" | "off" | "toggle" | "go" | "go-minus" | "flash" | "master" | "xfade-on" | "xfade-off", input?: { value?: number; pressed?: boolean }) => Promise<void>;
  setPlaybackPage: (page: number) => Promise<void>;
  updateControlDesk: (desk: import("./types").ControlDesk) => Promise<void>;
  selectControlDesk: (id: string) => void;
  savePlaybackDefinition: (playback: import("./types").PlaybackDefinition) => Promise<void>;
  readDmx: () => Promise<DmxSnapshot>;
  readVisualization: (preload?: boolean) => Promise<VisualizationSnapshot>;
  setDmxOverride: (
    universe: number,
    address: number,
    value: number | null,
  ) => Promise<void>;
  createShow: (name: string) => Promise<void>;
  saveShowAs: (name: string) => Promise<boolean>;
  initializeEmptyShow: () => Promise<boolean>;
  uploadShow: (file: File, overwrite?: boolean) => Promise<void>;
  openShow: (
    id: string,
    transition?: "hold_current" | "timed_fade" | "safe_blackout",
  ) => Promise<void>;
  rollbackShow: () => Promise<void>;
  downloadShow: (show: ShowEntry) => Promise<void>;
  previewMvr: (file: File, showId?: string) => Promise<import("./types").MvrImportPreview>;
  applyMvr: (token: string, input: { new_show?: { name: string; open_after_import: boolean }; existing_show_id?: string; resolutions?: Record<string, { action: string; universe?: number; address?: number }> }) => Promise<import("./types").MvrApplyResult>;
  previewMvrExport: (showId: string) => Promise<import("./types").MvrExportPreview>;
  downloadMvr: (show: ShowEntry) => Promise<void>;
  saveConfiguration: (configuration: DeskConfiguration) => Promise<boolean>;
  setControlTiming: (input: Partial<Pick<DeskConfiguration, "speed_groups_bpm" | "programmer_fade_millis" | "sequence_master_fade_millis">>) => Promise<void>;
  saveDeskLayout: (layout: StoredDeskLayout) => Promise<void>;
  saveStageLayout: (layout: StoredStageLayout) => Promise<void>;
  applyGroup: (id: string) => Promise<void>;
  selectGroup: (
    id: string,
    frozen?: boolean,
    rule?: Record<string, unknown>,
  ) => Promise<void>;
  selectionMacro: (rule: Record<string, unknown>) => Promise<void>;
  alignSelection: (
    attribute: string,
    mode: "left" | "right" | "center" | "out",
  ) => Promise<void>;
  preloadAction: (
    action: "enter" | "go" | "clear" | "release",
  ) => Promise<void>;
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
  storeDynamic: (
    speed: number,
    width: number,
    direction: string,
  ) => Promise<void>;
  storePlayback: (slot: number, cueListId?: string) => Promise<void>;
  storeGroup: (id: string, name: string, mode?: "merge" | "overwrite") => Promise<void>;
  setGroupMaster: (id: string, master: number) => Promise<void>;
  setGroupMasterFlash: (id: string, value: number) => Promise<void>;
  undoGroup: (id: string) => Promise<void>;
  refreshFrozenGroup: (id: string) => Promise<void>;
  detachDerivedGroup: (id: string) => Promise<void>;
  applyPreset: (id: string) => Promise<void>;
  storePreset: (
    id: string,
    name: string,
    mode: "merge" | "overwrite" | "add_missing_fixtures",
    family?: string,
  ) => Promise<void>;
  switchUser: (name: string) => void;
  exportPaperwork: () => void;
  shutdownServer: () => Promise<boolean>;
  clearProgrammer: (sessionId: string) => Promise<void>;
  clearProgrammerValues: () => Promise<void>;
  setMaster: (grandMaster?: number, blackout?: boolean) => Promise<void>;
  setDeskToken: (token: string) => void;
  setServerUrl: (url: string) => void;
  refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
  refreshMediaThumbnails: (
    fixtureId: string,
    elements: number[],
  ) => Promise<void>;
  configureMediaServer: (
    fixtureId: string,
    ipAddress: string | null,
    port?: number,
  ) => Promise<void>;
  saveFixtureDefinition: (definition: FixtureDefinition) => Promise<boolean>;
  deleteFixtureDefinition: (id: string, revision: number) => Promise<void>;
  patchFixture: (input: { name: string; definition: FixtureDefinition; universe: number | null; address: number | null; layer_id?: string }) => Promise<string | null>;
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
  const [showDirty, setShowDirty] = useState(false);
  const [patch, setPatch] = useState<PatchSnapshot | null>(null);
  const [patchLayers, setPatchLayers] = useState<VersionedObject<PatchLayer>[]>([]);
  const [playbacks, setPlaybacks] = useState<PlaybackSnapshot | null>(null);
  const [screens, setScreens] = useState<ScreenSnapshot | null>(null);
  const [shows, setShows] = useState<ShowEntry[]>([]);
  const [configuration, setConfiguration] = useState<DeskConfiguration | null>(
    null,
  );
  const [fixtureLibrary, setFixtureLibrary] = useState<FixtureDefinition[]>([]);
  const [mediaServers, setMediaServers] = useState<MediaServerFixture[]>([]);
  const [mediaPreviewUrls, setMediaPreviewUrls] = useState<
    Record<string, string>
  >({});
  const mediaPreviewUrlsRef = useRef<Record<string, string>>({});
  const [groups, setGroups] = useState<VersionedObject<StoredGroup>[]>([]);
  const [presets, setPresets] = useState<VersionedObject<StoredPreset>[]>([]);
  const [cueObjects, setCueObjects] = useState<
    VersionedObject<Record<string, unknown>>[]
  >([]);
  const [deskLayout, setDeskLayout] =
    useState<VersionedObject<StoredDeskLayout> | null>(null);
  const [stageLayout, setStageLayout] =
    useState<VersionedObject<StoredStageLayout> | null>(null);
  const [unresolvedMvrFixtures, setUnresolvedMvrFixtures] = useState<VersionedObject<Record<string, unknown>>[]>([]);
  const [commandLine, setCommandLineState] = useState("");
  const [selectedFixtures, setSelectedFixtures] = useState<string[]>([]);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  useEffect(
    () => () => {
      for (const url of Object.values(mediaPreviewUrlsRef.current))
        URL.revokeObjectURL(url);
    },
    [],
  );
  useEffect(() => {
    for (const url of Object.values(mediaPreviewUrlsRef.current))
      URL.revokeObjectURL(url);
    mediaPreviewUrlsRef.current = {};
    setMediaPreviewUrls({});
  }, [bootstrap?.active_show?.id]);

  const loadShowObjects = useCallback(
    async (showId: string | null, userId: string | null) => {
      if (!showId) {
        setGroups([]);
        setPresets([]);
        setCueObjects([]);
        setDeskLayout(null);
        setStageLayout(null);
        setUnresolvedMvrFixtures([]);
        return;
      }
      const [nextGroups, nextPresets, nextCueObjects, layouts, stageLayouts, nextPatchLayers, unresolvedMvr] =
        await Promise.all([
          client.objects<StoredGroup>(showId, "group"),
          client.objects<StoredPreset>(showId, "preset"),
          client.objects<Record<string, unknown>>(showId, "cue_list"),
          userId
            ? client.objects<StoredDeskLayout>(showId, "user_layout")
            : Promise.resolve([]),
          client.objects<StoredStageLayout>(showId, "stage_layout"),
          client.objects<PatchLayer>(showId, "patch_layer"),
          client.objects<Record<string, unknown>>(showId, "unresolved_mvr_fixture"),
        ]);
      setGroups(nextGroups);
      setPresets(nextPresets);
      setCueObjects(nextCueObjects);
      setDeskLayout(layouts.find((item) => item.id === userId) ?? null);
      setStageLayout(stageLayouts.find((item) => item.id === "main") ?? null);
      setPatchLayers(nextPatchLayers.length ? nextPatchLayers : [{ kind: "patch_layer", id: "default", revision: 0, updated_at: "", body: { id: "default", name: "Default", order: 0 } }]);
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
    setConfiguration((await client.configuration()).configuration);
    setFixtureLibrary(await client.fixtureLibrary());
    if (client.currentSession)
      setMediaServers((await client.mediaServers()).fixtures);
    await loadShowObjects(
      nextBootstrap.active_show?.id ?? null,
      client.currentSession?.user.id ?? null,
    );
  }, [client, loadShowObjects]);

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
        if (!enabled.length)
          throw new Error("No enabled desk user is configured");
        const remembered = localStorage.getItem("light.operator");
        const user =
          enabled.find((candidate) => candidate.name === remembered) ??
          enabled.find((candidate) => candidate.name === "Operator") ??
          enabled[0];
        const screenWindow = new URLSearchParams(window.location.search).has("screen");
        const restored = screenWindow ? JSON.parse(localStorage.getItem("light.primary-session") ?? "null") as SessionResponse | null : null;
        const nextSession = restored ?? await client.login(user.name);
        if (restored) client.restoreSession(restored);
        localStorage.setItem("light.operator", user.name);
        let effectiveBootstrap = initial;
        if (!initial.active_show) {
          const library = await client.shows();
          const defaultShow = library.find((show) => show.name === "Default Stage Show") ?? await client.createShow("Default Stage Show");
          await client.openShow(defaultShow.id, "hold_current");
          effectiveBootstrap = await client.bootstrap();
          setBootstrap(effectiveBootstrap);
        }
        const [
          nextPatch,
          nextPlaybacks,
          programmers,
          nextShows,
          nextConfiguration,
          nextMedia,
          nextFixtureLibrary,
          nextScreens,
        ] = await Promise.all([
          client.patch(),
          client.playbacks(),
          client.programmers(),
          client.shows(),
          client.configuration(),
          client.mediaServers(),
          client.fixtureLibrary(),
          client.screens(),
        ]);
        if (cancelled) return;
        setSession(nextSession);
        setPatch(nextPatch);
        setPlaybacks(nextPlaybacks);
        setShows(nextShows);
        setConfiguration(nextConfiguration.configuration);
        setMediaServers(nextMedia.fixtures);
        setFixtureLibrary(nextFixtureLibrary);
        setScreens(nextScreens);
        await loadShowObjects(
          effectiveBootstrap.active_show_error ? null : effectiveBootstrap.active_show?.id ?? null,
          nextSession.user.id,
        );
        const ownProgrammer = programmers.find(
          (programmer) => programmer.user_id === nextSession.user.id,
        );
        setCommandLineState(ownProgrammer?.command_line ?? "");
        setSelectedFixtures(ownProgrammer?.selected ?? []);
        unsubscribe = client.onEvent((event) => {
          if (event.kind === "desk_action" && (event.payload as { action?: string })?.action === "set") window.dispatchEvent(new CustomEvent("light:desk-action", { detail: "set" }));
          if (["show_object_changed", "preset_stored", "preload_stored"].includes(event.kind)) setShowDirty(true);
          if (["show_opened", "show_rolled_back"].includes(event.kind)) setShowDirty(false);
          if (
            ["playback_changed", "playback_page_changed", "show_opened", "show_object_changed", "preload_stored"].includes(
              event.kind,
            )
          ) {
            void client
              .playbacks()
              .then(setPlaybacks)
              .catch(() => undefined);
          }
          if (["screen_configuration_changed", "screen_page_changed", "playback_page_changed", "show_opened"].includes(event.kind)) void client.screens().then(setScreens).catch(() => undefined);
          if (
            [
              "show_opened",
              "show_rolled_back",
              "server_configuration_changed",
              "session_started",
              "session_disconnected",
              "programmer_changed",
              "programmer_cleared",
              "hardware_connection_changed",
            ].includes(event.kind)
          ) {
            void client
              .bootstrap()
              .then((next) => {
                setBootstrap(next);
                const own = next.active_programmers.find((programmer) => programmer.user_id === nextSession.user.id);
                if (own) {
                  setCommandLineState(own.command_line ?? "");
                  setSelectedFixtures(own.selected ?? []);
                }
                void loadShowObjects(
                  next.active_show?.id ?? null,
                  nextSession.user.id,
                );
              })
              .catch(() => undefined);
          }
          if (["show_opened", "show_object_changed"].includes(event.kind)) {
            void client
              .patch()
              .then(setPatch)
              .catch(() => undefined);
          }
          if (event.kind === "fixture_library_changed") {
            void client.fixtureLibrary().then(setFixtureLibrary).catch(() => undefined);
          }
          if (
            [
              "show_uploaded",
              "show_deleted",
              "show_opened",
              "show_rolled_back",
            ].includes(event.kind)
          )
            void client
              .shows()
              .then(setShows)
              .catch(() => undefined);
          if (
            [
              "show_opened",
              "media_thumbnails_refreshed",
              "media_preview_refreshed",
              "media_server_offline",
            ].includes(event.kind)
          )
            void client
              .mediaServers()
              .then((next) => setMediaServers(next.fixtures))
              .catch(() => undefined);
          if (["show_object_changed", "preset_stored", "preload_stored"].includes(event.kind))
            void client
              .bootstrap()
              .then((next) =>
                loadShowObjects(
                  next.active_show?.id ?? null,
                  nextSession.user.id,
                ),
              )
              .catch(() => undefined);
          if (["show_opened", "show_object_changed"].includes(event.kind))
            void client
              .programmers()
              .then((states) => {
                const own = states.find(
                  (programmer) =>
                    programmer.user_id === nextSession.user.id,
                );
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

  const setCommandLine = useCallback(
    (value: string) => {
      setCommandLineState(value);
      void client
        .setCommandLine(value)
        .catch((reason) => setError(String(reason)));
    },
    [client],
  );

  const value = useMemo<ServerContextValue>(
    () => ({
      status,
      error,
      dismissError: () => setError(null),
      simulateError: (message) => setError(message),
      readServerLogs: () => client.auditEvents(),
      bootstrap,
      session,
      showDirty,
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
      patchLayers,
      playbacks,
      screens,
      saveScreen: async (screen) => { try { await client.putScreen(screen); setScreens(await client.screens()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
      deleteScreen: async (id) => { try { await client.deleteScreen(id); setScreens(await client.screens()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
      setScreenPage: async (id, page) => { try { await client.setScreenPage(id, page); setScreens(await client.screens()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
      shows,
      configuration,
      fixtureLibrary,
      mediaServers,
      mediaPreviewUrls,
      groups,
      presets,
      cueObjects,
      deskLayout,
      stageLayout,
      unresolvedMvrFixtures,
      commandLine,
      selectedFixtures,
      selectedGroupId,
      refresh,
      setCommandLine,
      executeCommandLine: async () => {
        try {
          const result = (await client.executeCommandLine(commandLine)) as
            { programmer?: { selected?: string[] } } | undefined;
          if (result?.programmer?.selected)
            setSelectedFixtures(result.programmer.selected);
          setError(null);
          return true;
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
          return false;
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
      setProgrammer: async (fixtureId, attribute, level) => {
        try {
          await client.setProgrammer(fixtureId, attribute, level);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setGroupValue: async (attribute, level) => {
        try {
          if (!selectedGroupId)
            throw new Error(
              "Select a live group before setting group-relative values",
            );
          await client.setGroupProgrammer(selectedGroupId, attribute, level);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setPreloadGroupValue: async (attribute, level) => {
        try {
          if (!selectedGroupId)
            throw new Error(
              "Select a live group before setting group-relative preload values",
            );
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
        } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
      },
      setPlaybackPage: async (page) => {
        if (!playbacks?.desk) return;
        try { await client.setPlaybackPage(playbacks.desk.id, page); setPlaybacks(await client.playbacks()); setError(null); }
        catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
      },
      updateControlDesk: async (desk) => { try { const updated = await client.updateControlDesk(desk); setSession((current) => current ? { ...current, desk: updated } : current); setBootstrap(await client.bootstrap()); setPlaybacks((current) => current ? { ...current, desk: updated } : current); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
      selectControlDesk: (id) => { localStorage.setItem("light.control-desk", id); window.location.reload(); },
      savePlaybackDefinition: async (playback) => { if (!bootstrap?.active_show) return; try { const objects = await client.objects<import("./types").PlaybackDefinition>(bootstrap.active_show.id, "playback"); const existing = objects.find((item) => item.body.number === playback.number); await client.putObject(bootstrap.active_show.id, "playback", String(playback.number), playback, existing?.revision ?? 0); await refresh(); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
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
          if (bootstrap?.active_show) {
            const blob = await client.downloadShow(bootstrap.active_show.id);
            const bytes = new Uint8Array(await blob.arrayBuffer());
            let binary = "";
            for (const byte of bytes) binary += String.fromCharCode(byte);
            created = await client.createShow(name, btoa(binary), false);
          } else created = await client.createShow(name);
          await client.openShow(created.id, "hold_current");
          await refresh();
          setShowDirty(false);
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
          setShowDirty(false);
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
          await client.createShow(
            file.name.replace(/\.show$/i, ""),
            btoa(binary),
            overwrite,
          );
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
          setShowDirty(false);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
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
        try { const result = await client.applyMvr(token, input); await refresh(); setShows(await client.shows()); setShowDirty(false); setError(null); return result; }
        catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); throw reason; }
      },
      previewMvrExport: (showId) => client.mvrExportPreview(showId),
      downloadMvr: async (show) => {
        try { const blob = await client.downloadMvr(show.id); const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `${show.name}.mvr`; anchor.click(); URL.revokeObjectURL(url); setError(null); }
        catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
      },
      saveConfiguration: async (next) => {
        try {
          const result = await client.updateConfiguration(next);
          setConfiguration(result.configuration);
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
          const result = await client.updateConfiguration({ ...configuration, ...input });
          setConfiguration(result.configuration);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveDeskLayout: async (layout) => {
        try {
          if (!bootstrap?.active_show || !session)
            throw new Error("Open a show before saving a desk layout");
          const revision = deskLayout?.revision ?? 0;
          await client.putObject(
            bootstrap.active_show.id,
            "user_layout",
            session.user.id,
            layout,
            revision,
          );
          const layouts = await client.objects<StoredDeskLayout>(
            bootstrap.active_show.id,
            "user_layout",
          );
          setDeskLayout(
            layouts.find((item) => item.id === session.user.id) ?? null,
          );
          setError(null);
          setShowDirty(false);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      saveStageLayout: async (layout) => {
        try {
          if (!bootstrap?.active_show)
            throw new Error("Open a show before saving stage positions");
          await client.putObject(
            bootstrap.active_show.id,
            "stage_layout",
            "main",
            layout,
            stageLayout?.revision ?? 0,
          );
          const layouts = await client.objects<StoredStageLayout>(
            bootstrap.active_show.id,
            "stage_layout",
          );
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
          if (!bootstrap?.active_show)
            throw new Error("Open a show before storing preload data");
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
          if (!bootstrap?.active_show)
            throw new Error("Open a show before storing a dynamic");
          const target = cueObjects[0];
          if (!target)
            throw new Error("Create a cue list before storing a dynamic");
          const body = structuredClone(target.body) as {
            cues?: Array<{ phasers?: unknown[] }>;
          };
          const cue = body.cues?.[0];
          if (!cue) throw new Error("The cue list needs at least one cue");
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
          await client.putObject(
            bootstrap.active_show.id,
            "cue_list",
            target.id,
            body,
            target.revision,
          );
          await refresh();
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      storePlayback: async (slot, cueListId) => {
        try {
          if (!bootstrap?.active_show || !session) throw new Error("Open a show before storing a cue");
          const programmers = await client.programmers();
          const programmer = programmers.find((item) => item.user_id === session.user.id);
          if (!programmer) throw new Error("The current programmer is unavailable");
          const objects = await client.objects<import("./types").CueList>(bootstrap.active_show.id, "cue_list");
          const existing = cueListId ? objects.find((item) => item.id === cueListId) : undefined;
          const id = existing?.id ?? crypto.randomUUID();
          const current = existing?.body ?? { id, name: `Playback ${slot + 1}`, priority: 50, mode: "sequence" as const, looped: false, chaser_step_millis: 1000, speed_group: null, cues: [] };
          const active = playbacks?.active.find((item) => item.cue_list_id === id);
          const mergeActive = localStorage.getItem("light.store-merge-active-cue") === "true" && active && current.cues[active.cue_index];
          const cueNumber = mergeActive ? current.cues[active.cue_index].number : current.cues.length ? Math.max(...current.cues.map((cue) => cue.number)) + 1 : 1;
          const changes = (programmer.values as Array<{ fixture_id: string; attribute: string; value: import("./types").AttributeValue }>).map((value) => ({ fixture_id: value.fixture_id, attribute: value.attribute, value: value.value }));
          const group_changes = Object.entries(programmer.group_values ?? {}).flatMap(([group_id, attributes]) => Object.entries(attributes).map(([attribute, value]) => ({ group_id, attribute, value: value.value as import("./types").AttributeValue })));
          const previousCue = mergeActive ? current.cues[active.cue_index] : null;
          const mergeChanges = <T extends { fixture_id?: string; group_id?: string; attribute: string }>(previous: T[], incoming: T[]) => [...previous.filter((old) => !incoming.some((next) => next.attribute === old.attribute && next.fixture_id === old.fixture_id && next.group_id === old.group_id)), ...incoming];
          const cue = { number: cueNumber, name: previousCue?.name ?? `Cue ${cueNumber}`, fade_millis: previousCue?.fade_millis ?? 0, delay_millis: previousCue?.delay_millis ?? 0, trigger: previousCue?.trigger ?? { type: "manual" }, changes: mergeChanges(previousCue?.changes ?? [], changes), group_changes: mergeChanges(previousCue?.group_changes ?? [], group_changes), phasers: previousCue?.phasers ?? [] };
          const cues = mergeActive ? current.cues.map((existingCue, index) => index === active.cue_index ? cue : existingCue) : [...current.cues, cue];
          await client.putObject(bootstrap.active_show.id, "cue_list", id, { ...current, cues }, existing?.revision ?? 0);
          const playbackObjects = await client.objects<import("./types").PlaybackDefinition>(bootstrap.active_show.id, "playback");
          let playbackObject = playbackObjects.find((item) => item.body.target.type === "cue_list" && item.body.target.cue_list_id === id);
          if (!playbackObject) {
            const used = new Set(playbackObjects.map((item) => item.body.number));
            const number = Array.from({ length: 1000 }, (_, index) => index + 1).find((candidate) => !used.has(candidate));
            if (!number) throw new Error("The playback pool is full");
            const body: import("./types").PlaybackDefinition = { number, name: current.name, target: { type: "cue_list", cue_list_id: id }, buttons: ["go", "go_minus", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0 };
            await client.putObject(bootstrap.active_show.id, "playback", String(number), body, 0);
            playbackObject = { kind: "playback", id: String(number), body, revision: 1, updated_at: "" };
          }
          const pageNumber = playbacks?.active_page ?? 1;
          const pages = await client.objects<import("./types").PlaybackPage>(bootstrap.active_show.id, "playback_page");
          const pageObject = pages.find((item) => item.body.number === pageNumber);
          const page = pageObject?.body ?? { number: pageNumber, name: pageNumber === 1 ? "Main" : `Page ${pageNumber}`, slots: {} };
          await client.putObject(bootstrap.active_show.id, "playback_page", String(pageNumber), { ...page, slots: { ...page.slots, [slot + 1]: playbackObject.body.number } }, pageObject?.revision ?? 0);
          await refresh(); setError(null);
        } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
      },
      storeGroup: async (id, name, mode = "overwrite") => {
        try {
          if (!bootstrap?.active_show || !session)
            throw new Error("Open a show before storing groups");
          const existing = groups.find((item) => item.id === id);
          const programmers = await client.programmers();
          const programmer = programmers.find(
            (item) => item.user_id === session.user.id,
          );
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
          const scoped = Object.fromEntries(
            Object.entries(programmer?.group_values?.[id] ?? {}).map(
              ([attribute, value]) => [attribute, value.value],
            ),
          );
          const programming = {
            ...(existing?.body.programming ?? {}),
            ...scoped,
          };
          const body: StoredGroup = {
            ...existing?.body,
            name,
            fixtures: mode === "merge" ? [...new Set([...(existing?.body.fixtures ?? []), ...selectedFixtures])] : selectedFixtures,
            master: existing?.body.master ?? 1,
            playback_fader:
              existing?.body.playback_fader ??
              (numericId >= 1 && numericId <= 8 ? numericId : null),
            programming,
            derived_from,
            frozen_from,
          };
          await client.putObject(
            bootstrap.active_show.id,
            "group",
            id,
            body,
            existing?.revision ?? 0,
          );
          setGroups(
            await client.objects<StoredGroup>(
              bootstrap.active_show.id,
              "group",
            ),
          );
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      setGroupMaster: async (id, master) => {
        try {
          await client.setGroupMaster(id, master);
          setGroups((current) => current.map((group) => group.id === id ? { ...group, body: { ...group.body, master } } : group));
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
          if (!bootstrap?.active_show)
            throw new Error("Open a show before undoing a group change");
          const existing = groups.find((item) => item.id === id);
          if (!existing) throw new Error("Group does not exist");
          await client.undoObject(
            bootstrap.active_show.id,
            "group",
            id,
            existing.revision,
          );
          setGroups(
            await client.objects<StoredGroup>(
              bootstrap.active_show.id,
              "group",
            ),
          );
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      refreshFrozenGroup: async (id) => {
        try {
          if (!bootstrap?.active_show)
            throw new Error("Open a show before refreshing a frozen group");
          const existing = groups.find((item) => item.id === id);
          const sourceId = existing?.body.frozen_from?.source_group_id;
          if (!existing || !sourceId)
            throw new Error("Group is not a frozen group");
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
          setGroups(
            await client.objects<StoredGroup>(
              bootstrap.active_show.id,
              "group",
            ),
          );
          setSelectedFixtures(fixtures);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      detachDerivedGroup: async (id) => {
        try {
          if (!bootstrap?.active_show)
            throw new Error("Open a show before detaching a derived group");
          const existing = groups.find((item) => item.id === id);
          if (!existing?.body.derived_from)
            throw new Error("Group is not derived");
          await client.putObject(
            bootstrap.active_show.id,
            "group",
            id,
            { ...existing.body, derived_from: null },
            existing.revision,
          );
          setGroups(
            await client.objects<StoredGroup>(
              bootstrap.active_show.id,
              "group",
            ),
          );
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      applyPreset: async (id) => {
        try {
          const result = (await client.applyPreset(id)) as
            { programmer?: { selected?: string[] } } | undefined;
          if (result?.programmer?.selected)
            setSelectedFixtures(result.programmer.selected);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
      storePreset: async (id, name, mode, family = "All") => {
        try {
          if (!bootstrap?.active_show || !session)
            throw new Error("Open a show before storing presets");
          const programmers = await client.programmers();
          const programmer = programmers.find(
            (item) => item.user_id === session.user.id,
          );
          if (!programmer)
            throw new Error("The current programmer is unavailable");
          const values: Record<string, Record<string, unknown>> = {};
          const group_values: Record<
            string,
            Record<string, unknown>
          > = Object.fromEntries(
            Object.entries(programmer.group_values ?? {}).map(
              ([group, attributes]) => [
                group,
                Object.fromEntries(
                  Object.entries(attributes).map(([attribute, value]) => [
                    attribute,
                    value.value,
                  ]),
                ),
              ],
            ),
          );
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
          await client.storePreset(
            bootstrap.active_show.id,
            id,
            { name, values, group_values, family },
            mode,
            existing?.revision ?? 0,
          );
          setPresets(
            await client.objects<StoredPreset>(
              bootstrap.active_show.id,
              "preset",
            ),
          );
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
            setCommandLineState("");
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
          setMediaServers(
            (
              await client
                .mediaServers()
                .catch(() => ({ fixtures: mediaServers }))
            ).fixtures,
          );
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
          setMediaServers(
            (
              await client
                .mediaServers()
                .catch(() => ({ fixtures: mediaServers }))
            ).fixtures,
          );
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      },
    configureMediaServer: async (fixtureId, ipAddress, port = 4811) => {
        try {
          if (!bootstrap?.active_show)
            throw new Error("Open a show before configuring media servers");
          const fixtures = await client.objects<
            import("./types").PatchedFixture
          >(bootstrap.active_show.id, "patched_fixture");
          const object = fixtures.find(
            (candidate) => candidate.body.fixture_id === fixtureId,
          );
          if (!object) throw new Error("Patched fixture object was not found");
          const direct_control = ipAddress
            ? { protocol: "citp" as const, ip_address: ipAddress, port }
            : null;
          await client.putObject(
            bootstrap.active_show.id,
            "patched_fixture",
            object.id,
            { ...object.body, direct_control },
            object.revision,
          );
          setPatch(await client.patch());
          setMediaServers((await client.mediaServers()).fixtures);
          setError(null);
        } catch (reason) {
          setError(reason instanceof Error ? reason.message : String(reason));
      }
    },
    saveFixtureDefinition: async (definition) => {
      try { await client.putFixtureDefinition(definition); setFixtureLibrary(await client.fixtureLibrary()); setError(null); return true; }
      catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); return false; }
    },
    deleteFixtureDefinition: async (id, revision) => {
      try { await client.deleteFixtureDefinition(id, revision); setFixtureLibrary(await client.fixtureLibrary()); setError(null); }
      catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); }
    },
    patchFixture: async (input) => {
      try {
        if (!bootstrap?.active_show) throw new Error("No active show is available");
        if ((input.universe == null) !== (input.address == null)) throw new Error("Universe and address must both be set or both be empty");
        if (input.universe != null && input.address != null && (input.universe < 1 || input.address < 1 || input.address + input.definition.footprint - 1 > 512)) throw new Error("The fixture must fit within universe addresses 1–512");
        const fixture_id = crypto.randomUUID();
        const body = {
          fixture_id, name: input.name,
          definition: input.definition,
          universe: input.universe, address: input.address, layer_id: input.layer_id ?? "default", direct_control: null, location: { x: 0, y: 0, z: 0 }, rotation: { x: 0, y: 0, z: 0 }, logical_heads: [], multipatch: [],
        };
        await client.putObject(bootstrap.active_show.id, "patched_fixture", fixture_id, body, 0);
        await refresh(); setError(null); return fixture_id;
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason)); return null;
      }
    },
    updatePatchedFixture: async (fixtureId, changes) => {
      try {
        if (!bootstrap?.active_show) throw new Error("No active show is available");
        const objects = await client.objects<PatchedFixture>(bootstrap.active_show.id, "patched_fixture");
        const object = objects.find((candidate) => candidate.id === fixtureId);
        if (!object) throw new Error("Patched fixture object was not found");
        await client.putObject(bootstrap.active_show.id, "patched_fixture", fixtureId, { ...object.body, ...changes }, object.revision);
        await refresh(); setError(null); return true;
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason)); return false;
      }
    },
    savePatchLayer: async (layer) => {
      try { if (!bootstrap?.active_show) throw new Error("No active show is available"); const existing = patchLayers.find((item) => item.id === layer.id); await client.putObject(bootstrap.active_show.id, "patch_layer", layer.id, layer, existing?.revision ?? 0); await loadShowObjects(bootstrap.active_show.id, session?.user.id ?? null); setError(null); return true; }
      catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); return false; }
    },
    }),
    [
      status,
      error,
      bootstrap,
      session,
      showDirty,
      patch,
      playbacks,
      shows,
      configuration,
      mediaServers,
      mediaPreviewUrls,
      groups,
      presets,
      cueObjects,
      deskLayout,
      stageLayout,
      unresolvedMvrFixtures,
      commandLine,
      selectedFixtures,
      selectedGroupId,
      refresh,
      setCommandLine,
      client,
    ],
  );

  return (
    <ServerContext.Provider value={value}>{children}</ServerContext.Provider>
  );
}

export function useServer() {
  const context = useContext(ServerContext);
  if (!context) throw new Error("useServer must be used inside ServerProvider");
  return context;
}
