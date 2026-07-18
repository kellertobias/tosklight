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

const LEGACY_COMMAND_FAMILIES = new Set([
  "CUE", "SPD", "RECORD", "REC", "UPDATE", "DELETE", "DEL", "MOVE", "MOV", "COPY", "CPY", "SET",
]);

export function commandLineRequiresLegacyCompatibility(command: string): boolean {
  const family = command.trim().match(/^[A-Za-z]+/)?.[0]?.toUpperCase();
  return family !== undefined && LEGACY_COMMAND_FAMILIES.has(family);
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

  /** Temporary, deliberately named compatibility path for command families not yet atomic in v2. */
  async executeLegacyCommandLine(command: string): Promise<CommandResponse> {
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
