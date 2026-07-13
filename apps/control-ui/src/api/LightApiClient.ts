import type {
  BootstrapSnapshot,
  DmxSnapshot,
  DeskConfiguration,
  PatchSnapshot,
  PlaybackSnapshot,
  ServerEvent,
  SessionResponse,
  ScreenConfiguration,
  ScreenSnapshot,
  ShowEntry,
  VersionedObject,
  HelpCatalog,
  HelpTopic,
} from "./types";

type EventListener = (event: ServerEvent) => void;

function browserStorage(): Storage | null {
  const storage = globalThis.localStorage;
  return storage && typeof storage.getItem === "function" ? storage : null;
}

interface CommandResponse {
  protocol_version: number;
  request_id: string;
  ok: boolean;
  revision: number;
  payload?: unknown;
  error?: string;
}

export function defaultServerUrl(location = window.location): string {
  const configured = import.meta.env.VITE_LIGHT_SERVER_URL as string | undefined;
  if (configured) return configured.replace(/\/$/, "");
  if (location.protocol === "tauri:") return (browserStorage()?.getItem("light.server-url") || "http://127.0.0.1:5000").replace(/\/$/, "");
  return location.origin;
}

export function configuredServerUrl() { return defaultServerUrl(); }
export function saveServerUrl(value: string) {
  const url = new URL(value.trim());
  if (url.protocol !== "http:" && url.protocol !== "https:") throw new Error("Server URL must use http or https");
  browserStorage()?.setItem("light.server-url", url.toString().replace(/\/$/, ""));
}

export class LightApiClient {
  private session: SessionResponse | null = null;
  private socket: WebSocket | null = null;
  private listeners = new Set<EventListener>();
  private pending = new Map<string, { resolve: (value: unknown) => void; reject: (error: Error) => void; timer: number }>();
  private deskToken = browserStorage()?.getItem("light.desk-token") ?? "";

  constructor(private readonly baseUrl = defaultServerUrl()) {}

  helpCatalog(): Promise<HelpCatalog> { return this.request("/api/v1/help", {}, false); }
  helpTopic(id: string): Promise<HelpTopic> { return this.request(`/api/v1/help/topics/${encodeURIComponent(id)}`, {}, false); }

  get currentSession() { return this.session; }
  restoreSession(session: SessionResponse) { this.session = session; }
  setDeskToken(token: string) { this.deskToken = token.trim(); const storage = browserStorage(); if (this.deskToken) storage?.setItem("light.desk-token", this.deskToken); else storage?.removeItem("light.desk-token"); }
  private boundaryHeaders(headers = new Headers()) { if (this.deskToken) headers.set("x-light-desk-token", this.deskToken); return headers; }

  async bootstrap(): Promise<BootstrapSnapshot> {
    return this.request("/api/v1/bootstrap", {}, false);
  }

