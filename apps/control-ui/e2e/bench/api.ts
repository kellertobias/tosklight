export interface Session { session_id: string; token: string; user: { id: string; name: string }; desk: { id: string; osc_alias: string } }
export interface CommandResponse<T = unknown> { protocol_version: number; request_id: string; ok: boolean; revision: number; payload?: T; error?: string }

export class ApiDriver {
  session?: Session;
  constructor(readonly baseUrl: string) {}

  async login(username = "Operator"): Promise<Session> {
    this.session = await this.request<Session>("POST", "/api/v1/sessions", { username }, false);
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
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("API WebSocket connection timed out")), 5_000);
      socket.addEventListener("open", () => { clearTimeout(timer); resolve(); }, { once: true });
      socket.addEventListener("error", () => { clearTimeout(timer); reject(new Error("API WebSocket connection failed")); }, { once: true });
    });
    const requestId = crypto.randomUUID();
    try {
      return await new Promise<CommandResponse<T>>((resolve, reject) => {
        const timer = setTimeout(() => reject(new Error(`API command timed out: ${command}`)), 5_000);
        socket.addEventListener("message", (event) => {
          const response = JSON.parse(String(event.data)) as CommandResponse<T>;
          if (response.request_id !== requestId) return;
          clearTimeout(timer);
          if (response.ok) resolve(response);
          else reject(new Error(`${command} failed: ${response.error ?? "unknown error"}`));
        });
        socket.send(JSON.stringify({
          protocol_version: 1,
          request_id: requestId,
          session_id: this.session?.session_id,
          expected_revision: expectedRevision,
          command,
          payload,
        }));
      });
    } finally {
      socket.close();
    }
  }
}
