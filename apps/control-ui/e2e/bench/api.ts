export interface Session { session_id: string; client_id: string; token: string; user: { id: string; name: string }; desk: { id: string; osc_alias: string } }
export interface CommandResponse<T = unknown> { protocol_version: number; request_id: string; ok: boolean; revision: number; payload?: T; error?: string }

const WEB_SOCKET_TIMEOUT_MILLIS = 5_000;

export class ApiDriver {
  session?: Session;
  constructor(readonly baseUrl: string) {}

  async login(username = "Operator", deskId: string | null = this.session?.desk.id ?? null): Promise<Session> {
    this.session = await this.request<Session>("POST", "/api/v1/sessions", { username, desk_id: deskId }, false);
    return this.session;
  }

  async request<T>(method: string, path: string, body?: unknown, authenticate = true, revision?: number): Promise<T> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["content-type"] = "application/json";
    if (authenticate) {
      if (!this.session) throw new Error("API session is not initialized");
      headers.authorization = `Bearer ${this.session.token}`;
    }
    if (revision !== undefined) headers["if-match"] = String(revision);
    const response = await fetch(`${this.baseUrl}${path}`, { method, headers, body: body === undefined ? undefined : JSON.stringify(body) });
    if (!response.ok) throw new Error(`${method} ${path} returned ${response.status}: ${await response.text()}`);
    if (response.status === 204) return undefined as T;
    return response.json() as Promise<T>;
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
