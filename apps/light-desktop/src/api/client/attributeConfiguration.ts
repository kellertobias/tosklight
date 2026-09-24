import { ApiRequestError } from "../ApiRequestError";
import type {
	AttributeConfigurationPatch,
	AttributeConfigurationSnapshot,
} from "../attributeConfigurationModels";
import type {
	ColorIntentReport,
	ColorModelImpact,
	ColorProgrammingModel,
	AttributeConfigurationUpdateRequest,
	AttributeConfigurationPatch as WireAttributeConfigurationPatch,
	AttributeConfigurationSnapshot as WireAttributeConfigurationSnapshot,
	AttributeConfigurationUpdateOutcome as WireAttributeConfigurationUpdateOutcome,
} from "../generated/light-wire";
import type { ClientTransport } from "./transport";
import { jsonRequest } from "./transport";

export type {
	ColorIntentHeadReport,
	ColorIntentReport,
	ColorModelImpact,
	ColorResolutionQuality,
} from "../generated/light-wire";
export type {
	AttributeConfiguration,
	ColorProgrammingModel,
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

	/** What switching the show to `model` would do to the colour it already stores. */
	colorModelImpact(
		showId: string,
		model: ColorProgrammingModel,
	): Promise<ColorModelImpact> {
		return this.transport.request<ColorModelImpact>(
			`/api/v2/attribute-configuration/color-model-impact?model=${model}`,
			{ headers: showHeaders(showId) },
		);
	}

	/** How faithfully each head of the given fixtures shows its current colour. */
	colorIntentReport(
		showId: string,
		fixtureIds: readonly string[],
	): Promise<ColorIntentReport> {
		const query = fixtureIds.length
			? `?fixtures=${fixtureIds.map(encodeURIComponent).join(",")}`
			: "";
		return this.transport.request<ColorIntentReport>(
			`/api/v2/color-intent/report${query}`,
			{ headers: showHeaders(showId) },
		);
	}

	async update(
		showId: string,
		snapshot: AttributeConfigurationSnapshot,
		patch: AttributeConfigurationPatch,
		options: { acknowledgeColorModelImpact?: boolean } = {},
	): Promise<{ snapshot: AttributeConfigurationSnapshot }> {
		const { placements, ...otherChanges } = patch;
		const wirePatch: WireAttributeConfigurationPatch = {
			...otherChanges,
			...(placements !== undefined
				? {
						placements: placements?.map((placement) => ({
							...placement,
							push_turn_of: placement.push_turn_of ?? null,
						})),
					}
				: {}),
		};
		const acknowledge = options.acknowledgeColorModelImpact ?? false;
		try {
			return await this.sendUpdate(showId, snapshot, wirePatch, acknowledge);
		} catch (error) {
			if (!(error instanceof ApiRequestError) || error.status !== 409)
				throw error;
			const latest = await this.snapshot(showId);
			if (latest.object_revision !== snapshot.object_revision) throw error;
			return this.sendUpdate(showId, latest, wirePatch, acknowledge);
		}
	}

	private sendUpdate(
		showId: string,
		snapshot: AttributeConfigurationSnapshot,
		patch: WireAttributeConfigurationPatch,
		acknowledgeColorModelImpact: boolean,
	): Promise<{ snapshot: AttributeConfigurationSnapshot }> {
		const request: AttributeConfigurationUpdateRequest = {
			request_id: crypto.randomUUID(),
			expected_show_revision: snapshot.show_revision,
			expected_object_revision: snapshot.object_revision,
			patch,
			...(acknowledgeColorModelImpact
				? { acknowledge_color_model_impact: true }
				: {}),
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
