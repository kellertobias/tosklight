import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type PropsWithChildren } from "react";
import { LightApiClient } from "./LightApiClient";
import type { DeskModel } from "../types";
import type { BootstrapSnapshot, ConnectionStatus, DeskConfiguration, DmxSnapshot, MediaServerFixture, PatchSnapshot, PlaybackSnapshot, SessionResponse, ShowEntry, StoredGroup, StoredPreset, VersionedObject } from "./types";

export interface StoredDeskLayout { desks: DeskModel[]; activeDeskId: string }
export interface StoredStageLayout { positions: Record<string, { x: number; y: number; rotation: number }> }

interface ServerContextValue {
  status: ConnectionStatus;
  error: string | null;
  bootstrap: BootstrapSnapshot | null;
  session: SessionResponse | null;
  patch: PatchSnapshot | null;
  playbacks: PlaybackSnapshot | null;
  shows: ShowEntry[];
  configuration: DeskConfiguration | null;
  mediaServers: MediaServerFixture[];
  mediaPreviewUrls: Record<string, string>;
  groups: VersionedObject<StoredGroup>[];
  presets: VersionedObject<StoredPreset>[];
  cueObjects: VersionedObject<Record<string, unknown>>[];
  deskLayout: VersionedObject<StoredDeskLayout> | null;
  stageLayout: VersionedObject<StoredStageLayout> | null;
  commandLine: string;
  selectedFixtures: string[];
  selectedGroupId: string | null;
  refresh: () => Promise<void>;
  setCommandLine: (value: string) => void;
  executeCommandLine: () => Promise<void>;
  setSelection: (fixtures: string[]) => Promise<void>;
  setProgrammer: (fixtureId: string, attribute: string, value: number) => Promise<void>;
  setGroupValue: (attribute: string, value: number) => Promise<void>;
  setPreloadGroupValue: (attribute: string, value: number) => Promise<void>;
  playbackAction: (cueListId: string, action: "go" | "back" | "pause" | "release") => Promise<void>;
  readDmx: () => Promise<DmxSnapshot>;
  setDmxOverride: (universe: number, address: number, value: number | null) => Promise<void>;
  createShow: (name: string) => Promise<void>;
  saveShowAs: (name: string) => Promise<void>;
  uploadShow: (file: File, overwrite?: boolean) => Promise<void>;
  openShow: (id: string, transition?: "hold_current" | "timed_fade" | "safe_blackout") => Promise<void>;
  rollbackShow: () => Promise<void>;
  downloadShow: (show: ShowEntry) => Promise<void>;
  saveConfiguration: (configuration: DeskConfiguration) => Promise<boolean>;
  saveDeskLayout: (layout: StoredDeskLayout) => Promise<void>;
  saveStageLayout: (layout: StoredStageLayout) => Promise<void>;
  applyGroup: (id: string) => Promise<void>;
  selectGroup: (id: string, frozen?: boolean, rule?: Record<string, unknown>) => Promise<void>;
  selectionMacro: (rule: Record<string, unknown>) => Promise<void>;
  alignSelection: (attribute: string, mode: "left" | "right" | "center" | "out") => Promise<void>;
  preloadAction: (action: "enter" | "go" | "clear" | "release") => Promise<void>;
  storePreload: (input: { target: "preset" | "cue"; target_id: string; cue_number?: number; name?: string; mode?: "merge" | "overwrite" | "add_missing_fixtures" }, revision: number) => Promise<boolean>;
  storeDynamic: (speed: number, width: number, direction: string) => Promise<void>;
  storeGroup: (id: string, name: string) => Promise<void>;
  setGroupMaster: (id: string, master: number) => Promise<void>;
  undoGroup: (id: string) => Promise<void>;
  refreshFrozenGroup: (id: string) => Promise<void>;
  detachDerivedGroup: (id: string) => Promise<void>;
  applyPreset: (id: string) => Promise<void>;
  storePreset: (id: string, name: string, mode: "Merge" | "Overwrite" | "AddMissingFixtures") => Promise<void>;
  switchUser: (name: string) => void;
  exportPaperwork: () => void;
  shutdownServer: () => Promise<void>;
  clearProgrammer: (sessionId: string) => Promise<void>;
  setMaster: (grandMaster?: number, blackout?: boolean) => Promise<void>;
  setDeskToken: (token: string) => void;
  refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
  refreshMediaThumbnails: (fixtureId: string, elements: number[]) => Promise<void>;
  configureMediaServer: (fixtureId: string, ipAddress: string | null, port?: number) => Promise<void>;
}

