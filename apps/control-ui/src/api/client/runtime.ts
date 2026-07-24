import { ApiRequestError } from "../ApiRequestError";
import type { EventServerMessage } from "../generated/light-wire";
import type { BootstrapSnapshot, ServerEvent, SessionResponse } from "../types";
import { browserStorage, defaultServerUrl } from "./serverLocation";
import type { LiveClientTransport } from "./transport";

type EventListener = (event: ServerEvent) => void;

interface CommandResponse {
	protocol_version: number;
	request_id: string;
	ok: boolean;
	revision: number;
	payload?: unknown;
	error?: string;
}

interface PendingCommand {
	resolve: (value: unknown) => void;
	reject: (error: Error) => void;
	timer: number;
}

export class LightClientRuntime {
	private session: SessionResponse | null = null;
	private socket: WebSocket | null = null;
	private readonly listeners = new Set<EventListener>();
	private readonly pending = new Map<string, PendingCommand>();
	private deskToken = browserStorage()?.getItem("light.desk-token") ?? "";
	protected readonly transport: LiveClientTransport;

	constructor(private readonly baseUrl = defaultServerUrl()) {
		this.transport = {
			request: <T>(path: string, init?: RequestInit, authenticate?: boolean) =>
				this.request<T>(path, init, authenticate),
			blob: (path: string, init?: RequestInit) => this.requestBlob(path, init),
			absoluteUrl: (path: string) => `${this.baseUrl}${path}`,
			command: (command: string, payload: unknown, revision?: number) =>
				revision === undefined
					? this.command(command, payload)
					: this.command(command, payload, revision),
			commandWithRequestId: (
				command: string,
				payload: unknown,
				requestId: string,
				revision?: number,
			) => this.sendCommand(command, payload, requestId, revision),
		};
	}

	get currentSession(): SessionResponse | null {
		return this.session;
	}

	restoreSession(session: SessionResponse): void {
		this.session = session;
	}

	setDeskToken(token: string): void {
		this.deskToken = token.trim();
		const storage = browserStorage();
		if (this.deskToken) storage?.setItem("light.desk-token", this.deskToken);
		else storage?.removeItem("light.desk-token");
	}

	bootstrap(): Promise<BootstrapSnapshot> {
		return this.request("/api/v2/bootstrap", {}, false);
	}

	async login(username: string): Promise<SessionResponse> {
		const storage = browserStorage();
		const clientId = this.clientId(storage);
		const session = await this.request<SessionResponse>(
			"/api/v2/sessions",
			this.sessionRequest(username, clientId, storage),
			false,
		);
		this.installSession(session, storage);
		return session;
	}

	async closeSession(): Promise<void> {
		const session = this.session;
		if (!session) return;
		try {
			const response = await this.deleteSession(session);
			if (!response.ok && response.status !== 404) {
				throw new Error(await response.text());
			}
		} finally {
			if (this.session?.session_id === session.session_id) this.session = null;
			browserStorage()?.removeItem("light.primary-session");
		}
	}

	onEvent(listener: EventListener) {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	}

	connectEvents(onClose?: () => void): Promise<void> {
		if (!this.session) {
			throw new Error("A session is required before opening events");
		}
		this.disconnectEvents();
		const socket = new WebSocket(this.eventsUrl(), this.eventProtocols());
		this.socket = socket;
		socket.onclose = () => onClose?.();
		socket.addEventListener("open", () => {
			socket.send(
				JSON.stringify({
					type: "subscribe",
					filter: { capabilities: ["system"] },
					capacity: 256,
					rate_limits: [],
				}),
			);
		});
		socket.addEventListener("message", (message) =>
			this.handleSocketMessage(message),
		);
		return this.awaitSocketOpen(socket);
	}

	disconnectEvents(): void {
		if (this.socket) {
			this.socket.onclose = null;
			this.socket.close();
		}
		this.socket = null;
	}

	command(
		command: string,
		payload: unknown,
		expectedRevision?: number,
	): Promise<unknown> {
		return this.sendCommand(
			command,
			payload,
			crypto.randomUUID(),
			expectedRevision,
		);
	}

	private sendCommand(
		command: string,
		payload: unknown,
		requestId: string,
		expectedRevision?: number,
	): Promise<unknown> {
		if (!this.session || !this.socket || !this.socketIsOpen()) {
			return Promise.reject(new Error("Live server connection is not ready"));
		}
		return new Promise((resolve, reject) => {
			const timer = window.setTimeout(() => {
				this.pending.delete(requestId);
				reject(new Error(`Command timed out: ${command}`));
			}, 5_000);
			this.pending.set(requestId, {
				resolve,
				reject,
				timer,
			});
			this.socket?.send(
				JSON.stringify(
					this.commandEnvelope(command, payload, requestId, expectedRevision),
				),
			);
		});
	}

