import { useEffect, useState } from "react";
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
	family: ParameterFamily,
	onFamily: (family: ParameterFamily) => void,
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
			const currentIndex = parameterFamilyOrder.indexOf(family);
			const nextIndex =
				(currentIndex + direction + parameterFamilyOrder.length) %
				parameterFamilyOrder.length;
			onFamily(parameterFamilyOrder[nextIndex]);
		};
		window.addEventListener("light:encoder-action", handleNavigation);
		return () =>
			window.removeEventListener("light:encoder-action", handleNavigation);
	}, [active, family, onFamily]);
}

export function useParameterController(active = true) {
	const { dispatch } = useApp();
	const [family, setFamily] = useState<ParameterFamily>("Intensity");
	const [encoderPage, setEncoderPage] = useState(1);
	const [alignMode, setAlignMode] = useState<AlignMode | null>(null);
	const [dynamicsMode, setDynamicsMode] = useState(false);
	const projection = useParameterProjection(family, encoderPage, active);
	const valueActions = useParameterValueActions(projection);
	const actions = createParameterActions(projection, valueActions);
	useHardwareParameterEncoders(projection, actions);
	useHardwareParameterNavigation(projection.active, family, (next) => {
		setFamily(next);
		setEncoderPage(1);
	});
	const selectEncoderGroup = (next: ParameterFamily, page: number) => {
		setFamily(next);
		setEncoderPage(page);
	};
	return {
		...projection,
		...actions,
		dispatch,
		family,
		setFamily,
		encoderPage,
		selectEncoderGroup,
		alignMode,
		setAlignMode,
		dynamicsMode,
		setDynamicsMode,
	};
}

export type ParameterController = ReturnType<typeof useParameterController>;