  async login(username: string): Promise<SessionResponse> {
    const storage = browserStorage();
    let clientId = storage?.getItem("light.client-id");
    if (!clientId) { clientId = crypto.randomUUID(); storage?.setItem("light.client-id", clientId); }
    const session = await this.request<SessionResponse>("/api/v1/sessions", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ username, client_id: clientId, desk_id: storage?.getItem("light.control-desk") ?? null }),
    }, false);
    this.session = session;
    storage?.setItem("light.primary-session", JSON.stringify(session));
    if (session.desk) storage?.setItem("light.control-desk", session.desk.id);
    return session;
  }

  async closeSession() {
    const session = this.session;
    if (!session) return;
    try {
      const response = await fetch(`${this.baseUrl}/api/v1/sessions/${session.session_id}`, {
        method: "DELETE", keepalive: true, headers: this.boundaryHeaders(new Headers({ authorization: `Bearer ${session.token}` })),
      });
      if (!response.ok && response.status !== 404) throw new Error(await response.text());
    } finally {
      if (this.session?.session_id === session.session_id) this.session = null;
    }
  }

  createUser(name: string): Promise<import("./types").DeskUser> {
    return this.request("/api/v1/users", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name, enabled: true }),
    });
  }

  patch(): Promise<PatchSnapshot> {
    return this.request("/api/v1/patch", {}, false);
  }

  fixtureLibrary(): Promise<import("./types").FixtureDefinition[]> {
    return this.request("/api/v1/fixture-library", {}, false);
  }

  putFixtureDefinition(definition: import("./types").FixtureDefinition) {
    return this.request<import("./types").FixtureDefinition>("/api/v1/fixture-library", {
      method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify(definition),
    });
  }

  deleteFixtureDefinition(id: string, revision: number) {
    return this.request<void>(`/api/v1/fixture-library/${id}/${revision}`, { method: "DELETE" });
  }

  playbacks(): Promise<PlaybackSnapshot> {
    return this.request("/api/v1/playbacks");
  }
  screens(): Promise<ScreenSnapshot> { return this.request("/api/v1/screens"); }
  putScreen(screen: ScreenConfiguration): Promise<ScreenConfiguration> { return this.request(`/api/v1/screens/${screen.id}`, { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify(screen) }); }
  deleteScreen(id: string): Promise<void> { return this.request(`/api/v1/screens/${id}`, { method: "DELETE" }); }
  setScreenPage(id: string, page: number) { return this.request(`/api/v1/screens/${id}/page`, { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ page }) }); }

  visualization(preload = false): Promise<import("./types").VisualizationSnapshot> {
    return this.request(`/api/v1/visualization${preload ? "?preload=true" : ""}`);
  }

  dmx(): Promise<DmxSnapshot> {
    return this.request("/api/v1/dmx", {}, false);
  }

  mediaServers(): Promise<{ fixtures: import("./types").MediaServerFixture[] }> {
    return this.request("/api/v1/media");
  }

  refreshMediaPreview(fixtureId: string, source = 0, width = 320, height = 180) {
    return this.request<{ fixture_id: string; source: number; format: string; width: number; height: number }>(`/api/v1/media/${fixtureId}/preview/refresh`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ source, width, height }) });
  }

  async mediaPreview(fixtureId: string, source = 0): Promise<Blob> {
    if (!this.session) throw new Error("A server session is required");
    const response = await fetch(`${this.baseUrl}/api/v1/media/${fixtureId}/preview/${source}`, { headers: this.boundaryHeaders(new Headers({ authorization: `Bearer ${this.session.token}` })) });
    if (!response.ok) throw new Error(await response.text());
    return response.blob();
  }

  refreshMediaThumbnails(fixtureId: string, elements: number[], width = 128, height = 72) {
    return this.request<{ fixture_id: string; count: number }>(`/api/v1/media/${fixtureId}/thumbnails/refresh`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ library_type: 1, elements, width, height }) });
  }

  shows(): Promise<ShowEntry[]> {
    return this.request("/api/v1/shows", {}, false);
  }

  createShow(name: string, dataBase64: string | null = null, overwrite = false): Promise<ShowEntry> {
    return this.request("/api/v1/shows", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name, data_base64: dataBase64, overwrite }),
    });
  }

  openShow(id: string, transition: "hold_current" | "timed_fade" | "safe_blackout" = "safe_blackout", transitionMillis?: number): Promise<ShowEntry> {
    return this.request(`/api/v1/shows/${id}/open`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ transition, transition_millis: transitionMillis }),
    });
  }

  rollbackShow(): Promise<ShowEntry> {
    return this.request("/api/v1/shows/rollback", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ transition: "safe_blackout" }),
    });
  }

  async downloadShow(id: string): Promise<Blob> {
    const headers = this.boundaryHeaders(new Headers());
    if (!this.session) throw new Error("A server session is required");
    headers.set("authorization", `Bearer ${this.session.token}`);
    const response = await fetch(`${this.baseUrl}/api/v1/shows/${id}/download`, { headers });
    if (!response.ok) throw new Error(await response.text());
    return response.blob();
  }

  async previewMvr(file: File, showId?: string): Promise<import("./types").MvrImportPreview> {
    if (!this.session) throw new Error("A server session is required");
    const query = showId ? `?show_id=${encodeURIComponent(showId)}` : "";
    const headers = this.boundaryHeaders(new Headers({ authorization: `Bearer ${this.session.token}`, "content-type": "application/octet-stream" }));
    const response = await fetch(`${this.baseUrl}/api/v1/mvr/imports/preview${query}`, { method: "POST", headers, body: file });
    if (!response.ok) throw new Error(await response.text());
    return response.json();
  }

  applyMvr(token: string, input: { new_show?: { name: string; open_after_import: boolean }; existing_show_id?: string; resolutions?: Record<string, { action: string; universe?: number; address?: number }> }): Promise<import("./types").MvrApplyResult> {
    return this.request(`/api/v1/mvr/imports/${token}/apply`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(input) });
  }

  mvrExportPreview(id: string): Promise<import("./types").MvrExportPreview> { return this.request(`/api/v1/shows/${id}/mvr/preview`); }
  async downloadMvr(id: string): Promise<Blob> {
    if (!this.session) throw new Error("A server session is required");
    const headers = this.boundaryHeaders(new Headers({ authorization: `Bearer ${this.session.token}` }));
    const response = await fetch(`${this.baseUrl}/api/v1/shows/${id}/mvr`, { headers });
    if (!response.ok) throw new Error(await response.text());
    return response.blob();
  }

  configuration(): Promise<{ configuration: DeskConfiguration; output_health: import("./types").OutputHealth }> {
    return this.request("/api/v1/configuration", {}, false);
  }

  updateConfiguration(configuration: DeskConfiguration): Promise<{ configuration: DeskConfiguration; requires_restart: boolean }> {
    return this.request("/api/v1/configuration", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(configuration),
    });
  }

  shutdown(): Promise<{ shutting_down: boolean }> {
    return this.request("/api/v1/shutdown", { method: "POST" });
  }

  objects<T>(showId: string, kind: string): Promise<VersionedObject<T>[]> {
    return this.request(`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}`, {}, false);
  }

  putObject<T>(showId: string, kind: string, id: string, body: T, revision: number): Promise<{ revision: number }> {
    return this.request(`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`, {
      method: "PUT",
      headers: { "content-type": "application/json", "if-match": String(revision) },
      body: JSON.stringify(body),
    });
  }

  setDmxOverride(universe: number, address: number, value: number | null) {
    return this.request("/api/v1/dmx/override", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ universe, address, value }),
    });
  }

  programmers() {
    return this.request<import("./types").ProgrammerState[]>("/api/v1/programmers", {}, false);
  }

  auditEvents(after = 0) {
    return this.request<Array<{ revision: number; kind: string; payload: unknown }>>(`/api/v1/audit?after=${after}`);
  }

  clearProgrammer(sessionId: string) {
    return this.request(`/api/v1/programmers/${sessionId}/clear`, { method: "POST" });
  }
  clearProgrammerValues() {
    return this.command("programmer.clear", {});
  }

  selectGroup(groupId: string, frozen = false, rule: Record<string, unknown> = { type: "all" }) {
    return this.command("group.select", { group_id: groupId, frozen, rule });
  }

  selectionMacro(rule: Record<string, unknown>) {
    return this.command("selection.macro", { rule });
  }

  align(attribute: string, mode: "left" | "right" | "center" | "out", from = 0, to = 1) {
    return this.command("programmer.align", { attribute, mode, from, to });
  }
  preload(action: "enter" | "go" | "clear" | "release") { return this.command(`preload.${action}`, {}); }
  setPreloadGroup(groupId: string, attribute: string, value: number) { return this.command("preload.group.set", { group_id: groupId, attribute, value }); }
  storePreload(showId: string, input: { target: "preset" | "cue"; target_id: string; cue_number?: number; name?: string; mode?: "merge" | "overwrite" | "add_missing_fixtures" }, revision: number) {
    return this.request(`/api/v1/shows/${showId}/preload/store`, { method: "POST", headers: { "content-type": "application/json", "if-match": String(revision) }, body: JSON.stringify(input) });
  }

  undoObject(showId: string, kind: string, id: string, revision: number) {
    return this.request(`/api/v1/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}/undo`, { method: "POST", headers: { "if-match": String(revision) } });
  }

  playbackAction(cueListId: string, action: "go" | "back" | "pause" | "release") {
    return this.command(`playback.${action}`, { cue_list_id: cueListId });
  }
  poolPlaybackAction(number: number, action: "on" | "off" | "toggle" | "go" | "go-minus" | "flash" | "master" | "xfade-on" | "xfade-off", input: { value?: number; pressed?: boolean } = {}) {
    return this.request(`/api/v1/playback-pool/${number}/${action}`, { method: action === "master" ? "PUT" : "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(input) });
  }
  setPlaybackPage(deskId: string, page: number) { return this.request(`/api/v1/control-desks/${deskId}/page`, { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ page }) }); }
  updateControlDesk(desk: import("./types").ControlDesk) { return this.request<import("./types").ControlDesk>(`/api/v1/control-desks/${desk.id}`, { method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify(desk) }); }

  setProgrammer(fixtureId: string, attribute: string, value: number) {
    return this.command("programmer.set", { fixture_id: fixtureId, attribute, value });
  }
  setGroupProgrammer(groupId: string, attribute: string, value: number) {
    return this.command("programmer.group.set", { group_id: groupId, attribute, value });
  }
  setGroupMaster(groupId: string, value: number) { return this.command("group.master.set", { group_id: groupId, value }); }
  setGroupMasterFlash(groupId: string, value: number) { return this.command("group.master.flash", { group_id: groupId, value }); }

  setSelection(fixtures: string[]) {
    return this.command("selection.set", { fixtures });
  }

  setCommandLine(value: string) {
    return this.command("programmer.command_line", { value });
  }

  executeCommandLine(value: string) {
    return this.command("programmer.execute", { value });
  }

  applyPreset(presetId: string) {
    return this.command("preset.apply", { preset_id: presetId });
  }

  storePreset(showId: string, presetId: string, preset: { name: string; family?: string; values: Record<string, Record<string, unknown>>; group_values?: Record<string, Record<string, unknown>> }, mode: "merge" | "overwrite" | "add_missing_fixtures", revision: number) {
    return this.request(`/api/v1/shows/${showId}/presets/${encodeURIComponent(presetId)}/store`, {
      method: "POST",
      headers: { "content-type": "application/json", "if-match": String(revision) },
      body: JSON.stringify({ mode, preset }),
    });
  }

  setMaster(payload: { grand_master?: number; blackout?: boolean }) {
    return this.command("master.set", payload);
  }

  onEvent(listener: EventListener) {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  connectEvents(onClose?: () => void) {
    if (!this.session) throw new Error("A session is required before opening events");
    this.disconnectEvents();
    const url = new URL("/api/v1/events", this.baseUrl);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    const protocols = ["light.v1", `light.token.${this.session.token}`];
    if (this.deskToken) protocols.push(`light.desk.b64.${this.base64Url(this.deskToken)}`);
    const socket = new WebSocket(url, protocols);
    this.socket = socket;
    socket.onclose = () => onClose?.();
    socket.addEventListener("message", (message) => {
      const data = JSON.parse(String(message.data)) as ServerEvent | CommandResponse;
      if ("request_id" in data) {
        const pending = this.pending.get(data.request_id);
        if (!pending) return;
        window.clearTimeout(pending.timer);
        this.pending.delete(data.request_id);
        data.ok ? pending.resolve(data.payload) : pending.reject(new Error(data.error ?? "Command failed"));
        return;
      }
      this.listeners.forEach((listener) => listener(data));
    });
    return new Promise<void>((resolve, reject) => {
      socket.addEventListener("open", () => resolve(), { once: true });
      socket.addEventListener("error", () => reject(new Error("WebSocket connection failed")), { once: true });
    });
  }

  disconnectEvents() {
    if (this.socket) {
      this.socket.onclose = null;
      this.socket.close();
    }
    this.socket = null;
  }

  command(command: string, payload: unknown, expectedRevision?: number): Promise<unknown> {
    if (!this.session || !this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("Live server connection is not ready"));
    }
    const requestId = crypto.randomUUID();
    const envelope = {
      protocol_version: 1,
      request_id: requestId,
      session_id: this.session.session_id,
      expected_revision: expectedRevision,
      command,
      payload,
    };
    return new Promise((resolve, reject) => {
      const timer = window.setTimeout(() => {
        this.pending.delete(requestId);
        reject(new Error(`Command timed out: ${command}`));
      }, 5_000);
      this.pending.set(requestId, { resolve, reject, timer });
      this.socket?.send(JSON.stringify(envelope));
    });
  }

  private async request<T>(path: string, init: RequestInit = {}, authenticate = true): Promise<T> {
    const headers = this.boundaryHeaders(new Headers(init.headers));
    if (authenticate) {
      if (!this.session) throw new Error("A server session is required");
      headers.set("authorization", `Bearer ${this.session.token}`);
    }
    const response = await fetch(`${this.baseUrl}${path}`, { ...init, headers });
    if (!response.ok) {
      const body = await response.text();
      throw new Error(body || `${response.status} ${response.statusText}`);
    }
    if (response.status === 204) return undefined as T;
    return response.json() as Promise<T>;
  }
  private base64Url(value: string) { const bytes = new TextEncoder().encode(value); let binary = ""; for (const byte of bytes) binary += String.fromCharCode(byte); return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, ""); }
}
