import type { CommandHistoryEntry, DeskUser } from "../types";
import type { ClientTransport } from "./transport";

export class DeskApiClient {
	constructor(private readonly transport: ClientTransport) {}

	commandHistory(): Promise<CommandHistoryEntry[]> {
		return this.transport.request("/api/v1/command-history");
	}

	createUser(name: string): Promise<DeskUser> {
		return this.transport.request("/api/v1/users", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ name, enabled: true }),
		});
	}

	auditEvents(after = 0) {
		return this.transport.request<
			Array<{ revision: number; kind: string; payload: unknown }>
		>(`/api/v1/audit?after=${after}`);
	}
}
