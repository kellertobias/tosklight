import type {
	TimecodeTransportActionRequest,
	TimecodeTransportSnapshot,
} from "../generated/light-wire";
import {
	type TimecodeRuntime,
	timecodeRuntimeFromWire,
} from "../runtimeModels";
import { type ClientTransport, jsonRequest } from "./transport";

export interface TimecodeRunningApi {
	runtime(showId: string): Promise<TimecodeRuntime[]>;
	stop(showId: string, timecodeId: string): Promise<TimecodeRuntime>;
}

export class TimecodeRunningApiClient implements TimecodeRunningApi {
	constructor(private readonly transport: ClientTransport) {}

	async runtime(showId: string): Promise<TimecodeRuntime[]> {
		const values = await this.transport.request<TimecodeTransportSnapshot[]>(
			"/api/v2/timecodes/runtime",
			{ headers: showHeaders(showId) },
		);
		return values.map(timecodeRuntimeFromWire);
	}

	async stop(showId: string, timecodeId: string): Promise<TimecodeRuntime> {
		const body: TimecodeTransportActionRequest = {
			timecode_id: timecodeId,
			action: { type: "stop" },
		};
		const request = jsonRequest("POST", body);
		const value = await this.transport.request<TimecodeTransportSnapshot>(
			`/api/v2/timecodes/${encodeURIComponent(timecodeId)}/transport`,
			{
				...request,
				headers: { ...request.headers, ...showHeaders(showId) },
			},
		);
		return timecodeRuntimeFromWire(value);
	}
}

function showHeaders(showId: string): HeadersInit {
	return { "x-tosk-show": showId };
}
