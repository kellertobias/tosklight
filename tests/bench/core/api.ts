import type { SoftwareKey } from "@tosklight/light-controls/programmer-keypad";
import type {
  FixtureLibraryAction,
  FixtureLibraryActionOutcome,
  FixtureDefinitionsSnapshot,
  FixtureLibraryWarningsSnapshot,
  FixtureProfilesSnapshot,
  FixtureProfileRevisionsSnapshot,
  PlaybackAction,
  PlaybackActionOutcome,
  PlaybackAddress,
} from "../../../apps/light-desktop/src/api/generated/light-wire";
import type { PatchSnapshot as LegacyPatchSnapshot } from "../../../apps/light-desktop/src/api/types";
import { decodePatchSnapshot } from "../../../apps/light-desktop/src/api/patchWire";
import { projectionToPatchedFixture } from "../../../apps/light-desktop/src/features/patch/model";

export interface Session { session_id: string; client_id: string; token: string; user: { id: string; name: string }; desk: { id: string; osc_alias: string } }
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

/**
 * Command families whose complete mutation still belongs to the v1 compatibility grammar.
 *
 * Each variant names the production boundary that does not exist yet, so a scenario states which
 * ownership gap it is riding on rather than hiding behind an anonymous "legacy" call.
 */
export type CompatibilityCommandFamily =
  /** Preset deletion; Group and whole-Cue deletion have typed application actions. */
  | "preset_delete"
  /** Preset `MOVE`/`COPY`; only Cue transfer is intercepted by the typed Programming boundary. */
  | "preset_transfer"
  /** `UPDATE`; the command grammar is not yet routed through the typed Update workflow. */
  | "update";

export interface CompatibilityProgrammerCommand {
  family: CompatibilityCommandFamily;
  command: string;
}

export type CommandLineOwnership =
  | { via: "command-line-http" }
  | { via: "compatibility"; family: CompatibilityCommandFamily };

const CUE_TRANSFER = /^(?:MOVE|MOV|COPY|CPY)\s+(?:(?:PLAIN|STATUS)\s+)?SET\b/i;
const CUE_OR_GROUP_RECORD = /^(?:RECORD|REC)\s+(?:[+-]\s+)?(?:GROUP|CUE|SET)\b/i;
const PRESET_RECORD = /^(?:RECORD|REC)\s+\S+(?:\s+\S+){0,2}$/i;
const GROUP_DELETE = /^(?:DELETE|DEL)\s+GROUP\b/i;
const CUE_DELETE = /^(?:DELETE|DEL)\s+SET\b/i;

/**
 * Classifies one command against the grammars the server intercepts before its atomic-family check.
 *
 * `record_typed_command` routes Group recording, Preset recording, Cue recording, Cue transfer,
 * CUE navigation, whole-Cue deletion, and Speed Group commands through typed application
 * boundaries, so those reach public v2 HTTP contracts. CUE and SPD therefore have no
 * leading-token cases below.
 * Everything else in a legacy family is still compatibility-owned. This is a static ownership
 * decision on purpose: attempting v2 and falling back to v1 would hide an ownership regression.
 */