	private clientId(storage: Storage | null): string {
		const existing = storage?.getItem("light.client-id");
		if (existing) return existing;
		const created = crypto.randomUUID();
		storage?.setItem("light.client-id", created);
		return created;
	}

	private sessionRequest(
		username: string,
		clientId: string,
		storage: Storage | null,
	): RequestInit {
		return {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({
				username,
				client_id: clientId,
				desk_id: storage?.getItem("light.control-desk") ?? null,
			}),
		};
	}

	private installSession(session: SessionResponse, storage: Storage | null): void {
		this.session = session;
		storage?.setItem("light.primary-session", JSON.stringify(session));
		if (session.desk) storage?.setItem("light.control-desk", session.desk.id);
	}

	private deleteSession(session: SessionResponse): Promise<Response> {
		return fetch(`${this.baseUrl}/api/v2/sessions/${session.session_id}`, {
			method: "DELETE",
			keepalive: true,
			headers: this.boundaryHeaders(
				new Headers({ authorization: `Bearer ${session.token}` }),
			),
		});
	}

	private eventsUrl(): URL {
		const url = new URL("/api/v2/events", this.baseUrl);
		url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
		return url;
	}

	private eventProtocols(): string[] {
		if (!this.session) return [];
		const protocols = ["light.events.v2", `light.token.${this.session.token}`];
		if (this.deskToken) {
			protocols.push(`light.desk.b64.${base64Url(this.deskToken)}`);
		}
		return protocols;
	}

	private awaitSocketOpen(socket: WebSocket): Promise<void> {
		return new Promise<void>((resolve, reject) => {
			socket.addEventListener("open", () => resolve(), { once: true });
			socket.addEventListener(
				"error",
				() => reject(new Error("WebSocket connection failed")),
				{ once: true },
			);
		});
	}

	private handleSocketMessage(message: MessageEvent): void {
		const data = JSON.parse(String(message.data)) as
			| EventServerMessage
			| CommandResponse
			| { type?: string };
		if ("request_id" in data) {
			this.resolveCommand(data);
			return;
		}
		if (
			data.type === "event" &&
			"event" in data &&
			data.event.payload.type === "facade_notification"
		) {
			for (const listener of this.listeners)
				listener(data.event.payload.notification as ServerEvent);
			return;
		}
		if (data.type === "gap" || data.type === "error") this.socket?.close();
	}

	private resolveCommand(response: CommandResponse): void {
		const pending = this.pending.get(response.request_id);
		if (!pending) return;
		window.clearTimeout(pending.timer);
		this.pending.delete(response.request_id);
		if (response.ok) pending.resolve(response.payload);
		else pending.reject(new Error(response.error ?? "Command failed"));
	}

	private socketIsOpen(): boolean {
		return this.socket?.readyState === WebSocket.OPEN;
	}

	private commandEnvelope(
		command: string,
		payload: unknown,
		requestId: string,
		expectedRevision?: number,
	) {
		return {
			protocol_version: 1,
			request_id: requestId,
			session_id: this.session?.session_id,
			expected_revision: expectedRevision,
			command,
			payload,
		};
	}

	private boundaryHeaders(headers = new Headers()): Headers {
		if (this.deskToken) headers.set("x-light-desk-token", this.deskToken);
		return headers;
	}

	private async requestBlob(
		path: string,
		init: RequestInit = {},
	): Promise<Blob> {
		if (!this.session) throw new Error("A server session is required");
		const headers = this.boundaryHeaders(new Headers(init.headers));
		headers.set("authorization", `Bearer ${this.session.token}`);
		const response = await fetch(`${this.baseUrl}${path}`, { ...init, headers });
		if (!response.ok) throw new Error(await response.text());
		return response.blob();
	}

	private async request<T>(
		path: string,
		init: RequestInit = {},
		authenticate = true,
	): Promise<T> {
		const headers = this.boundaryHeaders(new Headers(init.headers));
		if (authenticate) this.authenticate(headers);
		const response = await fetch(`${this.baseUrl}${path}`, { ...init, headers });
		if (!response.ok) throw await apiError(response);
		if (response.status === 204) return undefined as T;
		return response.json() as Promise<T>;
	}

	private authenticate(headers: Headers): void {
		if (!this.session) throw new Error("A server session is required");
		headers.set("authorization", `Bearer ${this.session.token}`);
	}
}

async function apiError(response: Response): Promise<ApiRequestError> {
	const body = await response.text();
	return new ApiRequestError(
		body || `${response.status} ${response.statusText}`,
		response.status,
	);
}

function base64Url(value: string): string {
	const bytes = new TextEncoder().encode(value);
	let binary = "";
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary)
		.replaceAll("+", "-")
		.replaceAll("/", "_")
		.replace(/=+$/, "");
}
