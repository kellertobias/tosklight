import type { HelpCatalog, HelpTopic } from "../types";
import type { ClientTransport } from "./transport";

export class HelpApiClient {
	constructor(private readonly transport: ClientTransport) {}

	helpCatalog(): Promise<HelpCatalog> {
		return this.transport.request("/api/v1/help", {}, false);
	}

	helpTopic(id: string): Promise<HelpTopic> {
		return this.transport.request(
			`/api/v1/help/topics/${encodeURIComponent(id)}`,
			{},
			false,
		);
	}
}
