import type { SoftwareKey } from "../../../shared/programmerKeypad";

export interface Session { session_id: string; client_id: string; token: string; user: { id: string; name: string }; desk: { id: string; osc_alias: string } }
export interface CommandResponse<T = unknown> { protocol_version: number; request_id: string; ok: boolean; revision: number; payload?: T; error?: string }

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
  /** `SPD GRP` BPM and synchronization; no application-owned Speed Group action exists. */
  | "speed_group"
  /** Whole-Cue deletion; Cue recording subtract is a different operation and is not a substitute. */
  | "cue_delete"
  /** Preset `MOVE`/`COPY`; only Cue transfer is intercepted by the typed Programming boundary. */
  | "preset_transfer"
  /** `SET <cuelist> AT <page>.<slot>`; awaits the typed map-existing Playback topology action. */
  | "playback_set"
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

/**
 * Classifies one command against the grammars the server intercepts before its atomic-family check.
 *
 * `record_typed_command` routes Group recording, Preset recording, Cue recording, Cue transfer, and
 * CUE navigation through the typed Programming boundary, so those reach the public v2 command-line
 * HTTP contract. CUE therefore has no leading-token case below: it is owned outright.
 * Everything else in a legacy family is still compatibility-owned. This is a static ownership
 * decision on purpose: attempting v2 and falling back to v1 would hide an ownership regression.
 */
export function commandLineOwnership(command: string): CommandLineOwnership {
  const trimmed = command.trim();
  if (
    CUE_TRANSFER.test(trimmed) ||
    CUE_OR_GROUP_RECORD.test(trimmed) ||
    GROUP_DELETE.test(trimmed) ||
    PRESET_RECORD.test(trimmed)
  ) {
    return { via: "command-line-http" };
  }
  const family = trimmed.match(/^[A-Za-z]+/)?.[0]?.toUpperCase();
  switch (family) {
    case "SPD":
      return { via: "compatibility", family: "speed_group" };
    case "DELETE":
    case "DEL":
      return { via: "compatibility", family: "cue_delete" };
    case "MOVE":
    case "MOV":
    case "COPY":
    case "CPY":
      return { via: "compatibility", family: "preset_transfer" };
    case "SET":
      return { via: "compatibility", family: "playback_set" };
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
    this.session = await this.request<Session>("POST", "/api/v1/sessions", { username, desk_id: deskId }, false);
    return this.session;
  }

  async request<T>(method: string, path: string, body?: unknown, authenticate = true, revision?: number): Promise<T> {
    const response = await this.response(method, path, body, authenticate, revision);
    if (response.status === 204) return undefined as T;
    return response.json() as Promise<T>;
  }

  private async response(method: string, path: string, body?: unknown, authenticate = true, revision?: number): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["content-type"] = "application/json";
    if (authenticate) {
      if (!this.session) throw new Error("API session is not initialized");
      headers.authorization = `Bearer ${this.session.token}`;
    }
    if (revision !== undefined) headers["if-match"] = String(revision);
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

  /** Raw v1 textual command envelope. Private so new scenarios cannot reach it directly. */
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
    return `/api/v2/desks/${this.session.desk.id}/command-line`;
  }

  async command<T>(command: string, payload: unknown, expectedRevision?: number): Promise<CommandResponse<T>> {
    if (!this.session) throw new Error("API session is not initialized");
    const socket = new WebSocket(this.baseUrl.replace(/^http/, "ws") + "/api/v1/events", ["light.v1", `light.token.${this.session.token}`]);
    try {
      await waitForWebSocketOpen(socket);
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
