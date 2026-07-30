import type {
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
} from "../attributeConfigurationModels";
import type {
	AttributeConfigurationUpdateRequest,
	AttributeConfigurationSnapshot as WireAttributeConfigurationSnapshot,
	AttributeConfigurationUpdateOutcome as WireAttributeConfigurationUpdateOutcome,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export type {
	AttributeConfiguration,
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
	AttributeEncoderGroup,
	ConfiguredAttributeDescriptor,
	CustomAttributeDescriptor,
} from "../attributeConfigurationModels";

export class AttributeConfigurationApiClient {
	constructor(private readonly transport: ClientTransport) {}

	async snapshot(showId: string): Promise<AttributeConfigurationSnapshot> {
		const snapshot =
			await this.transport.request<WireAttributeConfigurationSnapshot>(
				"/api/v2/attribute-configuration",
				{ headers: showHeaders(showId) },
			);
		return mapSnapshot(snapshot);
	}

	update(
		showId: string,
		snapshot: AttributeConfigurationSnapshot,
		patch: AttributeConfigurationPatch,
	): Promise<{ snapshot: AttributeConfigurationSnapshot }> {
		const request: AttributeConfigurationUpdateRequest = {
			request_id: crypto.randomUUID(),
			expected_show_revision: snapshot.show_revision,
			expected_object_revision: snapshot.object_revision,
			patch,
		};
		const init = jsonRequest("POST", request);
		return this.transport
			.request<WireAttributeConfigurationUpdateOutcome>(
				"/api/v2/attribute-configuration/update",
				{
					...init,
					headers: { ...init.headers, ...showHeaders(showId) },
				},
			)
			.then((outcome) => ({ snapshot: mapSnapshot(outcome.snapshot) }));
	}
}

function mapSnapshot(
	snapshot: WireAttributeConfigurationSnapshot,
): AttributeConfigurationSnapshot {
	return {
		show_id: snapshot.show_id,
		show_revision: snapshot.show_revision,
		object_revision: snapshot.object_revision,
		configuration: {
			...snapshot.configuration,
			custom_attributes: snapshot.configuration.custom_attributes.map(
				(descriptor) => ({ ...descriptor }),
			),
			placements: snapshot.configuration.placements.map((placement) => ({
				...placement,
			})),
			activation_groups: snapshot.configuration.activation_groups.map(
				(group) => ({ ...group, members: [...group.members] }),
			),
		},
		recommended_configuration: {
			...snapshot.recommended_configuration,
			custom_attributes:
				snapshot.recommended_configuration.custom_attributes.map(
					(descriptor) => ({ ...descriptor }),
				),
			placements: snapshot.recommended_configuration.placements.map(
				(placement) => ({ ...placement }),
			),
			activation_groups:
				snapshot.recommended_configuration.activation_groups.map((group) => ({
					...group,
					members: [...group.members],
				})),
		},
		descriptors: snapshot.descriptors.map((descriptor) => ({ ...descriptor })),
		validation_error: snapshot.validation_error,
	};
}

function showHeaders(showId: string) {
	return { "x-tosk-show": showId };
}
