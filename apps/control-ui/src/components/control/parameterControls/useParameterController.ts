import { useState } from "react";
import { useApp } from "../../../state/AppContext";
import type { AlignMode, ParameterFamily } from "./model";
import {
	discreteParameterDisplay,
	discreteParameterTarget,
	hasParameterValue,
	normalizedParameterDisplay,
	normalizedParameterTarget,
} from "./parameterProgrammerState";
import { useHardwareParameterEncoders } from "./useHardwareParameterEncoders";
import {
	type ParameterProjection,
	useParameterProjection,
} from "./useParameterProjection";
import { useParameterValueActions } from "./useParameterValueActions";

function createParameterActions(
	projection: ParameterProjection,
	valueActions: ReturnType<typeof useParameterValueActions>,
) {
	return {
		...valueActions,
		programmerTarget: (attribute: string) =>
			normalizedParameterTarget(projection, attribute),
		programmerDiscreteTarget: (attribute: string) =>
			discreteParameterTarget(projection, attribute),
		encoderNormalizedDisplay: (attribute: string) =>
			normalizedParameterDisplay(projection, attribute),
		encoderDiscreteDisplay: (attribute: string) =>
			discreteParameterDisplay(projection, attribute),
		hasProgrammerValue: (attribute: string) =>
			hasParameterValue(projection, attribute),
	};
}

export function useParameterController(active = true) {
	const { dispatch } = useApp();
	const [family, setFamily] = useState<ParameterFamily>("Intensity");
	const [alignMode, setAlignMode] = useState<AlignMode | null>(null);
	const [dynamicsMode, setDynamicsMode] = useState(false);
	const projection = useParameterProjection(family, active);
	const valueActions = useParameterValueActions(projection);
	const actions = createParameterActions(projection, valueActions);
	useHardwareParameterEncoders(projection, actions);
	return {
		...projection,
		...actions,
		dispatch,
		family,
		setFamily,
		alignMode,
		setAlignMode,
		dynamicsMode,
		setDynamicsMode,
	};
}

export type ParameterController = ReturnType<typeof useParameterController>;
