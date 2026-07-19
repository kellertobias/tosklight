import { useEffect, useRef, useState } from "react";
import type { AttributeValue } from "../../../api/types";
import { useApp } from "../../../state/AppContext";
import {
	type AlignMode,
	type DirectControlChoice,
	type DirectValueChoice,
	discreteProgrammerTarget,
	formatDiscreteValues,
	formatNormalizedRange,
	formatNormalizedValue,
	normalizedProgrammerTarget,
	type ParameterFamily,
} from "./model";
import {
	type ParameterProjection as Projection,
	useParameterProjection,
} from "./useParameterProjection";

function programmerEntry(
	projection: Projection,
	fixtureId: string,
	attribute: string,
) {
	return projection.programmerValues.find(
		(candidate) =>
			candidate.fixtureId === fixtureId && candidate.attribute === attribute,
	);
}

function groupProgrammerEntry(projection: Projection, attribute: string) {
	return projection.groupProgrammerValues.find(
		(candidate) => candidate.attribute === attribute,
	);
}

function normalizedTarget(projection: Projection, attribute: string) {
	if (projection.selectedGroupId)
		return normalizedProgrammerTarget(
			groupProgrammerEntry(projection, attribute)?.value,
		);
	for (const fixtureId of projection.selectedFixtureIds) {
		const target = normalizedProgrammerTarget(
			programmerEntry(projection, fixtureId, attribute)?.value,
		);
		if (target != null) return target;
	}
}

function discreteTarget(projection: Projection, attribute: string) {
	if (projection.selectedGroupId)
		return discreteProgrammerTarget(
			groupProgrammerEntry(projection, attribute)?.value,
		);
	for (const fixtureId of projection.selectedFixtureIds) {
		const target = discreteProgrammerTarget(
			programmerEntry(projection, fixtureId, attribute)?.value,
		);
		if (target != null) return target;
	}
}

function normalizedDisplay(projection: Projection, attribute: string) {
	if (projection.selectedGroupId) {
		const target = normalizedTarget(projection, attribute);
		return target == null ? undefined : formatNormalizedValue(target);
	}
	return formatNormalizedRange(
		projection.selectedFixtureIds.flatMap((fixtureId) => {
			const target = normalizedProgrammerTarget(
				programmerEntry(projection, fixtureId, attribute)?.value,
			);
			const value =
				target ?? projection.normalizedByFixture.get(fixtureId)?.get(attribute);
			return value == null ? [] : [value];
		}),
	);
}

function discreteDisplay(projection: Projection, attribute: string) {
	if (projection.selectedGroupId) return discreteTarget(projection, attribute);
	return formatDiscreteValues(
		projection.selectedFixtureIds.flatMap((fixtureId) => {
			const target = discreteProgrammerTarget(
				programmerEntry(projection, fixtureId, attribute)?.value,
			);
			const value =
				target ?? projection.discreteByFixture.get(fixtureId)?.get(attribute);
			return value == null ? [] : [value];
		}),
	);
}

function spreadValue(points: number[], index: number, count: number) {
	if (points.length === 1 || count <= 1) return points[0] ?? 0;
	const position = (index * (points.length - 1)) / (count - 1);
	const left = Math.floor(position);
	const right = Math.ceil(position);
	return points[left] + (points[right] - points[left]) * (position - left);
}