const ServerContext = createContext<ServerContextValue | null>(null);

export function ServerProvider({ children }: PropsWithChildren) {
  const client = useRef(new LightApiClient()).current;
  const [status, setStatus] = useState<ConnectionStatus>("connecting");
  const [error, setError] = useState<string | null>(null);
  const [bootstrap, setBootstrap] = useState<BootstrapSnapshot | null>(null);
  const [session, setSession] = useState<SessionResponse | null>(null);
  const [patch, setPatch] = useState<PatchSnapshot | null>(null);
  const [playbacks, setPlaybacks] = useState<PlaybackSnapshot | null>(null);
  const [shows, setShows] = useState<ShowEntry[]>([]);
  const [configuration, setConfiguration] = useState<DeskConfiguration | null>(null);
  const [mediaServers, setMediaServers] = useState<MediaServerFixture[]>([]);
  const [mediaPreviewUrls, setMediaPreviewUrls] = useState<Record<string, string>>({});
  const mediaPreviewUrlsRef = useRef<Record<string, string>>({});
  const [groups, setGroups] = useState<VersionedObject<StoredGroup>[]>([]);
  const [presets, setPresets] = useState<VersionedObject<StoredPreset>[]>([]);
  const [cueObjects, setCueObjects] = useState<VersionedObject<Record<string, unknown>>[]>([]);
  const [deskLayout, setDeskLayout] = useState<VersionedObject<StoredDeskLayout> | null>(null);
  const [stageLayout, setStageLayout] = useState<VersionedObject<StoredStageLayout> | null>(null);
  const [commandLine, setCommandLineState] = useState("");
  const [selectedFixtures, setSelectedFixtures] = useState<string[]>([]);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  useEffect(() => () => { for (const url of Object.values(mediaPreviewUrlsRef.current)) URL.revokeObjectURL(url); }, []);
  useEffect(() => { for (const url of Object.values(mediaPreviewUrlsRef.current)) URL.revokeObjectURL(url); mediaPreviewUrlsRef.current = {}; setMediaPreviewUrls({}); }, [bootstrap?.active_show?.id]);

  const loadShowObjects = useCallback(async (showId: string | null, userId: string | null) => {
    if (!showId) {
      setGroups([]); setPresets([]); setCueObjects([]); setDeskLayout(null); setStageLayout(null);
      return;
    }
    const [nextGroups, nextPresets, nextCueObjects, layouts, stageLayouts] = await Promise.all([
      client.objects<StoredGroup>(showId, "group"),
      client.objects<StoredPreset>(showId, "preset"),
      client.objects<Record<string, unknown>>(showId, "cue_list"),
      userId ? client.objects<StoredDeskLayout>(showId, "user_layout") : Promise.resolve([]),
      client.objects<StoredStageLayout>(showId, "stage_layout"),
    ]);
    setGroups(nextGroups);
    setPresets(nextPresets);
    setCueObjects(nextCueObjects);
    setDeskLayout(layouts.find((item) => item.id === userId) ?? null);
    setStageLayout(stageLayouts.find((item) => item.id === "main") ?? null);
  }, [client]);

  const refresh = useCallback(async () => {
    const nextBootstrap = await client.bootstrap();
    setBootstrap(nextBootstrap);
    setPatch(await client.patch());
    if (client.currentSession) setPlaybacks(await client.playbacks());
    setShows(await client.shows());
    setConfiguration((await client.configuration()).configuration);
    if (client.currentSession) setMediaServers((await client.mediaServers()).fixtures);
    await loadShowObjects(nextBootstrap.active_show?.id ?? null, client.currentSession?.user.id ?? null);
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
        if (!enabled.length) throw new Error("No enabled desk user is configured");
        const remembered = localStorage.getItem("light.operator");
        const user = enabled.find((candidate) => candidate.name === remembered) ?? enabled[0];
        const nextSession = await client.login(user.name);
        localStorage.setItem("light.operator", user.name);
        const [nextPatch, nextPlaybacks, programmers, nextShows, nextConfiguration, nextMedia] = await Promise.all([
          client.patch(), client.playbacks(), client.programmers(), client.shows(), client.configuration(), client.mediaServers(),
        ]);
        if (cancelled) return;
        setSession(nextSession);
        setPatch(nextPatch);
        setPlaybacks(nextPlaybacks);
        setShows(nextShows);
        setConfiguration(nextConfiguration.configuration);
        setMediaServers(nextMedia.fixtures);
        await loadShowObjects(initial.active_show?.id ?? null, nextSession.user.id);
        const ownProgrammer = programmers.find((programmer) => programmer.session_id === nextSession.session_id);
        setCommandLineState(ownProgrammer?.command_line ?? "");
        setSelectedFixtures(ownProgrammer?.selected ?? []);
        unsubscribe = client.onEvent((event) => {
          if (["playback_changed", "show_opened", "show_object_changed"].includes(event.kind)) {
            void client.playbacks().then(setPlaybacks).catch(() => undefined);
          }
          if (["show_opened", "show_rolled_back", "server_configuration_changed", "session_started", "session_disconnected", "programmer_changed", "programmer_cleared"].includes(event.kind)) {
            void client.bootstrap().then((next) => { setBootstrap(next); void loadShowObjects(next.active_show?.id ?? null, nextSession.user.id); }).catch(() => undefined);
          }
          if (["show_opened", "show_object_changed"].includes(event.kind)) {
            void client.patch().then(setPatch).catch(() => undefined);
          }
          if (["show_uploaded", "show_deleted", "show_opened", "show_rolled_back"].includes(event.kind)) void client.shows().then(setShows).catch(() => undefined);
          if (["show_opened", "media_thumbnails_refreshed", "media_preview_refreshed", "media_server_offline"].includes(event.kind)) void client.mediaServers().then((next) => setMediaServers(next.fixtures)).catch(() => undefined);
          if (["show_object_changed", "preset_stored"].includes(event.kind)) void client.bootstrap().then((next) => loadShowObjects(next.active_show?.id ?? null, nextSession.user.id)).catch(() => undefined);
          if (["show_opened", "show_object_changed"].includes(event.kind)) void client.programmers().then((states) => { const own = states.find((programmer) => programmer.session_id === nextSession.session_id); if (own) setSelectedFixtures(own.selected); }).catch(() => undefined);
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
    return () => { cancelled = true; window.clearTimeout(retryTimer); unsubscribe(); client.disconnectEvents(); void client.closeSession(); };
  }, [client, loadShowObjects]);

  const setCommandLine = useCallback((value: string) => {
    setCommandLineState(value);
    void client.setCommandLine(value).catch((reason) => setError(String(reason)));
  }, [client]);

  const value = useMemo<ServerContextValue>(() => ({
    status, error, bootstrap, session, patch, playbacks, shows, configuration, mediaServers, mediaPreviewUrls, groups, presets, cueObjects, deskLayout, stageLayout, commandLine, selectedFixtures, selectedGroupId, refresh,
    setCommandLine,
    executeCommandLine: async () => {
      try {
        const result = await client.executeCommandLine(commandLine) as { programmer?: { selected?: string[] } } | undefined;
        if (result?.programmer?.selected) setSelectedFixtures(result.programmer.selected);
        setError(null);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      }
    },
    setSelection: async (fixtures) => { const previous = selectedFixtures; setSelectedFixtures(fixtures); setSelectedGroupId(null); try { await client.setSelection(fixtures); setError(null); } catch (reason) { setSelectedFixtures(previous); setError(reason instanceof Error ? reason.message : String(reason)); } },
    setProgrammer: async (fixtureId, attribute, level) => { try { await client.setProgrammer(fixtureId, attribute, level); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    setGroupValue: async (attribute, level) => { try { if (!selectedGroupId) throw new Error("Select a live group before setting group-relative values"); await client.setGroupProgrammer(selectedGroupId, attribute, level); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    setPreloadGroupValue: async (attribute, level) => { try { if (!selectedGroupId) throw new Error("Select a live group before setting group-relative preload values"); await client.setPreloadGroup(selectedGroupId, attribute, level); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    playbackAction: async (cueListId, action) => { try { await client.playbackAction(cueListId, action); setPlaybacks(await client.playbacks()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    readDmx: () => client.dmx(),
    setDmxOverride: async (universe, address, rawValue) => { try { await client.setDmxOverride(universe, address, rawValue); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    createShow: async (name) => { try { await client.createShow(name); setShows(await client.shows()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    saveShowAs: async (name) => { try { if (!bootstrap?.active_show) throw new Error("No active show is available to save as"); const blob = await client.downloadShow(bootstrap.active_show.id); const bytes = new Uint8Array(await blob.arrayBuffer()); let binary = ""; for (const byte of bytes) binary += String.fromCharCode(byte); await client.createShow(name, btoa(binary), false); setShows(await client.shows()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    uploadShow: async (file, overwrite = false) => { try { const bytes = new Uint8Array(await file.arrayBuffer()); let binary = ""; for (const byte of bytes) binary += String.fromCharCode(byte); await client.createShow(file.name.replace(/\.show$/i, ""), btoa(binary), overwrite); setShows(await client.shows()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    openShow: async (id, transition = "safe_blackout") => { try { await client.openShow(id, transition); await refresh(); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    rollbackShow: async () => { try { await client.rollbackShow(); await refresh(); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    downloadShow: async (show) => { try { const blob = await client.downloadShow(show.id); const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `${show.name}.show`; anchor.click(); URL.revokeObjectURL(url); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    saveConfiguration: async (next) => { try { const result = await client.updateConfiguration(next); setConfiguration(result.configuration); setError(null); return result.requires_restart; } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); return false; } },
    saveDeskLayout: async (layout) => { try { if (!bootstrap?.active_show || !session) throw new Error("Open a show before saving a desk layout"); const revision = deskLayout?.revision ?? 0; await client.putObject(bootstrap.active_show.id, "user_layout", session.user.id, layout, revision); const layouts = await client.objects<StoredDeskLayout>(bootstrap.active_show.id, "user_layout"); setDeskLayout(layouts.find((item) => item.id === session.user.id) ?? null); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    saveStageLayout: async (layout) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before saving stage positions"); await client.putObject(bootstrap.active_show.id, "stage_layout", "main", layout, stageLayout?.revision ?? 0); const layouts = await client.objects<StoredStageLayout>(bootstrap.active_show.id, "stage_layout"); setStageLayout(layouts.find((item) => item.id === "main") ?? null); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    applyGroup: async (id) => { try { const result = await client.selectGroup(id) as { programmer?: { selected?: string[] } }; setSelectedFixtures(result.programmer?.selected ?? []); setSelectedGroupId(id); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    selectGroup: async (id, frozen = false, rule = { type: "all" }) => { try { const result = await client.selectGroup(id, frozen, rule) as { programmer?: { selected?: string[] } }; const selected = result.programmer?.selected ?? []; setSelectedFixtures(selected); setSelectedGroupId(frozen ? null : id); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    selectionMacro: async (rule) => { try { const result = await client.selectionMacro(rule) as { programmer?: { selected?: string[] } }; setSelectedFixtures(result.programmer?.selected ?? []); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    alignSelection: async (attribute, mode) => { try { await client.align(attribute, mode); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    preloadAction: async (action) => { try { await client.preload(action); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    storePreload: async (input, revision) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before storing preload data"); await client.storePreload(bootstrap.active_show.id, input, revision); await refresh(); setError(null); return true; } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); return false; } },
    storeDynamic: async (speed, width, direction) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before storing a dynamic"); const target = cueObjects[0]; if (!target) throw new Error("Create a cue list before storing a dynamic"); const body = structuredClone(target.body) as { cues?: Array<{ phasers?: unknown[] }> }; const cue = body.cues?.[0]; if (!cue) throw new Error("The cue list needs at least one cue"); const phasers = cue.phasers ??= []; phasers.push({ fixture_ids: selectedGroupId ? [] : selectedFixtures, group_ids: selectedGroupId ? [selectedGroupId] : [], attribute: "intensity", phaser: { mode: "relative", steps: [{ position: 0, value: 0, curve_to_next: "sine" }, { position: 0.5, value: 1, curve_to_next: "sine" }], cycles_per_minute: speed, phase_start_degrees: direction === "Reverse" ? 360 : 0, phase_end_degrees: direction === "Reverse" ? 0 : 360, width: width / 100 } }); await client.putObject(bootstrap.active_show.id, "cue_list", target.id, body, target.revision); await refresh(); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    storeGroup: async (id, name) => { try { if (!bootstrap?.active_show || !session) throw new Error("Open a show before storing groups"); const existing = groups.find((item) => item.id === id); const programmers = await client.programmers(); const programmer = programmers.find((item) => item.session_id === session.session_id); const expression = programmer?.selection_expression; const derived_from = expression?.type === "live_group" && expression.group_id ? { source_group_id: expression.group_id, rule: expression.rule ?? { type: "all" } } : existing?.body.derived_from ?? null; const frozen_from = expression?.type === "frozen_group" && expression.group_id ? { source_group_id: expression.group_id, source_revision: expression.source_revision ?? 0, captured_at: new Date().toISOString() } : existing?.body.frozen_from ?? null; const numericId = Number(id); const scoped = Object.fromEntries(Object.entries(programmer?.group_values?.[id] ?? {}).map(([attribute, value]) => [attribute, value.value])); const programming = { ...(existing?.body.programming ?? {}), ...scoped }; const body: StoredGroup = { ...existing?.body, name, fixtures: selectedFixtures, master: existing?.body.master ?? 1, playback_fader: existing?.body.playback_fader ?? (numericId >= 1 && numericId <= 8 ? numericId : null), programming, derived_from, frozen_from }; await client.putObject(bootstrap.active_show.id, "group", id, body, existing?.revision ?? 0); setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group")); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    setGroupMaster: async (id, master) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before editing a group master"); const existing = groups.find((item) => item.id === id); if (!existing) throw new Error("Group does not exist"); await client.putObject(bootstrap.active_show.id, "group", id, { ...existing.body, master }, existing.revision); setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group")); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    undoGroup: async (id) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before undoing a group change"); const existing = groups.find((item) => item.id === id); if (!existing) throw new Error("Group does not exist"); await client.undoObject(bootstrap.active_show.id, "group", id, existing.revision); setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group")); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    refreshFrozenGroup: async (id) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before refreshing a frozen group"); const existing = groups.find((item) => item.id === id); const sourceId = existing?.body.frozen_from?.source_group_id; if (!existing || !sourceId) throw new Error("Group is not a frozen group"); const result = await client.selectGroup(sourceId, true) as { programmer?: { selected?: string[] } }; const fixtures = result.programmer?.selected ?? []; await client.putObject(bootstrap.active_show.id, "group", id, { ...existing.body, fixtures, frozen_from: { source_group_id: sourceId, source_revision: bootstrap.active_show.revision, captured_at: new Date().toISOString() } }, existing.revision); setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group")); setSelectedFixtures(fixtures); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    detachDerivedGroup: async (id) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before detaching a derived group"); const existing = groups.find((item) => item.id === id); if (!existing?.body.derived_from) throw new Error("Group is not derived"); await client.putObject(bootstrap.active_show.id, "group", id, { ...existing.body, derived_from: null }, existing.revision); setGroups(await client.objects<StoredGroup>(bootstrap.active_show.id, "group")); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    applyPreset: async (id) => { try { const result = await client.applyPreset(id) as { programmer?: { selected?: string[] } } | undefined; if (result?.programmer?.selected) setSelectedFixtures(result.programmer.selected); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    storePreset: async (id, name, mode) => { try { if (!bootstrap?.active_show || !session) throw new Error("Open a show before storing presets"); const programmers = await client.programmers(); const programmer = programmers.find((item) => item.session_id === session.session_id); if (!programmer) throw new Error("The current programmer is unavailable"); const values: Record<string, Record<string, unknown>> = {}; const group_values: Record<string, Record<string, unknown>> = Object.fromEntries(Object.entries(programmer.group_values ?? {}).map(([group, attributes]) => [group, Object.fromEntries(Object.entries(attributes).map(([attribute, value]) => [attribute, value.value]))])); for (const raw of programmer.values) { const value = raw as { fixture_id: string; attribute: string; value: unknown }; (values[value.fixture_id] ??= {})[value.attribute] = value.value; } const existing = presets.find((item) => item.id === id); await client.storePreset(bootstrap.active_show.id, id, { name, values, group_values }, mode, existing?.revision ?? 0); setPresets(await client.objects<StoredPreset>(bootstrap.active_show.id, "preset")); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    switchUser: (name) => { localStorage.setItem("light.operator", name); location.reload(); },
    exportPaperwork: () => { const payload = { generated_at: new Date().toISOString(), show: bootstrap?.active_show, patch, cue_lists: playbacks?.cue_lists, groups: groups.map((item) => item.body), presets: presets.map((item) => ({ id: item.id, name: item.body.name, fixtures: Object.keys(item.body.values).length })) }; const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" }); const url = URL.createObjectURL(blob); const anchor = document.createElement("a"); anchor.href = url; anchor.download = `${bootstrap?.active_show?.name ?? "show"}-paperwork.json`; anchor.click(); URL.revokeObjectURL(url); },
    shutdownServer: async () => { try { await client.shutdown(); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    clearProgrammer: async (sessionId) => { try { await client.clearProgrammer(sessionId); setBootstrap(await client.bootstrap()); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    setMaster: async (grandMaster, blackout) => { try { await client.setMaster({ grand_master: grandMaster, blackout }); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
    setDeskToken: (token) => { client.setDeskToken(token); location.reload(); },
    refreshMediaPreview: async (fixtureId, source = 0) => { try { await client.refreshMediaPreview(fixtureId, source); const blob = await client.mediaPreview(fixtureId, source); const url = URL.createObjectURL(blob); setMediaPreviewUrls((current) => { const previous = current[fixtureId]; if (previous) URL.revokeObjectURL(previous); const next = { ...current, [fixtureId]: url }; mediaPreviewUrlsRef.current = next; return next; }); setMediaServers((await client.mediaServers()).fixtures); setError(null); return true; } catch (reason) { setMediaServers((await client.mediaServers().catch(() => ({ fixtures: mediaServers }))).fixtures); setError(reason instanceof Error ? reason.message : String(reason)); return false; } },
    refreshMediaThumbnails: async (fixtureId, elements) => { try { await client.refreshMediaThumbnails(fixtureId, elements); setMediaServers((await client.mediaServers()).fixtures); setError(null); } catch (reason) { setMediaServers((await client.mediaServers().catch(() => ({ fixtures: mediaServers }))).fixtures); setError(reason instanceof Error ? reason.message : String(reason)); } },
    configureMediaServer: async (fixtureId, ipAddress, port = 4811) => { try { if (!bootstrap?.active_show) throw new Error("Open a show before configuring media servers"); const fixtures = await client.objects<import("./types").PatchedFixture>(bootstrap.active_show.id, "patched_fixture"); const object = fixtures.find((candidate) => candidate.body.fixture_id === fixtureId); if (!object) throw new Error("Patched fixture object was not found"); const direct_control = ipAddress ? { protocol: "citp" as const, ip_address: ipAddress, port } : null; await client.putObject(bootstrap.active_show.id, "patched_fixture", object.id, { ...object.body, direct_control }, object.revision); setPatch(await client.patch()); setMediaServers((await client.mediaServers()).fixtures); setError(null); } catch (reason) { setError(reason instanceof Error ? reason.message : String(reason)); } },
  }), [status, error, bootstrap, session, patch, playbacks, shows, configuration, mediaServers, mediaPreviewUrls, groups, presets, cueObjects, deskLayout, stageLayout, commandLine, selectedFixtures, selectedGroupId, refresh, setCommandLine, client]);

  return <ServerContext.Provider value={value}>{children}</ServerContext.Provider>;
}

export function useServer() {
  const context = useContext(ServerContext);
  if (!context) throw new Error("useServer must be used inside ServerProvider");
  return context;
}
