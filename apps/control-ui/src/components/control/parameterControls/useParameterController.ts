import { type Dispatch, type SetStateAction, useEffect, useState } from "react";
import { useApp } from "../../../state/AppContext";
import {
	type AlignMode,
	type ParameterFamily,
	parameterFamilyOrder,
} from "./model";
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

function useHardwareParameterNavigation(
	active: boolean,
	setFamily: Dispatch<SetStateAction<ParameterFamily>>,
) {
	useEffect(() => {
		if (!active) return;
		const handleNavigation = (event: Event) => {
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			if (control !== "nav") return;
			const direction =
				value === "up" || value === "left"
					? -1
					: value === "down" || value === "right"
						? 1
						: 0;
			if (!direction) return;
			setFamily((current) => {
				const currentIndex = parameterFamilyOrder.indexOf(current);
				const nextIndex =
					(currentIndex + direction + parameterFamilyOrder.length) %
					parameterFamilyOrder.length;
				return parameterFamilyOrder[nextIndex];
			});
		};
		window.addEventListener("light:encoder-action", handleNavigation);
		return () =>
			window.removeEventListener("light:encoder-action", handleNavigation);
	}, [active, setFamily]);
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
	useHardwareParameterNavigation(
		projection.active && projection.hardwareConnected,
		setFamily,
	);
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
