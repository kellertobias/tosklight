import { useEffect, useRef } from "react";
import type { ParameterProjection } from "./useParameterProjection";

interface HardwareParameterActions {
	canWriteValues: boolean;
	programmerTarget(attribute: string): number | undefined;
	programmerDiscreteTarget(attribute: string): string | undefined;
	applyParameter(attribute: string, level: number): Promise<unknown>;
}

interface AccumulatedEncoderValue {
	key: string;
	observedBase: number;
	value: number;
}

function encoderDelta(value: string | undefined) {
	if (value === "up") return 0.01;
	if (value === "down") return -0.01;
	if (value === "right") return 0.1;
	if (value === "left") return -0.1;
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
	directMode: boolean,
) {
	const latest = useRef({ projection, actions });
	const accumulated = useRef<AccumulatedEncoderValue | null>(null);
	latest.current = { projection, actions };
	useEffect(() => {
		if (!projection.active || !projection.hardwareConnected || directMode)
			return;
		const handleEncoder = (event: Event) => {
			const { projection, actions } = latest.current;
			if (!actions.canWriteValues) return;
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const attribute =
				projection.encoderSlots[Number(control.split("/")[1]) - 1];
			const delta = encoderDelta(value);
			if (!attribute || delta == null) return;
			if (
				actions.programmerDiscreteTarget(attribute) ??
				projection.discrete.get(attribute)
			)
				return;
			const base =
				actions.programmerTarget(attribute) ??
				projection.normalized.get(attribute) ??
				0;
			accumulated.current = nextEncoderValue(
				accumulated.current,
				targetKey(projection, attribute),
				base,
				delta,
			);
			void actions.applyParameter(attribute, accumulated.current.value);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () => {
			accumulated.current = null;
			window.removeEventListener("light:encoder-action", handleEncoder);
		};
	}, [projection.active, projection.hardwareConnected, directMode]);
}
