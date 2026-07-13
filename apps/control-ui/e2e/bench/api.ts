export interface Session { session_id: string; token: string; user: { id: string; name: string }; desk: { id: string; osc_alias: string } }

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
}
