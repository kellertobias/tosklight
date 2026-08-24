import type { SoftwareKey } from "@tosklight/ui/programmer-keypad";
import type {
  FixtureLibraryAction,
  FixtureLibraryActionOutcome,
  FixtureDefinitionsSnapshot,
  FixtureLibraryWarningsSnapshot,
  FixtureProfilesSnapshot,
  FixtureProfileRevisionsSnapshot,
  LiveAction,
  PlaybackAction,
  PlaybackActionOutcome,
  PlaybackAddress,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import type { PatchSnapshot as LegacyPatchSnapshot } from "../../../apps/light-desktop/src/api/types";
import { decodePatchSnapshot } from "../../../apps/light-desktop/src/api/patchWire";
import { projectionToPatchedFixture } from "../../../apps/light-desktop/src/features/patch/model";

export interface Session { session_id: string; client_id: string; token: string; desk: { id: string; name: string } }
export interface CommandResponse<T = unknown> { protocol_version: number; request_id: string; ok: boolean; revision: number; payload?: T; error?: string }
export interface FixtureLibrarySnapshot {
  definitions: unknown[];
  profiles: unknown[];
  warnings: string[];
}

export type CommandTarget = "FIXTURE" | "GROUP";
export type CommandKeyPhase = "press" | "release";

export interface CommandLineState {
  text: string;
  target: CommandTarget;
  pristine: boolean;
  revision: number;
  pending_choice: unknown | null;
}

export interface RevisionedCommandLine {
  commandLine: CommandLineState;
  etag: string;
}

interface CommandOperationBase {
  request_id: string;
  command_line: CommandLineState;
}

export type CommandOperationResponse = CommandOperationBase & (
  | {
      outcome: "accepted";
      action: string;
      applied?: number;
      warning?: string;
    }
  | {
      outcome: "choice_required";
      pending_choice: unknown;
    }
  | {
      outcome: "rejected";
      error: string;
    }
);

const WEB_SOCKET_TIMEOUT_MILLIS = 5_000;

export class ApiDriver {
  session?: Session;
  constructor(readonly baseUrl: string) {}

  async login(username = "Operator", deskId: string | null = this.session?.desk.id ?? null): Promise<Session> {
    this.session = await this.request<Session>("POST", "/api/v2/sessions", { username, desk_id: deskId }, false);
    return this.session;
  }

  async shows<T = any>(): Promise<T[]> {
    const snapshot = await this.request<{ shows: T[] }>("GET", "/api/v2/shows");
    return snapshot.shows;
  }

  async showObjects<T = Record<string, unknown>>(
    showId: string,
    kind: string,
  ): Promise<Array<{ id: string; body: T; revision: number }>> {
    const snapshot = await this.request<{
      objects: Array<{ id: string; body: T; revision: number }>;
    }>(
      "GET",
      `/api/v2/objects/${encodeURIComponent(kind)}`,
      undefined,
      true,
      undefined,
      { showId },
    );
    return snapshot.objects;
  }

  async showObject<T = Record<string, unknown>>(
    showId: string,
    kind: string,
    id: string,
  ): Promise<{ id: string; body: T; revision: number } | null> {
    const snapshot = await this.request<{
      object: { id: string; body: T; revision: number } | null;
    }>(
      "GET",
      `/api/v2/objects/${encodeURIComponent(kind)}/${encodeURIComponent(id)}`,
      undefined,
      true,
      undefined,
      { showId },
    );
    return snapshot.object;
  }

  seedShowObject<T = unknown>(
    showId: string,
    kind: string,
    objectId: string,
    body: T,
    expectedRevision = 0,
  ): Promise<{ revision: number; event_sequence: number | null }> {
    return this.request(
      "POST",
      `/api/v2/test/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(objectId)}`,
      {
        expected_revision: expectedRevision,
        action: { type: "put", body },
      },
    );
  }

  deleteSeededShowObject(
    showId: string,
    kind: string,
    objectId: string,
    expectedRevision: number,
  ): Promise<void> {
    return this.request(
      "POST",
      `/api/v2/test/shows/${showId}/objects/${encodeURIComponent(kind)}/${encodeURIComponent(objectId)}`,
      {
        expected_revision: expectedRevision,
        action: { type: "delete" },
      },
    );
  }

  createShow<T = any>(input: { name: string; data_base64?: string | null; overwrite?: boolean }): Promise<T> {
    return this.showResult<T>({
      type: "create",
      data_base64: null,
      overwrite: false,
      ...input,
    });
  }

  openShow<T = any>(
    showId: string,
    input: { transition?: string; transition_millis?: number } = {},
  ): Promise<T> {
    return this.showResult<T>({
      type: "open",
      show_id: showId,
      transition: "safe_blackout",
      transition_millis: null,
      ...input,
    });
  }

  openDefaultShow<T = any>(input: { transition?: string; transition_millis?: number } = {}): Promise<T> {
    return this.showResult<T>({
      type: "open_default",
      transition: "safe_blackout",
      transition_millis: null,
      ...input,
    });
  }

  rollbackShow<T = any>(input: { transition?: string; transition_millis?: number } = {}): Promise<T> {
    return this.showResult<T>({
      type: "rollback",
      transition: "safe_blackout",
      transition_millis: null,
      ...input,
    });
  }

  renameShow<T = any>(showId: string, name: string): Promise<T> {
    return this.showResult<T>({ type: "rename", show_id: showId, name });
  }

  overwriteShow<T = any>(sourceShowId: string, destinationShowId: string): Promise<T> {
    return this.showResult<T>({
      type: "overwrite",
      source_show_id: sourceShowId,
      destination_show_id: destinationShowId,
    });
  }

  async showRevisions<T = any>(showId: string): Promise<T[]> {
    const shows = await this.shows<Array<{ id: string; revisions: T[] }>[number]>();
    return shows.find((show) => show.id === showId)?.revisions ?? [];
  }

  async downloadShow(showId: string): Promise<Buffer> {
    const response = await fetch(
      `${this.baseUrl}/api/v2/shows/${encodeURIComponent(showId)}/download`,
      { headers: { authorization: `Bearer ${this.session?.token}` } },
    );
    if (!response.ok) {
      throw new Error(`Show download failed: ${response.status} ${await response.text()}`);
    }
    return Buffer.from(await response.arrayBuffer());
  }

  saveShowRevision<T = any>(showId: string, name: string): Promise<T> {
    return this.revisionResult<T>({ type: "save_revision", show_id: showId, name });
  }

  openShowRevision<T = any>(
    showId: string,
    revision: number,
    input: { transition?: string; transition_millis?: number } = {},
  ): Promise<T> {
    return this.showResult<T>({
      type: "open_revision",
      show_id: showId,
      revision,
      transition: "safe_blackout",
      transition_millis: null,
      ...input,
    });
  }

  private async showResult<T>(action: Record<string, unknown>): Promise<T> {
    const result = await this.showLibraryAction(action);
    if (result.type !== "show") throw new Error(`Expected show result, received ${result.type}`);
    return result.show as T;
  }

  private async revisionResult<T>(action: Record<string, unknown>): Promise<T> {
    const result = await this.showLibraryAction(action);
    if (result.type !== "revision") throw new Error(`Expected revision result, received ${result.type}`);
    return result.revision as T;
  }

  private async showLibraryAction(action: Record<string, unknown>): Promise<any> {
    const outcome = await this.request<{ result: any }>("POST", "/api/v2/shows", {
      request_id: crypto.randomUUID(),
      action,
    });
    return outcome.result;
  }

  async patch(): Promise<LegacyPatchSnapshot> {
    const snapshot = decodePatchSnapshot(
      await this.request<unknown>("GET", "/api/v2/patch"),
    );
    const profiles = new Map(
      snapshot.profileRevisions.map((profile) => [
        `${profile.profileId}:${profile.profileRevision}`,
        profile,
      ]),
    );
    return {
      revision: snapshot.patchRevision,
      fixtures: snapshot.fixtures.map((fixture) => {
        const profile = profiles.get(
          `${fixture.profileId}:${fixture.profileRevision}`,
        );
        if (!profile) {
          throw new Error(
            `Patch profile ${fixture.profileId} revision ${fixture.profileRevision} is missing`,
          );
        }
        return projectionToPatchedFixture(fixture, profile, () => null);
      }),
      // Output routes remain owned by the output/configuration surface, not the v2 Patch contract.
      routes: [],
    };
  }

  async request<T>(
    method: string,
    path: string,
    body?: unknown,
    authenticate = true,
    revision?: number,
    context?: { showId?: string; deskId?: string },
  ): Promise<T> {
    const normalized = this.v2DeskManagementRequest(method, path, body);
    const response = await this.response(
      normalized.method,
      normalized.path,
      normalized.body,
      authenticate,
      revision,
      context,
    );
    if (response.status === 204) return undefined as T;
    const value = await response.json();
    return value as T;
  }

  private v2DeskManagementRequest(method: string, path: string, body: unknown) {
    if (method === "POST" && /^\/api\/v2\/files\/[^/]+\/operations$/.test(path)) {
      return {
        method,
        path,
        body: { request_id: crypto.randomUUID(), ...(body as object) },
      };
    }
    if (
      method === "PUT" &&
      /^\/api\/v2\/files\/[^/]+\/(?:notes|text)$/.test(path)
    ) {
      return {
        method: "POST",
        path: `${path}/update`,
        body: { request_id: crypto.randomUUID(), ...(body as object) },
      };
    }
    if (method === "POST" && path === "/api/v2/files/input-context") {
      return {
        method,
        path: `${path}/claim`,
        body: { request_id: crypto.randomUUID(), ...(body as object) },
      };
    }
    if (method === "PUT" && path === "/api/v2/configuration") {
      return {
        method: "POST",
        path: "/api/v2/configuration/update",
        body: { request_id: crypto.randomUUID(), patch: body },
      };
    }
    if (method === "PUT" && /^\/api\/v2\/speed-groups\/[^/]+$/.test(path)) {
      const configuration = body as Record<string, unknown> & { enabled?: boolean };
      const { enabled, ...settings } = configuration;
      return {
        method: "POST",
        path: `${path}/settings/update`,
        body: {
          request_id: crypto.randomUUID(),
          source: { type: enabled ? "sound_to_light" : "manual" },
          configuration: settings,
        },
      };
    }
    if (path.startsWith("/api/v2/desk-lock")) {
      if (!this.session) throw new Error("API session is not initialized");
      const suffix = path.slice("/api/v2/desk-lock".length);
      return {
        method:
          suffix === "/lock" && method === "POST"
            ? "GET"
            : method === "PUT"
              ? "POST"
              : method,
        path: `/api/v2/control-desks/${this.session.desk.id}/desk-lock${
          method === "PUT" ? "/update" : suffix
        }`,
        body:
          method === "PUT"
            ? { request_id: crypto.randomUUID(), ...(body as object) }
            : suffix === "/lock" && method === "POST"
              ? undefined
              : body,
      };
    }
    return { method, path, body };
  }

  async fixtureLibrarySnapshot(): Promise<FixtureLibrarySnapshot> {
    const [definitions, profiles, warnings] = await Promise.all([
      this.fixtureDefinitionsSnapshot(),
      this.fixtureProfilesSnapshot(),
      this.fixtureLibraryWarningsSnapshot(),
    ]);
    return {
      definitions: definitions.definitions,
      profiles: profiles.profiles,
      warnings: warnings.warnings,
    };
  }

  fixtureDefinitionsSnapshot(): Promise<FixtureDefinitionsSnapshot> {
    return this.request("GET", "/api/v2/fixture-library/definitions");
  }

  fixtureProfilesSnapshot(): Promise<FixtureProfilesSnapshot> {
    return this.request("GET", "/api/v2/fixture-library/profiles");
  }

  fixtureLibraryWarningsSnapshot(): Promise<FixtureLibraryWarningsSnapshot> {
    return this.request("GET", "/api/v2/fixture-library/warnings");
  }

  async fixtureProfileRevisions<T = unknown>(id: string): Promise<T[]> {
    const snapshot = await this.request<FixtureProfileRevisionsSnapshot>(
      "GET",
      `/api/v2/fixture-library/profiles/${encodeURIComponent(id)}/revisions`,
    );
    return snapshot.profiles as T[];
  }

  async fixtureLibraryAction<T = FixtureLibraryActionOutcome["result"]>(
    action: FixtureLibraryAction,
  ): Promise<T> {
    const outcome = await this.request<FixtureLibraryActionOutcome>(
      "POST",
      "/api/v2/fixture-library",
      { request_id: crypto.randomUUID(), action },
    );
    return outcome.result as T;
  }

  importFixturePackage(source: Uint8Array): Promise<FixtureLibraryActionOutcome["result"]> {
    return this.fixtureLibraryAction({
      type: "import_package",
      package_base64: Buffer.from(source).toString("base64"),
    });
  }

  playbackNumberAction<T = PlaybackActionOutcome>(
    playbackNumber: number,
    action: string,
    input: Record<string, unknown> = {},
  ): Promise<T> {
    return this.playbackHttpAction(
      { kind: "playback", playback_number: playbackNumber },
      action,
      input,
    );
  }

  cueListPlaybackAction<T = PlaybackActionOutcome>(
    cueListId: string,
    action: string,
    input: Record<string, unknown> = {},
  ): Promise<T> {
    return this.playbackHttpAction(
      { kind: "cue_list", cue_list_id: cueListId },
      action,
      input,
    );
  }

  currentPagePlaybackAction<T = PlaybackActionOutcome>(
    slot: number,
    action: string,
    input: Record<string, unknown> = {},
  ): Promise<T> {
    return this.playbackHttpAction({ kind: "current_page", slot }, action, input);
  }

  explicitPagePlaybackAction<T = PlaybackActionOutcome>(
    page: number,
    slot: number,
    action: string,
    input: Record<string, unknown> = {},
  ): Promise<T> {
    return this.playbackHttpAction(
      { kind: "explicit_page", page, slot },
      action,
      input,
    );
  }

  private async playbackHttpAction<T>(
    address: PlaybackAddress,
    action: string,
    input: Record<string, unknown>,
  ): Promise<T> {
    if (!this.session) throw new Error("API session is not initialized");
    const bootstrap = await this.request<{ active_show: { id: string } | null }>(
      "GET",
      "/api/v2/bootstrap",
      undefined,
      false,
    );
    if (!bootstrap.active_show) throw new Error("No active show");
    return this.request<T>(
      "POST",
      "/api/v2/playback-actions",
      {
        request_id: crypto.randomUUID(),
        address,
        action: structuredPlaybackAction(action, input),
        surface: input.surface === "virtual" ? "virtual" : "physical",
      },
      true,
      undefined,
      { showId: bootstrap.active_show.id, deskId: this.session.desk.id },
    );
  }

  private async response(
    method: string,
    path: string,
    body?: unknown,
    authenticate = true,
    revision?: number,
    context?: { showId?: string; deskId?: string },
  ): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["content-type"] = "application/json";
    if (authenticate) {
      if (!this.session) throw new Error("API session is not initialized");
      headers.authorization = `Bearer ${this.session.token}`;
      if (path.startsWith("/api/v2/command-line")) {
        headers["x-tosk-desk"] = this.session.desk.id;
      }
    }
    if (revision !== undefined) headers["if-match"] = String(revision);
    if (context?.showId) headers["x-tosk-show"] = context.showId;
    if (context?.deskId) headers["x-tosk-desk"] = context.deskId;
    const response = await fetch(`${this.baseUrl}${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
    if (!response.ok) throw new Error(`${method} ${path} returned ${response.status}: ${await response.text()}`);
    return response;
  }

  async getCommandLine(): Promise<RevisionedCommandLine> {
    const response = await this.response("GET", this.commandLinePath());
    return parseRevisionedCommandLine(response);
  }

  async replaceCommandLine(text: string, expectedRevision: number): Promise<RevisionedCommandLine> {
    const response = await this.response("PUT", this.commandLinePath(), { text }, true, expectedRevision);
    return parseRevisionedCommandLine(response);
  }

  /** Replaces the visible command-line text against its current revision. */
  async setCommandLineText(text: string): Promise<RevisionedCommandLine> {
    for (let attempt = 0; attempt < 2; attempt++) {
      const { commandLine } = await this.getCommandLine();
      try {
        return await this.replaceCommandLine(text, commandLine.revision);
      } catch (error) {
        const isRevisionRace =
          error instanceof Error &&
          error.message.includes("returned 409:") &&
          error.message.includes("command-line revision conflict");
        if (!isRevisionRace || attempt === 1) throw error;
      }
    }
    throw new Error("unreachable command-line revision retry state");
  }

  /**
   * Sets the FIXTURE/GROUP command target.
   *
   * The command target is carried as an explicit v2 live action.
   */
  async setCommandTarget(target: CommandTarget): Promise<CommandResponse> {
    return this.liveAction({
      type: "command_target",
      request: { value: target },
    });
  }

  async sendCommandKey(
    key: SoftwareKey,
    phase: CommandKeyPhase = "press",
    requestId = crypto.randomUUID(),
  ): Promise<CommandOperationResponse> {
    return this.commandLineOperation("keys", { key, phase, request_id: requestId });
  }

  async executeCommandLineRaw(command?: string, requestId = crypto.randomUUID()): Promise<CommandOperationResponse> {
    return this.commandLineOperation("execute", { command, request_id: requestId });
  }

  async executeCommandLine(command?: string, requestId = crypto.randomUUID()): Promise<CommandOperationResponse> {
    const response = await this.executeCommandLineRaw(command, requestId);
    if (response.outcome === "rejected") {
      throw new Error(`programmer.execute failed: ${response.error}`);
    }
    return response;
  }

  alignProgrammerSelection(
    mode: "off" | "left" | "right" | "out" | "in",
  ): Promise<CommandResponse> {
    const requestId = crypto.randomUUID();
    return this.liveAction({
      type: "programming_align",
      request: {
        request_id: requestId,
        mode,
      },
    }, requestId);
  }

  controlFixtureAction(
    fixtureId: string,
    actionId: string,
    active: boolean,
  ): Promise<CommandResponse> {
    const requestId = crypto.randomUUID();
    return this.liveAction({
      type: "fixture_control",
      request: {
        request_id: requestId,
        fixture_id: fixtureId,
        action_id: actionId,
        active,
      },
    }, requestId);
  }

  private async commandLineOperation(operation: "keys" | "execute", body: unknown): Promise<CommandOperationResponse> {
    const response = await this.response("POST", `${this.commandLinePath()}/${operation}`, body);
    const result = await response.json() as CommandOperationResponse;
    validateCommandRevision(response, result.command_line);
    return result;
  }

  private commandLinePath(): string {
    if (!this.session) throw new Error("API session is not initialized");
    return "/api/v2/command-line";
  }

  async liveAction<T>(action: LiveAction, requestId = crypto.randomUUID()): Promise<CommandResponse<T>> {
    if (!this.session) throw new Error("API session is not initialized");
    const socket = new WebSocket(this.baseUrl.replace(/^http/, "ws") + "/api/v2/events", ["light.events.v2", `light.token.${this.session.token}`]);
    try {
      await waitForWebSocketOpen(socket);
      socket.send(JSON.stringify({
        type: "subscribe",
        filter: { capabilities: ["system"] },
        capacity: 32,
        rate_limits: [],
      }));
      return await new Promise<CommandResponse<T>>((resolve, reject) => {
        const finish = (response?: CommandResponse<T>, error?: Error) => {
          clearTimeout(timer);
          socket.removeEventListener("message", onMessage);
          socket.removeEventListener("close", onClose);
          if (error) reject(error);
          else resolve(response!);
        };
        const onClose = () => finish(undefined, new Error(`API WebSocket closed before ${action.type} responded`));
        const onMessage = (event: MessageEvent) => {
          const response = JSON.parse(String(event.data)) as CommandResponse<T>;
          if (response.request_id !== requestId) return;
          if (response.ok) finish(response);
          else finish(undefined, new Error(`${action.type} failed: ${response.error ?? "unknown error"}`));
        };
        const timer = setTimeout(
          () => finish(undefined, new Error(`API action timed out: ${action.type}`)),
          WEB_SOCKET_TIMEOUT_MILLIS,
        );
        timer.unref();
        socket.addEventListener("message", onMessage);
        socket.addEventListener("close", onClose, { once: true });
        try {
          socket.send(JSON.stringify({
            type: "action",
            protocol_version: 2,
            request_id: requestId,
            session_id: this.session?.session_id,
            action,
          }));
        } catch (error) {
          finish(undefined, error instanceof Error ? error : new Error(String(error)));
        }
      });
    } finally {
      // A command connection is short lived, but its close handshake must finish while
      // the test server is still alive. Merely calling close() leaves an Undici socket
      // in CLOSING; killing the bench server immediately afterwards can strand that
      // handle in the Playwright worker indefinitely.
      await closeWebSocket(socket, `API action ${action.type}`);
    }
  }
}

function structuredPlaybackAction(
  action: string,
  input: Record<string, unknown>,
): PlaybackAction {
  const pressed = typeof input.pressed === "boolean" ? input.pressed : true;
  if (action === "release") return { type: "release" };
  if (action === "button") {
    if (typeof input.button !== "number") throw new Error("button number is required");
    return { type: "configured_button", number: input.button, pressed };
  }
  if (action === "master") {
    if (typeof input.value !== "number") throw new Error("master value is required");
    return { type: "master", value: input.value };
  }
  if (action === "go-to" || action === "load") {
    if (typeof input.cue_number !== "number") throw new Error("cue number is required");
    return {
      type: action === "go-to" ? "go_to" : "load",
      cue_number: input.cue_number,
    };
  }
  if (action === "xfade-on" || action === "xfade-off") {
    return { type: "crossfade", enabled: action === "xfade-on" };
  }
  if (action === "temp-on" || action === "temp-off") {
    return { type: "temporary", enabled: action === "temp-on", pressed };
  }
  const type = ({
    "go-minus": "back",
    "fast-forward": "fast_forward",
    "fast-rewind": "fast_rewind",
    "select-contents": "select_contents",
    "select-dereferenced": "select_dereferenced",
    "pause-dynamics": "pause_dynamics",
  } as Record<string, string>)[action] ?? action;
  return { type, pressed } as PlaybackAction;
}

async function parseRevisionedCommandLine(response: Response): Promise<RevisionedCommandLine> {
  const commandLine = await response.json() as CommandLineState;
  const etag = validateCommandRevision(response, commandLine);
  return { commandLine, etag };
}

function validateCommandRevision(response: Response, commandLine: CommandLineState): string {
  const etag = response.headers.get("etag");
  if (etag === null) throw new Error("Command-line response is missing its ETag");
  const revision = Number(etag.replace(/^W\//, "").replace(/^\"|\"$/g, ""));
  if (!Number.isSafeInteger(revision) || revision !== commandLine.revision) {
    throw new Error(`Command-line ETag ${etag} does not match revision ${commandLine.revision}`);
  }
  return etag;
}

async function waitForWebSocketOpen(socket: WebSocket): Promise<void> {
  if (socket.readyState === WebSocket.OPEN) return;
  if (socket.readyState === WebSocket.CLOSING || socket.readyState === WebSocket.CLOSED) {
    throw new Error("API WebSocket closed before connecting");
  }
  await new Promise<void>((resolve, reject) => {
    const finish = (error?: Error) => {
      clearTimeout(timer);
      socket.removeEventListener("open", onOpen);
      socket.removeEventListener("error", onError);
      if (error) reject(error); else resolve();
    };
    const onOpen = () => finish();
    const onError = () => finish(new Error("API WebSocket connection failed"));
    const timer = setTimeout(
      () => finish(new Error("API WebSocket connection timed out")),
      WEB_SOCKET_TIMEOUT_MILLIS,
    );
    timer.unref();
    socket.addEventListener("open", onOpen, { once: true });
    socket.addEventListener("error", onError, { once: true });
  });
}

/** Closes a test WebSocket and proves that its underlying Node handle was released. */
export async function closeWebSocket(socket: WebSocket, owner: string): Promise<void> {
  if (socket.readyState === WebSocket.CLOSED) return;
  await new Promise<void>((resolve, reject) => {
    const finish = (error?: Error) => {
      clearTimeout(timer);
      socket.removeEventListener("close", onClose);
      if (error) reject(error); else resolve();
    };
    const onClose = () => finish();
    const timer = setTimeout(
      () => finish(new Error(`${owner} WebSocket did not close within ${WEB_SOCKET_TIMEOUT_MILLIS}ms`)),
      WEB_SOCKET_TIMEOUT_MILLIS,
    );
    timer.unref();
    socket.addEventListener("close", onClose, { once: true });
    if (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING) {
      try {
        socket.close(1000, "test operation complete");
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
      }
    }
  });
}
