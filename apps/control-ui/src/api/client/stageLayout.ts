import type {
	StageLayoutActionOutcome,
	StageLayoutActionRequest,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export class StageLayoutApiClient {
	constructor(private readonly transport: ClientTransport) {}

	moveSelection(
		request: StageLayoutActionRequest,
	): Promise<StageLayoutActionOutcome> {
		return this.transport.request(
			"/api/v2/stage-layout/actions",
			jsonRequest("POST", request),
		);
	}
}
