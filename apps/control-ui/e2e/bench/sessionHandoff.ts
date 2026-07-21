import type { Page } from "@playwright/test";
import {
	SESSION_HANDOFF_RECEIVER,
	type SessionHandoffPublication,
} from "../../src/features/session/sessionHandoff";
import type { SessionResponse } from "../../src/api/types";

const ACCEPT_HANDOFF_BINDING = "__lightAcceptSessionHandoff";

interface BrowserPublication {
	document_id: string;
	publication: SessionHandoffPublication;
}

interface CaptureWaiter {
	after: number;
	predicate: (session: SessionResponse) => boolean;
	resolve: (session: SessionResponse) => void;
	reject: (reason: Error) => void;
	timer: ReturnType<typeof setTimeout>;
}

export class BrowserSessionHandoff {
	private installed = false;
	private disposed = false;
	private receipt = 0;
	private adoptedDocument: string | null = null;
	private currentDocument: string | null = null;
	private currentGeneration = -1;
	private current: SessionResponse | null = null;
	private readonly retiredDocuments = new Set<string>();
	private readonly waiters = new Set<CaptureWaiter>();

	constructor(private readonly page: Page) {
		page.on("close", this.onPageClosed);
		page.on("framenavigated", this.onFrameNavigated);
	}

	async install(): Promise<void> {
		if (this.disposed) throw new Error("Session handoff authority was disposed");
		if (this.installed) return;
		this.installed = true;
		await this.page.exposeBinding(
			ACCEPT_HANDOFF_BINDING,
			(_source, value) => this.receive(value),
		);
		await this.page.addInitScript(
			({ receiverName, bindingName }) => {
				const documentId = crypto.randomUUID();
				const accept = (window as unknown as Record<string, unknown>)[
					bindingName
				] as (value: unknown) => Promise<void>;
				const receive = (publication: unknown) =>
					accept({ document_id: documentId, publication });
				Object.defineProperty(receive, "documentId", { value: documentId });
				Object.defineProperty(window, receiverName, {
					configurable: true,
					value: receive,
				});
			},
			{
				receiverName: SESSION_HANDOFF_RECEIVER,
				bindingName: ACCEPT_HANDOFF_BINDING,
			},
		);
	}

	async adoptCurrentDocument(): Promise<void> {
		if (this.disposed) throw new Error("Session handoff authority was disposed");
		const documentId = await this.page.evaluate((receiverName) => {
			const receiver = (window as unknown as Record<string, unknown>)[
				receiverName
			];
			return typeof receiver === "function"
				? Reflect.get(receiver, "documentId")
				: null;
		}, SESSION_HANDOFF_RECEIVER);
		if (typeof documentId !== "string" || !documentId)
			throw new Error("The current document has no session handoff authority");
		if (this.currentDocument && this.currentDocument !== documentId) {
			this.retiredDocuments.add(this.currentDocument);
			this.clearCurrentDocument();
		}
		this.adoptedDocument = documentId;
		this.currentDocument = documentId;
	}

	checkpoint(): number {
		return this.receipt;
	}

	currentSession(): SessionResponse | null {
		return this.current;
	}

	waitForCapture(
		after: number,
		predicate: (session: SessionResponse) => boolean = () => true,
		timeout = 10_000,
	): Promise<SessionResponse> {
		if (this.disposed)
			return Promise.reject(new Error("Session handoff authority was disposed"));
		if (this.receipt > after && this.current && predicate(this.current))
			return Promise.resolve(this.current);
		return new Promise((resolve, reject) => {
			const waiter = {
				after,
				predicate,
				resolve,
				reject,
				timer: setTimeout(() => {
					this.waiters.delete(waiter);
					reject(new Error("Timed out waiting for the public session handoff"));
				}, timeout),
			};
			this.waiters.add(waiter);
		});
	}

	dispose(): void {
		this.page.off("close", this.onPageClosed);
		this.page.off("framenavigated", this.onFrameNavigated);
		if (this.disposed) return;
		this.disposed = true;
		this.clearAuthority();
	}

	private readonly onFrameNavigated = (frame: ReturnType<Page["mainFrame"]>) => {
		if (frame !== this.page.mainFrame()) return;
		if (this.currentDocument) this.retiredDocuments.add(this.currentDocument);
		this.adoptedDocument = null;
		this.clearCurrentDocument();
	};