export function commandLineOwnership(command: string): CommandLineOwnership {
  const trimmed = command.trim();
  if (
    CUE_TRANSFER.test(trimmed) ||
    CUE_OR_GROUP_RECORD.test(trimmed) ||
    GROUP_DELETE.test(trimmed) ||
    CUE_DELETE.test(trimmed) ||
    PRESET_RECORD.test(trimmed)
  ) {
    return { via: "command-line-http" };
  }
  const family = trimmed.match(/^[A-Za-z]+/)?.[0]?.toUpperCase();
  switch (family) {
    case "DELETE":
    case "DEL":
      return { via: "compatibility", family: "preset_delete" };
    case "MOVE":
    case "MOV":
    case "COPY":
    case "CPY":
      return { via: "compatibility", family: "preset_transfer" };
    case "UPDATE":
      return { via: "compatibility", family: "update" };
    default:
      return { via: "command-line-http" };
  }
}

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
    if (method === "POST" && path === "/api/v2/users") {
      return value.user as T;
    }
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
    if (method === "POST" && path === "/api/v2/users") {
      return {
        method,
        path: "/api/v2/users/create",
        body: { request_id: crypto.randomUUID(), ...(body as object) },
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
    const { commandLine } = await this.getCommandLine();
    return this.replaceCommandLine(text, commandLine.revision);
  }

  /**
   * Sets the FIXTURE/GROUP command target.
   *
   * The command target has no typed v2 owner yet; the production frontend still issues this v1
   * command from `api/client/programming.ts`, so acceptance coverage matches that surface.
   */
  async setCompatibilityCommandTarget(target: CommandTarget): Promise<CommandResponse> {
    return this.command("programmer.command_target", { value: target });
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

  /**
   * Executes one command family that is still owned by the v1 compatibility grammar.
   *
   * The caller names the missing production boundary, so the remaining compatibility surface stays
   * countable and reviewable instead of looking like an ordinary command-line action.
   */
  async executeCompatibilityProgrammerCommand(
    request: CompatibilityProgrammerCommand,
  ): Promise<CommandResponse> {
    const ownership = commandLineOwnership(request.command);
    if (ownership.via === "command-line-http") {
      throw new Error(
        `${request.command} is owned by the v2 command-line HTTP contract; use executeCommandLine`,
      );
    }
    if (ownership.family !== request.family) {
      throw new Error(
        `${request.command} belongs to the ${ownership.family} compatibility family, not ${request.family}`,
      );
    }
    return this.sendCompatibilityCommandLine(request.command);
  }

  alignProgrammerSelection(
    attribute: "pan" | "tilt",
    mode: "left" | "right" | "center" | "out",
    from = 0,
    to = 1,
  ): Promise<CommandResponse> {
    return this.command("programmer.align", { attribute, mode, from, to });
  }

  controlFixtureAction(
    fixtureId: string,
    actionId: string,
    active: boolean,
  ): Promise<CommandResponse> {
    return this.command("programmer.control_action", {
      fixture_id: fixtureId,
      action_id: actionId,
      active,
    });
  }

  /** Raw textual WebSocket command envelope. Private so new scenarios cannot reach it directly. */
  private async sendCompatibilityCommandLine(command: string): Promise<CommandResponse> {
    return this.command("programmer.execute", { value: command });
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

  async command<T>(command: string, payload: unknown, expectedRevision?: number): Promise<CommandResponse<T>> {
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
      const requestId = crypto.randomUUID();
      return await new Promise<CommandResponse<T>>((resolve, reject) => {
        const finish = (response?: CommandResponse<T>, error?: Error) => {
          clearTimeout(timer);
          socket.removeEventListener("message", onMessage);
          socket.removeEventListener("close", onClose);
          if (error) reject(error);
          else resolve(response!);
        };
        const onClose = () => finish(undefined, new Error(`API WebSocket closed before ${command} responded`));
        const onMessage = (event: MessageEvent) => {
          const response = JSON.parse(String(event.data)) as CommandResponse<T>;
          if (response.request_id !== requestId) return;
          if (response.ok) finish(response);
          else finish(undefined, new Error(`${command} failed: ${response.error ?? "unknown error"}`));
        };
        const timer = setTimeout(
          () => finish(undefined, new Error(`API command timed out: ${command}`)),
          WEB_SOCKET_TIMEOUT_MILLIS,
        );
        timer.unref();
        socket.addEventListener("message", onMessage);
        socket.addEventListener("close", onClose, { once: true });
        try {
          socket.send(JSON.stringify({
            protocol_version: 1,
            request_id: requestId,
            session_id: this.session?.session_id,
            expected_revision: expectedRevision,
            command,
            payload,
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
      await closeWebSocket(socket, `API command ${command}`);
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
