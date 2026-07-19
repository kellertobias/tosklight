import type { HighlightAction, HighlightState } from "../types";
import type { LiveClientTransport } from "./transport";

export class OutputApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	setDmxOverride(universe: number, address: number, value: number | null) {
		return this.transport.request("/api/v1/dmx/override", {
			method: "PUT",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ universe, address, value }),
		});
	}

	highlight(): Promise<HighlightState> {
		return this.transport.request("/api/v1/highlight");
	}

	highlightAction(action: HighlightAction): Promise<HighlightState> {
		return this.transport.request("/api/v1/highlight/action", {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ action }),
		});
	}

	setPatchPreviewHighlight(active: boolean, fixtureIds: string[] = []) {
		return this.transport.request<{ active: boolean; allowed: boolean }>(
			"/api/v1/patch-preview-highlight",
			{
				method: "PUT",
				headers: { "content-type": "application/json" },
				body: JSON.stringify({ active, fixture_ids: fixtureIds }),
			},
		);
	}

	setMaster(payload: { grand_master?: number; blackout?: boolean }) {
		return this.transport.command("master.set", payload);
	}
}
