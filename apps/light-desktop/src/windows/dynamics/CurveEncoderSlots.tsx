import { type ScalarDynamicLane } from "../../features/dynamics/laneModel";
import {
	commitEditableLane,
	dynamicLaneDomain,
	editableLane,
	editableRandomRange,
} from "../../features/dynamics/editableLane";
import {
	type DynamicLaneDomain,
	formatLaneValue,
	laneDomainForKey,
} from "../../features/dynamics/laneDomain";
import type { DynamicLaneProjection } from "../../api/types";
import {
	Button,
	ColorPickerField,
	CyclingValueToggle,
	FadedDivider,
	FormLayout,
	GroupedSelectionField,
	IconPickerField,
	MultiValueToggle,
	MultiValueToggleField,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "@tosklight/ui";
import {
	EncoderSection,
	type EncoderSectionItem,
	type HardwareEncoderDisplayHandle,
} from "@tosklight/ui/encoders";
import { ModalFrame } from "@tosklight/ui/modals";
import {
	PoolCard,
	PoolGrid,
	type PoolSlotViewModel,
} from "@tosklight/ui/pools";
import {
	WindowHeader,
	WindowScrollArea,
	WindowSettings,
} from "@tosklight/ui/window-kit";
import {
	type CSSProperties,
	type MutableRefObject,
	type ReactNode,
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import { createLightApi } from "../../api/client/api";
import type {
	DynamicDefinitionProjection,
	DynamicDefinitionStatusProjection,
	DynamicLaneModeProjection,
	DynamicPeriodicFunctionProjection,
	DynamicPhaseOrderingProjection,
	DynamicRandomGroupProjection,
	DynamicRuntimeSnapshotProjection,
	DynamicScalarSourceProjection,
	DynamicUpdateIntent,
	SpeedGroupId,
} from "../../api/types";
import { useCommandLineSurface } from "../../components/control/commandLine/useCommandLineSurface";
import { monotonicEpochMillis } from "../../components/control/soundToLightAnalyzer";
import { useSoundToLight } from "../../components/control/useSoundToLight";
import {
	useActiveShowId,
	useAttributeRegistry,
	useHardwareConnected,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { useDynamicEditorSession } from "../../features/dynamics/DynamicEditorSessionContext";
import { DynamicMutationWriter } from "../../features/dynamics/DynamicMutationWriter";
import { useDynamicsActions } from "../../features/dynamics/DynamicsActionsContext";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../../features/programmingInteraction/ProgrammingInteractionView";
import type { ShowObject } from "../../features/showObjects/contracts";
import {
	useDynamics,
	usePresets,
	useShowObjectsStore,
} from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useSpeedGroupRuntimeView } from "../../features/speedGroupRuntime/SpeedGroupRuntimeView";
import { useApp } from "../../state/AppContext";
import {
	clamp,
	keyframeName,
	laneShapeLabel,
	primaryInterpolationIndex,
	primaryInterpolationLabel,
	rationalFromNumber,
	rationalValue,
	setPrimaryInterpolation,
	sourceCurrent,
	wrappedIndex,
} from "./DynamicsEditor";
import { encoderChoices, type DynamicEncoderSlot } from "./DynamicEncoderDeck";

type PresetObject = ShowObject<"preset">;

function curveSlotFactories(
	lane: ScalarDynamicLane | undefined,
	domain: DynamicLaneDomain,
	onLaneChange: (
		update: (lane: ScalarDynamicLane) => ScalarDynamicLane,
		mutationGroup?: string,
	) => Promise<void>,
	presets: readonly PresetObject[],
) {
	const disabled = !lane;
	const unassigned = (id: string): DynamicEncoderSlot => ({
		id,
		label: "Unassigned",
		display: "—",
		value: 0,
		minimum: 0,
		maximum: 1,
		inputScale: 1,
		fineStep: 0.01,
		coarseStep: 0.1,
		disabled: true,
		apply: async () => undefined,
	});
	const sourceSlot = (
		id: string,
		label: string,
		source: DynamicScalarSourceProjection | undefined,
		replace: (
			lane: ScalarDynamicLane,
			source: DynamicScalarSourceProjection,
		) => ScalarDynamicLane,
	): DynamicEncoderSlot => ({
		id,
		label,
		display: scalarSourceEncoderDisplay(source, domain),
		value: scalarSourceEncoderValue(source, domain),
		minimum: domain.minimum,
		maximum: domain.maximum,
		inputScale: domain.inputScale,
		fineStep: domain.fineStep,
		coarseStep: domain.coarseStep,
		disabled,
		presets: source
			? scalarSourcePresetChoices(presets, domain.presetKey, source)
			: undefined,
		apply: (value, group) =>
			onLaneChange(
				(item) =>
					normalizePwmLane(
						replace(item, {
							type: "value",
							value: clamp(value, domain.minimum, domain.maximum),
						}),
					),
				group,
			),
		selectPreset: (value, group) =>
			onLaneChange(
				(item) =>
					normalizePwmLane(
						replace(item, scalarSourceFromPresetChoice(value, item.attribute)),
					),
				group,
			),
	});
	const speedSlot: DynamicEncoderSlot = {
		id: "lane-speed",
		label: "Speed",
		display: lane
			? `${lane.speed_multiplier.numerator}/${lane.speed_multiplier.denominator}`
			: "—",
		value: lane ? rationalValue(lane.speed_multiplier) : 1,
		minimum: 0.0625,
		maximum: 16,
		inputScale: 1,
		fineStep: 0.0625,
		coarseStep: 0.5,
		disabled,
		apply: (value, group) =>
			onLaneChange(
				(item) => ({
					...item,
					speed_multiplier: rationalFromNumber(value),
				}),
				group,
			),
	};
	const widthSlot: DynamicEncoderSlot = {
		id: "curve-width",
		label: "Curve width",
		display: lane ? `${Math.round(lane.width * 100)}%` : "—",
		value: lane?.width ?? 1,
		minimum: 0.05,
		maximum: 1,
		inputScale: 100,
		fineStep: 0.01,
		coarseStep: 0.1,
		disabled,
		apply: (value, group) =>
			onLaneChange(
				(item) => ({ ...item, width: clamp(value, 0.05, 1) }),
				group,
			),
	};
	return { disabled, unassigned, sourceSlot, speedSlot, widthSlot };
}

function keyframeCurveSlots({
	lane,
	keyframeIndex,
	onKeyframeIndex,
	onLaneChange,
	disabled,
	sourceSlot,
	widthSlot,
	speedSlot,
}: {
	lane: ScalarDynamicLane | undefined;
	keyframeIndex: number;
	onKeyframeIndex(index: number): void;
	onLaneChange: Parameters<typeof curveSlotFactories>[2];
	disabled: boolean;
	sourceSlot: ReturnType<typeof curveSlotFactories>["sourceSlot"];
	widthSlot: DynamicEncoderSlot;
	speedSlot: DynamicEncoderSlot;
}): DynamicEncoderSlot[] {
	const resolvedIndex = Math.min(
		keyframeIndex,
		Math.max(0, (lane?.keyframes.points.length ?? 1) - 1),
	);
	const point = lane?.keyframes.points[resolvedIndex];
	return [
		{
			id: "keyframe",
			label: "Keyframe",
			display: point
				? `${keyframeName(resolvedIndex)} · ${resolvedIndex + 1}/${lane?.keyframes.points.length ?? 1}`
				: "—",
			value: resolvedIndex,
			minimum: 0,
			maximum: Math.max(0, (lane?.keyframes.points.length ?? 1) - 1),
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			disabled,
			choices: encoderChoices(
				"Keyframe",
				resolvedIndex,
				(lane?.keyframes.points ?? []).map((_, index) => ({
					label: keyframeName(index),
					description: `${index + 1}/${lane?.keyframes.points.length ?? 1}`,
				})),
			),
			apply: async (value) =>
				onKeyframeIndex(
					wrappedIndex(value, lane?.keyframes.points.length ?? 1),
				),
		},
		sourceSlot(
			"keyframe-value",
			point?.source.type === "value" ? "Value" : "Source value",
			point?.source,
			(item, source) => ({
				...item,
				keyframes: {
					...item.keyframes,
					points: item.keyframes.points.map((candidate, index) =>
						index === resolvedIndex ? { ...candidate, source } : candidate,
					),
				},
			}),
		),
		{
			id: "keyframe-time",
			label: "Keyframe time",
			display: point ? `${Math.round(point.position * 100)}%` : "—",
			value: point?.position ?? 0,
			minimum: 0,
			maximum: 1,
			inputScale: 100,
			fineStep: 0.01,
			coarseStep: 0.1,
			disabled: disabled || resolvedIndex === 0,
			apply: (value, group) =>
				onLaneChange((item) => {
					const previous =
						item.keyframes.points[resolvedIndex - 1]?.position ?? 0;
					const next =
						item.keyframes.points[resolvedIndex + 1]?.position ?? 0.999;
					return {
						...item,
						keyframes: {
							...item.keyframes,
							points: item.keyframes.points.map((candidate, index) =>
								index === resolvedIndex
									? {
											...candidate,
											position: clamp(value, previous + 0.01, next - 0.01),
										}
									: candidate,
							),
						},
					};
				}, group),
		},
		{
			id: "interpolation",
			label: "Interpolation",
			display: lane ? primaryInterpolationLabel(lane) : "—",
			value: lane ? primaryInterpolationIndex(lane) : 0,
			minimum: 0,
			maximum: interpolations.length - 1,
			inputScale: 1,
			fineStep: 1,
			coarseStep: 1,
			disabled,
			choices: encoderChoices(
				"Interpolation",
				lane ? primaryInterpolationIndex(lane) : 0,
				[
					{ label: "Linear" },
					{ label: "Ease in" },
					{ label: "Ease out" },
					{ label: "Ease in + out" },
					{ label: "Hold" },
					{ label: "Drop" },
				],
			),
			apply: (value, group) =>
				onLaneChange((item) => setPrimaryInterpolation(item, value), group),
		},
		widthSlot,
		speedSlot,
	];
}

function scalarCurveEditorEncoderSlots(
	lane: ScalarDynamicLane | undefined,
	domain: DynamicLaneDomain,
	dynamic: DynamicDefinitionProjection,
	keyframeIndex: number,
	onKeyframeIndex: (index: number) => void,
	onLaneChange: (
		update: (lane: ScalarDynamicLane) => ScalarDynamicLane,
		mutationGroup?: string,
	) => Promise<void>,
	presets: readonly PresetObject[],
): DynamicEncoderSlot[] {
	const { disabled, unassigned, sourceSlot, speedSlot, widthSlot } =
		curveSlotFactories(lane, domain, onLaneChange, presets);
	if (!lane || lane.mode === "keyframes")
		return keyframeCurveSlots({
			lane,
			keyframeIndex,
			onKeyframeIndex,
			onLaneChange,
			disabled,
			sourceSlot,
			widthSlot,
			speedSlot,
		});
	if (lane.mode === "middle_amplitude") {
		const valueSlots: DynamicEncoderSlot[] = [
			sourceSlot(
				"middle",
				"Middle",
				lane.middle_amplitude.middle,
				(item, middle) => ({
					...item,
					middle_amplitude: { ...item.middle_amplitude, middle },
				}),
			),
			{
				id: "amplitude",
				label: "Amplitude",
				display: formatLaneValue(domain, lane.middle_amplitude.amplitude),
				value: lane.middle_amplitude.amplitude,
				minimum: 0,
				maximum: domain.maximum - domain.minimum,
				inputScale: domain.inputScale,
				fineStep: domain.fineStep,
				coarseStep: domain.coarseStep,
				apply: (value, group) =>
					onLaneChange(
						(item) => ({
							...item,
							middle_amplitude: {
								...item.middle_amplitude,
								amplitude: clamp(value, 0, domain.maximum - domain.minimum),
							},
						}),
						group,
					),
			},
		];
		return lane.middle_amplitude.function === "pwm"
			? [
					...middleAmplitudePwmValueSlots(lane, domain, onLaneChange),
					...pwmEncoderSlots(lane, onLaneChange),
					speedSlot,
				]
			: [
					...valueSlots,
					unassigned("middle-unassigned-1"),
					unassigned("middle-unassigned-2"),
					widthSlot,
					speedSlot,
				];
	}
	if (lane.mode === "random") {
		const group = dynamic.random_groups.find(
			(candidate) => candidate.id === lane.random_group_id,
		);
		const range = group ? editableRandomRange(group, domain.key) : {};
		return [
			{
				...sourceSlot("random-low", "Low", range.low, (item) => item),
				disabled: true,
			},
			{
				...sourceSlot("random-high", "High", range.high, (item) => item),
				disabled: true,
			},
			unassigned("random-unassigned-1"),
			unassigned("random-unassigned-2"),
			widthSlot,
			speedSlot,
		];
	}
	const valueSlots: DynamicEncoderSlot[] = [
		sourceSlot("maximum", "Top", lane.max_min.maximum, (item, maximum) => ({
			...item,
			max_min: { ...item.max_min, maximum },
		})),
		sourceSlot("minimum", "Bottom", lane.max_min.minimum, (item, minimum) => ({
			...item,
			max_min: { ...item.max_min, minimum },
		})),
	];
	return lane.max_min.function === "pwm"
		? [...valueSlots, ...pwmEncoderSlots(lane, onLaneChange), speedSlot]
		: [
				...valueSlots,
				unassigned("bounds-unassigned-1"),
				unassigned("bounds-unassigned-2"),
				widthSlot,
				speedSlot,
			];
}

function middleAmplitudePwmValueSlots(
	lane: ScalarDynamicLane,
	domain: DynamicLaneDomain,
	onLaneChange: (
		update: (lane: ScalarDynamicLane) => ScalarDynamicLane,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	const { minimum, maximum } = domain;
	const middle = scalarSourceEncoderValue(lane.middle_amplitude.middle, domain);
	const top = clamp(middle + lane.middle_amplitude.amplitude, minimum, maximum);
	const bottom = clamp(
		middle - lane.middle_amplitude.amplitude,
		minimum,
		maximum,
	);
	const slot = (
		field: "top" | "bottom",
		value: number,
	): DynamicEncoderSlot => ({
		id: `pwm-${field}`,
		label: field === "top" ? "Top" : "Bottom",
		display: formatLaneValue(domain, value),
		value,
		minimum,
		maximum,
		inputScale: domain.inputScale,
		fineStep: domain.fineStep,
		coarseStep: domain.coarseStep,
		apply: (nextValue, group) =>
			onLaneChange((item) => {
				const currentMiddle = scalarSourceEncoderValue(
					item.middle_amplitude.middle,
					domain,
				);
				const currentTop = clamp(
					currentMiddle + item.middle_amplitude.amplitude,
					minimum,
					maximum,
				);
				const currentBottom = clamp(
					currentMiddle - item.middle_amplitude.amplitude,
					minimum,
					maximum,
				);
				const nextTop =
					field === "top"
						? clamp(nextValue, currentBottom, maximum)
						: currentTop;
				const nextBottom =
					field === "bottom"
						? clamp(nextValue, minimum, currentTop)
						: currentBottom;
				return normalizePwmLane({
					...item,
					middle_amplitude: {
						...item.middle_amplitude,
						middle: {
							type: "value",
							value: (nextTop + nextBottom) / 2,
						},
						amplitude: (nextTop - nextBottom) / 2,
					},
				});
			}, group),
	});
	return [slot("top", top), slot("bottom", bottom)];
}

export function normalizePwmLane(lane: ScalarDynamicLane) {
	const functionName =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.function
			: lane.mode === "max_min"
				? lane.max_min.function
				: null;
	if (functionName !== "pwm") return lane;
	const pwm =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm;
	const on = clamp(pwm.on, 0, 1);
	const normalizedPwm = {
		...pwm,
		attack: clamp(pwm.attack, 0, on),
		on,
		decay: clamp(pwm.decay, 0, 1 - on),
		off: 1 - on,
	};
	return lane.mode === "middle_amplitude"
		? {
				...lane,
				width: 1,
				middle_amplitude: {
					...lane.middle_amplitude,
					pwm: normalizedPwm,
				},
			}
		: {
				...lane,
				width: 1,
				max_min: { ...lane.max_min, pwm: normalizedPwm },
			};
}

function pwmEncoderSlots(
	lane: ScalarDynamicLane,
	onLaneChange: (
		update: (lane: ScalarDynamicLane) => ScalarDynamicLane,
		mutationGroup?: string,
	) => Promise<void>,
): DynamicEncoderSlot[] {
	const pwm =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm;
	const slot = (
		field: "attack" | "on" | "decay",
		label: string,
	): DynamicEncoderSlot => ({
		id: `pwm-${field}`,
		label,
		display: `${Math.round(pwm[field] * 100)}%`,
		value: pwm[field],
		minimum: 0,
		maximum: 1,
		inputScale: 100,
		fineStep: 0.01,
		coarseStep: 0.1,
		apply: (value, group) =>
			onLaneChange((item) => setLanePwmValue(item, field, value), group),
	});
	return [slot("attack", "Attack"), slot("on", "On"), slot("decay", "Decay")];
}

function setLanePwmValue(
	lane: ScalarDynamicLane,
	field: "attack" | "on" | "decay",
	value: number,
): ScalarDynamicLane {
	const pwm =
		lane.mode === "middle_amplitude"
			? lane.middle_amplitude.pwm
			: lane.max_min.pwm;
	const nextPwm = { ...pwm };
	if (field === "attack") nextPwm.attack = clamp(value, 0, nextPwm.on);
	if (field === "on") {
		nextPwm.on = clamp(value, nextPwm.attack, 1 - nextPwm.decay);
		nextPwm.off = 1 - nextPwm.on;
	}
	if (field === "decay") nextPwm.decay = clamp(value, 0, nextPwm.off);
	return lane.mode === "middle_amplitude"
		? {
				...lane,
				width: 1,
				middle_amplitude: { ...lane.middle_amplitude, pwm: nextPwm },
			}
		: {
				...lane,
				width: 1,
				max_min: { ...lane.max_min, pwm: nextPwm },
			};
}

/** A source's encoder position; Current and Preset sit at the middle of the domain. */
function scalarSourceEncoderValue(
	source: DynamicScalarSourceProjection | undefined,
	domain: DynamicLaneDomain,
) {
	if (source?.type === "value") return source.value;
	return domain.address ? (domain.minimum + domain.maximum) / 2 : 0;
}

export function scalarSourceEncoderDisplay(
	source: DynamicScalarSourceProjection | undefined,
	domain: DynamicLaneDomain = laneDomainForKey("intensity"),
) {
	if (!source) return "—";
	if (source.type === "current") return "Current";
	if (source.type === "preset") return "Preset";
	return formatLaneValue(domain, source.value);
}

function scalarSourcePresetChoices(
	presets: readonly PresetObject[],
	attribute: string,
	source: DynamicScalarSourceProjection,
): NonNullable<EncoderSectionItem["presets"]> {
	const available = [...presets]
		.filter((preset) => presetContainsAttribute(preset, attribute))
		.sort((left, right) => {
			const family = presetFamilyLabel(left).localeCompare(
				presetFamilyLabel(right),
			);
			return family || left.body.number - right.body.number;
		});
	const families = new Map<string, PresetObject[]>();
	for (const preset of available) {
		const family = presetFamilyLabel(preset);
		families.set(family, [...(families.get(family) ?? []), preset]);
	}
	return {
		selectedValue:
			source.type === "current"
				? "current"
				: source.type === "preset"
					? `preset:${source.preset_id}`
					: undefined,
		groups: [
			{
				label: "Source",
				options: [
					{
						value: "current",
						label: "Current",
						description: "Use the current value for this attribute.",
					},
				],
			},
			...[...families].map(([family, items]) => ({
				label: family,
				options: items.map((preset) => ({
					value: `preset:${preset.id}`,
					label: preset.body.name,
					description: `${family} ${preset.body.number}`,
				})),
			})),
		],
	};
}

function presetContainsAttribute(preset: PresetObject, attribute: string) {
	const fixtureValues = Object.values(preset.body.values);
	const groupValues = Object.values(preset.body.group_values ?? {});
	return [...fixtureValues, ...groupValues].some((values) =>
		Object.hasOwn(values, attribute),
	);
}

function presetFamilyLabel(preset: PresetObject) {
	const family = preset.body.family;
	return !family || family === "All" ? "Mixed" : family;
}

function scalarSourceFromPresetChoice(
	value: string,
	attribute: string,
): DynamicScalarSourceProjection {
	if (value === "current") return sourceCurrent;
	if (value.startsWith("preset:"))
		return {
			type: "preset",
			preset_id: value.slice("preset:".length),
			attribute,
			last_valid_by_target: [],
		};
	return sourceCurrent;
}

const curveFunctionOptions: Array<{
	value: DynamicPeriodicFunctionProjection | "random";
	label: string;
	description: string;
}> = [
	{
		value: "sinus",
		label: "Sinus",
		description:
			"Smooth wave · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "cosinus",
		label: "Cosinus",
		description:
			"Smooth wave starting at maximum · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "linear_up",
		label: "Linear +",
		description:
			"Steady rise · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "linear_down",
		label: "Linear −",
		description:
			"Steady fall · Top and Bottom or Middle and Amplitude, Curve width, Speed.",
	},
	{
		value: "pwm",
		label: "PWM",
		description:
			"Shaped pulse · Top and Bottom or Middle and Amplitude, Attack, On, Decay, Off.",
	},
	{
		value: "random",
		label: "Random",
		description:
			"Seeded gate values and timing · configuration comes from the linked Random group.",
	},
];

export function curveFunctionSelectionGroups() {
	return [
		{
			label: "Periodic functions",
			options: curveFunctionOptions.slice(0, 5).map((option) => ({
				...option,
				icon: <CurveFunctionIcon functionName={option.value} />,
			})),
		},
		{
			label: "Random function",
			options: curveFunctionOptions.slice(5).map((option) => ({
				...option,
				icon: <CurveFunctionIcon functionName={option.value} />,
			})),
		},
	];
}

function CurveFunctionIcon({
	functionName,
}: {
	functionName: DynamicPeriodicFunctionProjection | "random";
}) {
	const path =
		functionName === "sinus"
			? "M1 12C4 3 8 3 12 12s8 9 11 0"
			: functionName === "cosinus"
				? "M1 4c5 0 6 16 11 16S18 4 23 4"
				: functionName === "linear_up"
					? "M2 21 22 3"
					: functionName === "linear_down"
						? "M2 3 22 21"
						: functionName === "pwm"
							? "M2 20V4h10v16h10"
							: "M2 16 6 7l4 10 4-12 4 11 4-7";
	return (
		<svg
			className="dynamic-function-icon"
			viewBox="0 0 24 24"
			aria-hidden="true"
		>
			<path d={path} />
		</svg>
	);
}
const interpolations = [
	"linear",
	"ease_in",
	"ease_out",
	"ease_in_out",
	"hold",
	"drop",
] as const;

export function curveEditorEncoderSlots(
	lane: DynamicLaneProjection | undefined,
	dynamic: DynamicDefinitionProjection,
	keyframeIndex: number,
	onKeyframeIndex: (index: number) => void,
	onLaneChange: (
		update: (lane: DynamicLaneProjection) => DynamicLaneProjection,
		mutationGroup?: string,
	) => Promise<void>,
	presets: readonly PresetObject[],
): DynamicEncoderSlot[] {
	const editable = lane ? editableLane(lane) : undefined;
	const domain = lane ? dynamicLaneDomain(lane) : laneDomainForKey("intensity");
	if (lane && (!editable || !domain))
		return [
			...Array.from(
				{ length: 4 },
				(_, index): DynamicEncoderSlot => ({
					id: `intent-value-${index}`,
					label: "Unassigned",
					display: "—",
					value: 0,
					minimum: 0,
					maximum: 1,
					inputScale: 1,
					fineStep: 0.01,
					coarseStep: 0.1,
					disabled: true,
					apply: async () => undefined,
				}),
			),
			{
				id: "curve-width",
				label: "Curve width",
				display: `${Math.round(lane.width * 100)}%`,
				value: lane.width,
				minimum: 0.05,
				maximum: 1,
				inputScale: 100,
				fineStep: 0.01,
				coarseStep: 0.1,
				apply: (value, group) =>
					onLaneChange(
						(current) => ({ ...current, width: clamp(value, 0.05, 1) }),
						group,
					),
			},
			{
				id: "lane-speed",
				label: "Speed",
				display: `${lane.speed_multiplier.numerator}/${lane.speed_multiplier.denominator}`,
				value: rationalValue(lane.speed_multiplier),
				minimum: 0.0625,
				maximum: 16,
				inputScale: 1,
				fineStep: 0.0625,
				coarseStep: 0.5,
				apply: (value, group) =>
					onLaneChange(
						(current) => ({
							...current,
							speed_multiplier: rationalFromNumber(value),
						}),
						group,
					),
			},
		];
	return scalarCurveEditorEncoderSlots(
		editable ?? undefined,
		domain ?? laneDomainForKey("intensity"),
		dynamic,
		keyframeIndex,
		onKeyframeIndex,
		(update, group) =>
			onLaneChange((current) => {
				const view = editableLane(current);
				return view ? commitEditableLane(update(view), current) : current;
			}, group),
		presets,
	);
}
