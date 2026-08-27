import { useEffect, useRef } from "react";
import { attributeBands, steppedValue } from "./attributeBands";
import {
	attributeDomain,
	domainStep,
	selectedChannelUnit,
} from "./attributeDomain";
import type { ParameterProjection } from "./useParameterProjection";

interface HardwareParameterActions {
	canWriteValues: boolean;
	relativeSteps: boolean;
	programmerTarget(attribute: string): number | undefined;
	programmerDiscreteTarget(attribute: string): string | undefined;
	applyParameter(
		attribute: string,
		level: number,
		undoGroup?: string | null,
		requestId?: string,
	): Promise<unknown>;
	stepParameter(
		attribute: string,
		delta: number,
		undoGroup?: string | null,
		requestId?: string,
	): Promise<unknown>;
}

interface AccumulatedEncoderValue {
	key: string;
	observedBase: number;
	value: number;
}

interface EncoderUndoGroup {
	key: string;
	id: string;
	lastSampleAt: number;
}

const ENCODER_UNDO_GROUP_IDLE_MILLIS = 250;

export function encoderDelta(
	attribute: string,
	value: string | undefined,
	unit?: string | null,
) {
	// Every byte-addressed media source steps one DMX address per detent, including the
	// audio.* names a show patched before TL-367 still declares.
	const addressStep =
		attribute === "media.folder" ||
		attribute === "media.file" ||
		attribute === "media.mask.folder" ||
		attribute === "media.mask.file" ||
		attribute === "audio.folder" ||
		attribute === "audio.file"
			? 1 / 255
			: null;
	// A detent moves the value the operator reads by a round amount of its own units, so a colour
	// temperature moves in Kelvin rather than in hundredths of the channel behind it.
	const domain = attributeDomain(attribute, unit);
	const fine = addressStep ?? domainStep(domain, false) * 10;
	const coarse = addressStep ?? domainStep(domain, true) * 10;
	if (value === "up") return fine;
	if (value === "down") return -fine;
	if (value === "right") return coarse;
	if (value === "left") return -coarse;
}

function targetKey(projection: ParameterProjection, attribute: string) {
	const target = projection.selectedGroupId
		? `group:${projection.selectedGroupId}`
		: `fixtures:${projection.selectedFixtureIds.join("\u0000")}`;
	return `${projection.programmerValuesRoute ?? "unavailable"}|${target}|${attribute}`;
}

function nextEncoderValue(
	current: AccumulatedEncoderValue | null,
	key: string,
	base: number,
	delta: number,
) {
	const externalChange =
		current &&
		current.key === key &&
		base !== current.observedBase &&
		base !== current.value;
	const start =
		!current || current.key !== key || externalChange ? base : current.value;
	return {
		key,
		observedBase: base,
		value: Math.max(0, Math.min(1, start + delta)),
	};
}

export function useHardwareParameterEncoders(
	projection: ParameterProjection,
	actions: HardwareParameterActions,
) {
	const latest = useRef({ projection, actions });
	const accumulated = useRef<AccumulatedEncoderValue | null>(null);
	const undoGroup = useRef<EncoderUndoGroup | null>(null);
	latest.current = { projection, actions };
	useEffect(() => {
		if (!projection.active) return;
		const handleEncoder = (event: Event) => {
			const { projection, actions } = latest.current;
			if (!actions.canWriteValues) return;
			const {
				control,
				value,
				request_id: requestId,
			} = (
				event as CustomEvent<{
					control: string;
					value?: string;
					request_id?: string;
				}>
			).detail;
			const slot = Number(control.split("/")[1]) - 1;
			const primaryAttribute = projection.encoderSlots[slot];
			const pushTurnAttribute = projection.encoderPushTurnSlots[slot];
			const attribute =
				pushTurnAttribute && (value === "left" || value === "right")
					? pushTurnAttribute
					: primaryAttribute;
			const delta = attribute
				? encoderDelta(
						attribute,
						value,
						selectedChannelUnit(
							projection.selectedFixtures,
							projection.selectedFixtureIds,
							attribute,
						)?.unit ?? projection.attributeUnits.get(attribute),
					)
				: undefined;
			if (!attribute || delta == null) return;
			// A wheel steps one slot per detent whichever surface the detent came from.
			const bands = attributeBands(
				projection.selectedFixtures,
				projection.selectedFixtureIds,
				attribute,
			);
			if (bands) {
				const base =
					actions.programmerTarget(attribute) ??
					projection.normalized.get(attribute) ??
					0;
				const target = steppedValue(
					bands,
					base,
					delta < 0 ? -1 : 1,
					value === "left" || value === "right",
				);
				if (target !== base) void actions.applyParameter(attribute, target);
				return;
			}
			if (
				actions.programmerDiscreteTarget(attribute) ??
				projection.discrete.get(attribute)
			)
				return;
			const key = targetKey(projection, attribute);
			const now = performance.now();
			if (
				!undoGroup.current ||
				undoGroup.current.key !== key ||
				now - undoGroup.current.lastSampleAt > ENCODER_UNDO_GROUP_IDLE_MILLIS
			)
				undoGroup.current = {
					key,
					id: crypto.randomUUID(),
					lastSampleAt: now,
				};
			else undoGroup.current.lastSampleAt = now;
			if (actions.relativeSteps) {
				void actions.stepParameter(
					attribute,
					delta,
					undoGroup.current.id,
					requestId,
				);
				return;
			}
			const base =
				actions.programmerTarget(attribute) ??
				projection.normalized.get(attribute) ??
				0;
			accumulated.current = nextEncoderValue(
				accumulated.current,
				key,
				base,
				delta,
			);
			void actions.applyParameter(
				attribute,
				accumulated.current.value,
				undoGroup.current.id,
				requestId,
			);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () => {
			accumulated.current = null;
			undoGroup.current = null;
			window.removeEventListener("light:encoder-action", handleEncoder);
		};
	}, [projection.active]);
}
