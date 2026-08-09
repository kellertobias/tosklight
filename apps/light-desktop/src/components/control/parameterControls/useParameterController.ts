import { useEffect, useState } from "react";
import { useApp } from "../../../state/AppContext";
import {
	type AlignMode,
	alignModes,
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
	const [requestedEncoderPage, setRequestedEncoderPage] = useState(1);
	const [encoderPageAnchor, setEncoderPageAnchor] = useState<string | null>(
		null,
	);
	const [alignMode, setAlignModeState] = useState<AlignMode | null>(null);
	const [alignAttribute, setAlignAttribute] = useState<string | null>(null);
	const [dynamicsMode, setDynamicsMode] = useState(false);
	const projection = useParameterProjection(
		family,
		requestedEncoderPage,
		active,
		encoderPageAnchor,
	);
	const valueActions = useParameterValueActions(projection);
	const rawActions = createParameterActions(projection, valueActions);
	const setAlignMode = (mode: AlignMode | null) => {
		setAlignModeState(mode);
		setAlignAttribute(null);
	};
	const actions = {
		...rawActions,
		stepParameter: (
			attribute: string,
			delta: number,
			undoGroup?: string | null,
			requestId?: string,
		) => {
			if (alignMode && alignAttribute == null) setAlignAttribute(attribute);
			else if (alignMode && alignAttribute !== attribute) setAlignMode(null);
			return rawActions.stepParameter(attribute, delta, undoGroup, requestId);
		},
	};
	useHardwareParameterEncoders(projection, actions);
	const selectEncoderGroup = (next: ParameterFamily, page: number) => {
		const group = projection.encoderGroups.find(
			(candidate) => candidate.id === next.toLowerCase(),
		);
		const anchor = group?.pages[page - 1]?.slots.find(Boolean)?.id ?? null;
		setFamily(next);
		setRequestedEncoderPage(page);
		setEncoderPageAnchor(anchor);
	};
	useHardwareParameterNavigation(projection.active, family, (next) =>
		selectEncoderGroup(next, 1),
	);
	useEffect(() => {
		if (!projection.active || !projection.programmerActions) return;
		const programmerActions = projection.programmerActions;
		const handleAlign = () => {
			const nextIndex =
				alignMode == null ? 0 : alignModes.indexOf(alignMode) + 1;
			const next =
				nextIndex >= alignModes.length ? null : alignModes[nextIndex];
			void programmerActions
				.alignSelection(next ?? "off")
				.then(() => setAlignMode(next))
				.catch(() => undefined);
		};
		window.addEventListener("light:align-action", handleAlign);
		return () => window.removeEventListener("light:align-action", handleAlign);
	}, [alignMode, projection.active, projection.programmerActions]);
	useEffect(() => {
		const group = projection.encoderGroups.find(
			(candidate) => candidate.id === family.toLowerCase(),
		);
		const anchorStillPresent = group?.pages.some((page) =>
			page.slots.some((descriptor) => descriptor?.id === encoderPageAnchor),
		);
		if (!anchorStillPresent) {
			const anchor =
				group?.pages[projection.encoderPage - 1]?.slots.find(Boolean)?.id ??
				null;
			setEncoderPageAnchor(anchor);
		}
		setRequestedEncoderPage(projection.encoderPage);
	}, [
		family,
		projection.encoderGroups,
		projection.encoderPage,
		encoderPageAnchor,
	]);
	return {
		...projection,
		...actions,
		dispatch,
		family,
		setFamily,
		encoderPage: projection.encoderPage,
		encoderPageAnchor,
		selectEncoderGroup,
		alignMode,
		setAlignMode,
		dynamicsMode,
		setDynamicsMode,
	};
}

export type ParameterController = ReturnType<typeof useParameterController>;
