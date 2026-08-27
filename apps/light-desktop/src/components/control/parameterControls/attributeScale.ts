import { attributeBands, bandLabel, steppedValue } from "./attributeBands";
import {
	type AttributeDomain,
	attributeDomain,
	domainStep,
	formatAttributeValue,
	selectedChannelUnit,
} from "./attributeDomain";
import type { ParameterController } from "./useParameterController";

export interface AttributeScale {
	domain: AttributeDomain;
	display: string;
	/// Whether the attribute has slots the encoder can step between, so turning it is meaningful
	/// even though the value it holds is one of a fixed set.
	stepsBySlot: boolean;
	/// Moves the attribute by one detent, in whatever the attribute's own steps are.
	stepAttribute(delta: number, undoGroup?: string): void;
}

/**
 * How one attribute reads and moves under the encoder holding it.
 *
 * The scale comes from the fixture where it states one and from the attribute registry otherwise,
 * so a colour temperature reads in Kelvin and a gobo wheel reads as the gobo it is sitting on
 * rather than as a percentage of the channel behind either of them.
 */
export function attributeScale(
	controller: ParameterController,
	attribute: string,
	value: number,
): AttributeScale {
	const channelUnit = selectedChannelUnit(
		controller.selectedFixtures,
		controller.selectedFixtureIds,
		attribute,
	);
	const domain = attributeDomain(
		attribute,
		channelUnit?.unit ?? controller.attributeUnits.get(attribute),
		channelUnit ?? undefined,
	);
	const bands = attributeBands(
		controller.selectedFixtures,
		controller.selectedFixtureIds,
		attribute,
	);
	return {
		domain,
		stepsBySlot: bands !== null,
		display:
			controller.encoderSemanticDisplay(attribute) ??
			controller.encoderNormalizedDisplay(attribute) ??
			(bands ? bandLabel(bands, value) : undefined) ??
			formatAttributeValue(domain, value),
		stepAttribute(delta, undoGroup) {
			if (!bands) {
				void controller.stepParameter(attribute, delta, undoGroup);
				return;
			}
			// One detent is one slot, so the encoder never rests between two of them.
			const target = steppedValue(
				bands,
				value,
				delta < 0 ? -1 : 1,
				Math.abs(delta) > domainStep(domain, false),
			);
			if (target !== value) void controller.applyParameter(attribute, target);
		},
	};
}