	private readonly onPageClosed = () => {
		this.disposed = true;
		this.clearAuthority();
	};

	private clearCurrentDocument(): void {
		this.currentDocument = null;
		this.currentGeneration = -1;
		this.current = null;
	}

	private clearAuthority(): void {
		this.adoptedDocument = null;
		this.clearCurrentDocument();
		for (const waiter of this.waiters) {
			clearTimeout(waiter.timer);
			waiter.reject(new Error("Session handoff authority was disposed"));
		}
		this.waiters.clear();
	}

	private receive(value: unknown): void {
		if (this.disposed) return;
		const envelope = decodeBrowserPublication(value);
		if (
			this.adoptedDocument &&
			envelope.document_id !== this.adoptedDocument
		)
			return;
		if (this.retiredDocuments.has(envelope.document_id)) return;
		if (envelope.document_id !== this.currentDocument) {
			if (this.currentDocument)
				this.retiredDocuments.add(this.currentDocument);
			this.currentDocument = envelope.document_id;
			this.currentGeneration = -1;
			this.current = null;
		}
		const publication = envelope.publication;
		if (publication.generation < this.currentGeneration) return;
		this.currentGeneration = publication.generation;
		this.receipt += 1;
		if (publication.type === "captured") {
			this.current = decodeSession(publication.session);
		} else if (
			!publication.session_id ||
			publication.session_id === this.current?.session_id
		) {
			this.current = null;
		}
		this.resolveWaiters();
	}

	private resolveWaiters(): void {
		if (!this.current) return;
		for (const waiter of this.waiters) {
			if (this.receipt <= waiter.after || !waiter.predicate(this.current))
				continue;
			clearTimeout(waiter.timer);
			this.waiters.delete(waiter);
			waiter.resolve(this.current);
		}
	}
}

function decodeBrowserPublication(value: unknown): BrowserPublication {
	const envelope = record(value, "session handoff envelope");
	const documentId = stringAt(envelope, "document_id");
	const publication = record(envelope.publication, "session handoff publication");
	const generation = numberAt(publication, "generation");
	if (publication.type === "released") {
		const sessionId = publication.session_id;
		if (sessionId !== null && typeof sessionId !== "string")
			throw new Error("Invalid released session identity");
		return {
			document_id: documentId,
			publication: { type: "released", generation, session_id: sessionId },
		};
	}
	if (publication.type !== "captured")
		throw new Error("Invalid session handoff publication type");
	return {
		document_id: documentId,
		publication: {
			type: "captured",
			generation,
			session: decodeSession(publication.session),
		},
	};
}

function decodeSession(value: unknown): SessionResponse {
	const session = record(value, "session");
	const user = record(session.user, "session user");
	const desk = record(session.desk, "session desk");
	return {
		session_id: stringAt(session, "session_id"),
		client_id: stringAt(session, "client_id"),
		token: stringAt(session, "token"),
		user: {
			id: stringAt(user, "id"),
			name: stringAt(user, "name"),
			enabled: booleanAt(user, "enabled"),
		},
		desk: {
			id: stringAt(desk, "id"),
			name: stringAt(desk, "name"),
			osc_alias: stringAt(desk, "osc_alias"),
			columns: numberAt(desk, "columns"),
			rows: numberAt(desk, "rows"),
			buttons: numberAt(desk, "buttons"),
		},
	};
}

function record(value: unknown, label: string): Record<string, unknown> {
	if (!value || typeof value !== "object" || Array.isArray(value))
		throw new Error(`Invalid ${label}`);
	return value as Record<string, unknown>;
}

function stringAt(value: Record<string, unknown>, field: string): string {
	const result = value[field];
	if (typeof result !== "string" || !result)
		throw new Error(`Invalid ${field}`);
	return result;
}

function numberAt(value: Record<string, unknown>, field: string): number {
	const result = value[field];
	if (!Number.isSafeInteger(result) || (result as number) < 0)
		throw new Error(`Invalid ${field}`);
	return result as number;
}

function booleanAt(value: Record<string, unknown>, field: string): boolean {
	const result = value[field];
	if (typeof result !== "boolean") throw new Error(`Invalid ${field}`);
	return result;
}
