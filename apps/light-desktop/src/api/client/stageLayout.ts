import type {
	StageLayoutAction,
	StageLayoutActionOutcome,
	StageProjection2d,
} from "../generated/light-wire";
import { type ClientTransport, jsonRequest } from "./transport";

export type { StageProjection2d } from "../generated/light-wire";

export class StageLayoutApiClient {
	constructor(private readonly transport: ClientTransport) {}

	action(
		showId: string,
		action: StageLayoutAction,
	): Promise<StageLayoutActionOutcome> {
		const request = jsonRequest("POST", {
			request_id: crypto.randomUUID(),
			action,
		});
		return this.transport.request("/api/v2/stage-layout/actions", {
			...request,
			headers: {
				...request.headers,
				"x-tosk-show": showId,
			},
		});
	}

	regenerate2d(
		showId: string,
		projection: StageProjection2d,
	): Promise<StageLayoutActionOutcome> {
		return this.action(showId, {
			type: "regenerate_2d",
			projection,
		});
	}
}