function createParameterActions(
	projection: Projection,
	setGenerationStatus: (status: string) => void,
) {
	const { server } = projection;
	const applyParameter = async (attribute: string, level: number) => {
		if (projection.selectedGroupId) {
			await server.setGroupValue(attribute, level);
			return;
		}
		await Promise.all(
			projection.selectedFixtureIds.map((fixtureId) =>
				server.setProgrammer(fixtureId, attribute, level),
			),
		);
	};
	const applyParameterRange = async (
		attribute: string,
		percentages: number[],
	) => {
		const points = percentages.map(
			(value) => Math.max(0, Math.min(100, value)) / 100,
		);
		if (projection.selectedGroupId) {
			await server.setGroupValue(attribute, { kind: "spread", value: points });
			return;
		}
		await server.setProgrammerMany(
			projection.selectedFixtureIds.map((fixtureId, index) => ({
				fixtureId,
				attribute,
				value: spreadValue(points, index, projection.selectedFixtureIds.length),
			})),
		);
	};
	const releaseParameter = async (attribute: string) => {
		if (projection.selectedGroupId) {
			await server.releaseGroupValue(attribute);
			return;
		}
		const fixtureIds = new Set(
			projection.programmerValues
				.filter((entry) => entry.attribute === attribute)
				.map((entry) => entry.fixtureId),
		);
		await Promise.all(
			projection.selectedFixtureIds
				.filter((fixtureId) => fixtureIds.has(fixtureId))
				.map((fixtureId) => server.releaseProgrammer(fixtureId, attribute)),
		);
	};
	const applyDirectValue = async (choice: DirectValueChoice) => {
		const value: AttributeValue = {
			kind: "discrete",
			value: choice.semanticId,
		};
		await Promise.all(
			choice.assignments.map((assignment) =>
				server.setProgrammerValue(
					assignment.fixtureId,
					assignment.attribute,
					value,
				),
			),
		);
	};
	const applyControlAction = async (
		choice: DirectControlChoice,
		active: boolean,
	) => {
		await Promise.all(
			choice.fixtureIds.map((fixtureId) =>
				server.controlFixtureAction(fixtureId, choice.actionId, active),
			),
		);
	};
	const generateDirectPresets = async () => {
		setGenerationStatus("Generating portable presets…");
		const result = await server.generateFixturePresets(
			projection.directChoices.fixtureIds,
		);
		setGenerationStatus(
			result
				? `Created ${result.created.length} portable preset${result.created.length === 1 ? "" : "s"}`
				: "Preset generation failed",
		);
	};
	const directChoiceActive = (choice: DirectValueChoice) =>
		choice.assignments.some((assignment) =>
			projection.programmerValues.some(
				(entry) =>
					entry.fixtureId === assignment.fixtureId &&
					entry.attribute === assignment.attribute &&
					discreteProgrammerTarget(entry.value) === choice.semanticId,
			),
		);
	const hasProgrammerValue = (attribute: string) =>
		projection.selectedGroupId
			? projection.groupProgrammerValues.some(
					(entry) => entry.attribute === attribute,
				)
			: projection.programmerValues.some(
					(entry) => entry.attribute === attribute,
				);
	return {
		programmerTarget: (attribute: string) =>
			normalizedTarget(projection, attribute),
		programmerDiscreteTarget: (attribute: string) =>
			discreteTarget(projection, attribute),
		encoderNormalizedDisplay: (attribute: string) =>
			normalizedDisplay(projection, attribute),
		encoderDiscreteDisplay: (attribute: string) =>
			discreteDisplay(projection, attribute),
		applyParameter,
		applyParameterRange,
		releaseParameter,
		applyDirectValue,
		applyControlAction,
		generateDirectPresets,
		directChoiceActive,
		hasProgrammerValue,
	};
}

type ParameterActions = ReturnType<typeof createParameterActions>;

function useHardwareEncoders(
	projection: Projection,
	actions: ParameterActions,
	directMode: boolean,
) {
	const latest = useRef({ projection, actions });
	latest.current = { projection, actions };
	useEffect(() => {
		if (!projection.active || !projection.hardwareConnected || directMode)
			return;
		const handleEncoder = (event: Event) => {
			const { projection, actions } = latest.current;
			const { control, value } = (
				event as CustomEvent<{
					control: string;
					value?: string;
				}>
			).detail;
			const attribute =
				projection.encoderSlots[Number(control.split("/")[1]) - 1];
			if (!attribute || !["up", "down", "left", "right"].includes(value ?? ""))
				return;
			if (
				actions.programmerDiscreteTarget(attribute) ??
				projection.discrete.get(attribute)
			)
				return;
			const current =
				actions.programmerTarget(attribute) ??
				projection.normalized.get(attribute) ??
				0;
			const delta =
				value === "up"
					? 0.01
					: value === "down"
						? -0.01
						: value === "right"
							? 0.1
							: -0.1;
			void actions.applyParameter(
				attribute,
				Math.max(0, Math.min(1, current + delta)),
			);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	}, [projection.active, projection.hardwareConnected, directMode]);
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
	const actions = createParameterActions(projection, setGenerationStatus);
	useHardwareEncoders(projection, actions, directMode);
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
