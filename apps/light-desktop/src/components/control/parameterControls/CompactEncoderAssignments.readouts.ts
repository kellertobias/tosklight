import { attributeScale } from "./attributeScale";
import { formatNormalizedValue, parameterLabels } from "./model";
import { familySlotUnsupported } from "./familyEncoders/familyEncoderBinding";
import type { ParameterController } from "./useParameterController";

export interface CompactEncoderReadout {
	label: string;
	text: string;
	assigned: boolean;
	detail?: string;
	pushTurn?: { label: string; text: string };
}

function attributeLabel(controller: ParameterController, attribute: string) {
	return parameterLabels[attribute] ?? controller.attributeLabels.get(attribute) ?? attribute.replaceAll(".", " ");
}

function spreadPoints(value: unknown): readonly number[] | null {
	if (!value || typeof value !== "object") return null;
	const record = value as Record<string, unknown>;
	if (record.kind === "spread" && Array.isArray(record.value) && record.value.every((point) => typeof point === "number" && Number.isFinite(point)))
		return record.value as number[];
	return record.value === value ? null : spreadPoints(record.value);
}

function legacyReadout(controller: ParameterController, attribute: string) {
	const label = attributeLabel(controller, attribute);
	const semantic = controller.encoderSemanticDisplay(attribute);
	if (semantic) return { label, text: semantic };
	const discrete = controller.encoderDiscreteDisplay(attribute);
	if (discrete !== undefined) return { label, text: discrete };
	const value = controller.programmerTarget(attribute) ?? controller.normalized.get(attribute);
	const scale = attributeScale(controller, attribute, value ?? 0);
	const entries = controller.selectedGroupId
		? controller.groupProgrammerValues.filter((entry) => entry.attribute === attribute)
		: controller.selectedFixtureIds.flatMap((id) => controller.programmerValues.filter((entry) => entry.fixtureId === id && entry.attribute === attribute));
	const spreads = entries.map((entry) => spreadPoints(entry.value));
	if (spreads.some(Boolean)) {
		const first = spreads[0];
		const shared = first && spreads.every((points) => points && points.length === first.length && points.every((point, index) => point === first[index]));
		return {
			label,
			text: shared ? first.map((point) => formatNormalizedValue(point, scale.domain)).join(" thru ") : "Mixed",
		};
	}
	// A missing readout is unknown, not a fabricated zero. The controller's display helper
	// owns live selection ranges, group values and indexed function names.
	const normalized = controller.encoderNormalizedDisplay(attribute, scale.domain);
	return { label, text: normalized ?? (value === undefined ? "—" : scale.display) };
}

/** Presentation of the exact slots the existing hardware controller binds; never writes. */
export function compactEncoderReadout(controller: ParameterController, index: number): CompactEncoderReadout {
	const slot = controller.familyEncoders?.componentSlot(index);
	if (slot) {
		const shown = controller.familyEncoders.display(index);
		const unsupported = shown.unsupported || familySlotUnsupported(slot);
		const noPoints = slot.edit === "target_reference" && shown.text === "—" && !controller.familyEncoders.pointChoices.some((choice) => choice.reference.kind === "point");
		return {
			label: slot.label,
			text: noPoints ? "No Points" : shown.text,
			assigned: true,
			...(unsupported ? { detail: "Unsupported" } : shown.source === "resolved" ? { detail: shown.provenance ?? "Resolved" } : shown.text.includes(" thru ") ? { detail: "Spread" } : {}),
		};
	}
	const attribute = controller.encoderSlots[index];
	if (!attribute) return { label: "Unassigned", text: "—", assigned: false };
	const readout = legacyReadout(controller, attribute);
	const pushTurn = controller.encoderPushTurnSlots[index];
	return {
		...readout,
		assigned: true,
		...(readout.text.includes(" thru ") ? { detail: "Spread" } : {}),
		...(pushTurn ? { pushTurn: legacyReadout(controller, pushTurn) } : {}),
	};
}

/** Bound long fixture spreads to the small card while keeping the full sequence in its label. */
export function compactEncoderValueText(text: string): string {
	const points = text.split(" thru ");
	if (points.length < 2) return text;
	if (points.every((point) => point === points[0])) return points[0];
	const parsed = points.map((point) => point.match(/^(-?\d+(?:\.\d+)?)(.*)$/));
	if (parsed.every((point) => point && point[2] === parsed[0]?.[2])) {
		const numbers = parsed.map((point) => Number(point![1]));
		const ascending = numbers.every((value, index) => index === 0 || value >= numbers[index - 1]);
		const descending = numbers.every((value, index) => index === 0 || value <= numbers[index - 1]);
		if (ascending || descending) return `${parsed[0]![1]}…${parsed.at(-1)![1]}${parsed[0]![2]}`;
		return `${Math.min(...numbers)}…${Math.max(...numbers)}${parsed[0]![2]} · Mixed`;
	}
	return `Mixed · ${points[0]}…${points.at(-1)}`;
}
