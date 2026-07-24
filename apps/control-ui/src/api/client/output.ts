import type { HighlightAction, HighlightState } from "../types";
import type {
	OutputRuntimeActionOutcome,
	OutputRuntimeActionRequest,
} from "../../features/outputRuntime/contracts";
import {
	decodeOutputRuntimeActionOutcome,
	encodeOutputRuntimeActionRequest,
} from "../outputRuntimeWire";
import type { LiveClientTransport } from "./transport";

export class OutputApiClient {
	constructor(private readonly transport: LiveClientTransport) {}

	async outputRuntimeLiveAction(
		showId: string,
		request: OutputRuntimeActionRequest,
	): Promise<OutputRuntimeActionOutcome> {
		const wireRequest = encodeOutputRuntimeActionRequest(request);
		const value = await this.transport.commandWithRequestId(
			"output_runtime.action",
			wireRequest,
			wireRequest.request_id,
		);
		return decodeOutputRuntimeActionOutcome(value, showId, request);
	}

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
}
