import type {
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
	AttributeConfigurationUpdateOutcome,
	AttributeConfigurationUpdateRequest,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export class AttributeConfigurationApiClient {
	constructor(private readonly transport: ClientTransport) {}

	snapshot(showId: string): Promise<AttributeConfigurationSnapshot> {
		return this.transport.request("/api/v2/attribute-configuration", {
			headers: showHeaders(showId),
		});
	}

	update(
		showId: string,
		snapshot: AttributeConfigurationSnapshot,
		patch: AttributeConfigurationPatch,
	): Promise<AttributeConfigurationUpdateOutcome> {
		const request: AttributeConfigurationUpdateRequest = {
			request_id: crypto.randomUUID(),
			expected_show_revision: snapshot.show_revision,
			expected_object_revision: snapshot.object_revision,
			patch,
		};
		const init = jsonRequest("POST", request);
		return this.transport.request("/api/v2/attribute-configuration/update", {
			...init,
			headers: { ...init.headers, ...showHeaders(showId) },
		});
	}
}

function showHeaders(showId: string) {
	return { "x-tosk-show": showId };
}
