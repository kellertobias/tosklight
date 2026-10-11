import { useEffect, useRef, useState } from "react";
import { useSessionSnapshot } from "../../../features/deskSnapshot/DeskSnapshotState";
import { useEncoderNavigation } from "./useEncoderNavigation";
import type { EncoderOwnerContext } from "./useEncoderNavigation";
import { useDynamicEditorSession } from "../../../features/dynamics/DynamicEditorSessionContext";
import { useProgrammingAlignmentView } from "../../../features/programmingInteraction/ProgrammingInteractionView";
import type { AttributeDomain } from "./attributeDomain";
import { useApp } from "../../../state/AppContext";
import {
	type ParameterFamily,
	parameterFamilyOrder,
} from "./model";
import {
	discreteParameterDisplay,
	discreteParameterTarget,
	hasParameterValue,
	normalizedParameterDisplay,
	normalizedParameterTarget,
	parameterSemanticDisplay,
} from "./parameterProgrammerState";
import { useFamilyEncoderBinding } from "./familyEncoders/useFamilyEncoderBinding";
import { useHardwareParameterEncoders } from "./useHardwareParameterEncoders";
import {
	type ParameterProjection,
	useParameterProjection,
} from "./useParameterProjection";
import { useParameterValueActions } from "./useParameterValueActions";

export function parameterTargetForShiftDigit(
	key: string,
):
	| { type: "all" }
	| { type: "dynamics" }
	| { type: "family"; family: ParameterFamily }
	| null {
	if (key === "0") return { type: "all" };
	if (key === "5") return { type: "dynamics" };
	const family: Partial<Record<string, ParameterFamily>> = {
		"1": "Intensity",
		"2": "Color",
		"3": "Position",
		"4": "Beam",
		"6": "Shapers",
		"7": "Focus",
		"8": "Control",
		"9": "Media",
	};
	return family[key] ? { type: "family", family: family[key] } : null;
}

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
		encoderNormalizedDisplay: (attribute: string, domain?: AttributeDomain) =>
			normalizedParameterDisplay(projection, attribute, domain),
		encoderDiscreteDisplay: (attribute: string) =>
			discreteParameterDisplay(projection, attribute),
		encoderSemanticDisplay: (attribute: string) =>
			parameterSemanticDisplay(projection, attribute),
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

export function useParameterController(
	active = true,
	{ bindHardware = true, ownerContext = "programmer" }: { bindHardware?: boolean; ownerContext?: EncoderOwnerContext } = {},
) {
	const { dispatch } = useApp();
	const session = useSessionSnapshot();
	const [navigation, selectNavigation, navigationActions] = useEncoderNavigation(session?.desk.id ?? null);
	const editor = useDynamicEditorSession();
	const localOwnerContext = ownerContext === "programmer" && editor.session ? "dynamic-editor" : ownerContext;
	useEffect(() => {
		if (bindHardware) navigationActions.publishOwnerContext(localOwnerContext);
	}, [bindHardware, localOwnerContext, navigationActions.publishOwnerContext]);
	const encoderContext = bindHardware ? localOwnerContext : navigation.ownerContext;
	const family = navigation.family;
	const requestedEncoderPage = navigation.page;
	const setFamily = (next: ParameterFamily) => selectNavigation(next, 1);
	const [encoderPageAnchor, setEncoderPageAnchor] = useState<string | null>(
		null,
	);
	const anchorNavigation = useRef(navigation);
	const effectiveAnchor = anchorNavigation.current === navigation ? encoderPageAnchor : null;
	useEffect(() => {
		if (anchorNavigation.current !== navigation) {
			anchorNavigation.current = navigation;
			setEncoderPageAnchor(null);
		}
	}, [navigation]);
	const alignment = useProgrammingAlignmentView(active);
	const alignMode = alignment?.mode === "off" ? null : alignment?.mode ?? null;
	const dynamicsMode = navigation.dynamicsMode;
	const setDynamicsMode = navigationActions.setDynamicsMode;
	const projection = useParameterProjection(
		family,
		requestedEncoderPage,
		active,
		effectiveAnchor,
	);
	const valueActions = useParameterValueActions(projection);
	const rawActions = createParameterActions(projection, valueActions);
	const actions = rawActions;
	const familyEncoders = useFamilyEncoderBinding(projection, family);
	useHardwareParameterEncoders(
		{ ...projection, ...familyEncoders.overrides, active: projection.active && bindHardware && encoderContext === "programmer" && !dynamicsMode },
		{ ...actions, familyEncoderDetent: familyEncoders.detent },
	);
	useEffect(() => {
		familyEncoders.selectPage(family, requestedEncoderPage);
	}, [family, requestedEncoderPage, familyEncoders.active]);
	const selectEncoderGroup = (next: ParameterFamily, page: number) => {
		familyEncoders.selectPage(next, page);
		const group = projection.encoderGroups.find(
			(candidate) => candidate.id === next.toLowerCase(),
		);
		const anchor = group?.pages[page - 1]?.slots.find(Boolean)?.id ?? null;
		selectNavigation(next, page);
		setEncoderPageAnchor(anchor);
	};
	useHardwareParameterNavigation(projection.active && bindHardware && encoderContext === "programmer", family, (next) =>
		selectEncoderGroup(next, 1),
	);
	useEffect(() => {
		if (!projection.active || !bindHardware || encoderContext !== "programmer" || !projection.programmerActions) return;
		const programmerActions = projection.programmerActions;
		const handleAlign = () => {
			void programmerActions
				.alignSelection("cycle")
				.catch(() => undefined);
		};
		const handleAlignOff = () => {
			void programmerActions
				.alignSelection("off")
				.catch(() => undefined);
		};
		window.addEventListener("light:align-action", handleAlign);
		window.addEventListener("light:align-off", handleAlignOff);
		return () => {
			window.removeEventListener("light:align-action", handleAlign);
			window.removeEventListener("light:align-off", handleAlignOff);
		};
	}, [projection.active, projection.programmerActions, bindHardware, encoderContext]);
	useEffect(() => {
		if (!active || !bindHardware) return;
		const selectFromKey = (event: Event) => {
			const key = (event as CustomEvent<string>).detail;
			const target = parameterTargetForShiftDigit(key);
			if (target?.type === "dynamics") {
				setDynamicsMode(true);
				return;
			}
			setDynamicsMode(false);
			if (target?.type === "all") {
				selectEncoderGroup("Intensity", 1);
				return;
			}
			if (target?.type === "family") selectEncoderGroup(target.family, 1);
		};
		window.addEventListener("light:parameter-family-key", selectFromKey);
		return () =>
			window.removeEventListener("light:parameter-family-key", selectFromKey);
	}, [active, bindHardware, selectEncoderGroup]);
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
		...familyEncoders.overrides,
		familyEncoders,
		encoderPageAnchor,
		selectEncoderGroup,
		alignMode,
		dynamicsMode,
		setDynamicsMode,
		encoderContext,
	};
}

export type ParameterController = ReturnType<typeof useParameterController>;
