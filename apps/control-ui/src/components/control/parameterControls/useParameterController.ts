import { useState } from "react";
import { useApp } from "../../../state/AppContext";
import type {
	AlignMode,
	DirectControlChoice,
	DirectValueChoice,
	ParameterFamily,
} from "./model";
import {
	directParameterChoiceActive,
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
	setGenerationStatus: (status: string) => void,
) {
	const applyControlAction = async (
		choice: DirectControlChoice,
		active: boolean,
	) => {
		await Promise.all(
			choice.fixtureIds.map((fixtureId) =>
				projection.server.controlFixtureAction(
					fixtureId,
					choice.actionId,
					active,
				),
			),
		);
	};
	const generateDirectPresets = async () => {
		setGenerationStatus("Generating portable presets…");
		const result = await projection.server.generateFixturePresets(
			projection.directChoices.fixtureIds,
		);
		setGenerationStatus(generationMessage(result?.created.length));
	};
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
		applyControlAction,
		generateDirectPresets,
		directChoiceActive: (choice: DirectValueChoice) =>
			directParameterChoiceActive(projection, choice),
		hasProgrammerValue: (attribute: string) =>
			hasParameterValue(projection, attribute),
	};
}

function generationMessage(created: number | undefined) {
	if (created == null) return "Preset generation failed";
	return `Created ${created} portable preset${created === 1 ? "" : "s"}`;
}

export function useParameterController(active = true) {
	const { dispatch } = useApp();
	const [family, setFamily] = useState<ParameterFamily>("Intensity");
	const [directMode, setDirectMode] = useState(false);
	const [latchedActions, setLatchedActions] = useState<Record<string, boolean>>(
		{},
	);
	const [generationStatus, setGenerationStatus] = useState<string | null>(null);
	const [alignMode, setAlignMode] = useState<AlignMode | null>(null);
	const [dynamicsMode, setDynamicsMode] = useState(false);
	const projection = useParameterProjection(family, active);
	const valueActions = useParameterValueActions(projection);
	const actions = createParameterActions(
		projection,
		valueActions,
		setGenerationStatus,
	);
	useHardwareParameterEncoders(projection, actions, directMode);
	return {
		...projection,
		...actions,
		dispatch,
		family,
		setFamily,
		directMode,
		setDirectMode,
		latchedActions,
		setLatchedActions,
		generationStatus,
		alignMode,
		setAlignMode,
		dynamicsMode,
		setDynamicsMode,
	};
}

export type ParameterController = ReturnType<typeof useParameterController>;
