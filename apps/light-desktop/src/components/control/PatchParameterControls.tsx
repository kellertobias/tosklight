import { useEffect, useRef, useState } from "react";
import type { MultiPatchInstance, PatchedFixture } from "../../api/types";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";
import {
	type LocationEncoderSlot,
	PatchParameterEncoderSurfaces,
	type VisualizationEncoderSlot,
} from "./PatchParameterEncoderSurfaces";

type PatchEncoderGroup = "location" | "visualization";

const locationSlots: readonly LocationEncoderSlot[] = [
	{
		kind: "location",
		axis: "x",
		label: "Location X",
		unit: "m",
		fineStep: 0.001,
		coarseStep: 0.01,
	},
	{
		kind: "location",
		axis: "y",
		label: "Location Y",
		unit: "m",
		fineStep: 0.001,
		coarseStep: 0.01,
	},
	{
		kind: "location",
		axis: "z",
		label: "Location Z",
		unit: "m",
		fineStep: 0.001,
		coarseStep: 0.01,
	},
	{
		kind: "rotation",
		axis: "x",
		label: "Rotation X",
		unit: "°",
		fineStep: 1,
		coarseStep: 10,
	},
	{
		kind: "rotation",
		axis: "y",
		label: "Rotation Y",
		unit: "°",
		fineStep: 1,
		coarseStep: 10,
	},
	{
		kind: "rotation",
		axis: "z",
		label: "Rotation Z",
		unit: "°",
		fineStep: 1,
		coarseStep: 10,
	},
];

const visualizationSlots: readonly VisualizationEncoderSlot[] = [
	{ label: "Bracket", kind: "bracket" },
	{ label: "Shaper 1 Angle", kind: "shaper", element: 1 },
	{ label: "Shaper 2 Angle", kind: "shaper", element: 2 },
	{ label: "Shaper 3 Angle", kind: "shaper", element: 3 },
	{ label: "Shaper 4 Angle", kind: "shaper", element: 4 },
	{ label: "Shaper Module Rotation", kind: "module" },
];

export function PatchParameterControls({
	hardwareConnected = false,
}: {
	hardwareConnected?: boolean;
}) {
	const patch = usePatch();
	usePatchView();
	const [group, setGroup] = useState<PatchEncoderGroup>("location");
	const accumulated = useRef(
		new Map<number, { targetKey: string; observed: number; value: number }>(),
	);
	const target = selectedPhysicalTarget(
		patch.fixtures,
		patch.selectedPatchInstance,
	);
	const targetKey = target
		? `${target.fixture.fixture_id}:${target.multipatch?.id ?? "primary"}`
		: "none";
	useEffect(() => accumulated.current.clear(), [targetKey, group]);
	const visualizationCapabilities = target
		? physicalVisualizationCapabilities(target.fixture)
		: null;

	const storedValue = (slot: LocationEncoderSlot) => {
		const stored = target?.instance[slot.kind]?.[slot.axis] ?? 0;
		return slot.kind === "location" ? stored / 1_000 : stored;
	};
	const setValue = (slot: LocationEncoderSlot, value: number) => {
		if (!target || !Number.isFinite(value)) return;
		const stored = slot.kind === "location" ? Math.round(value * 1_000) : value;
		void patch.updateFixtureIntent(
			target.fixture.fixture_id,
			target.multipatch?.id ?? null,
			slot.kind === "location"
				? {
						type: "set_location_axis",
						axis: slot.axis,
						millimetres: stored,
					}
				: {
						type: "set_rotation_axis",
						axis: slot.axis,
						degrees: stored,
					},
		);
	};
	const applyAbsolute = (slotIndex: number, value: number) => {
		const slot = locationSlots[slotIndex];
		if (!slot) return;
		accumulated.current.delete(slotIndex);
		setValue(slot, value);
	};
	const applyStep = (slotIndex: number, delta: number) => {
		const slot = locationSlots[slotIndex];
		if (!slot || !target) return;
		const observed = storedValue(slot);
		const previous = accumulated.current.get(slotIndex);
		const start =
			previous &&
			previous.targetKey === targetKey &&
			(previous.observed === observed || previous.value === observed)
				? previous.value
				: observed;
		const value = start + delta;
		accumulated.current.set(slotIndex, { targetKey, observed, value });
		setValue(slot, value);
	};
	const visualizationStoredValue = (slot: VisualizationEncoderSlot) =>
		installedVisualizationValue(target, slot);
	const visualizationAvailable = (slot: VisualizationEncoderSlot) =>
		isVisualizationSlotAvailable(visualizationCapabilities, slot);
	const setVisualizationValue = (
		slot: VisualizationEncoderSlot,
		value: number,
	) => {
		if (!target || !visualizationAvailable(slot) || !Number.isFinite(value))
			return;
		const degrees = normalizeInstalledAngle(value);
		void patch.updateFixtureIntent(
			target.fixture.fixture_id,
			target.multipatch?.id ?? null,
			slot.kind === "bracket"
				? { type: "set_bracket_angle", degrees }
				: slot.kind === "module"
					? { type: "set_shaper_module_rotation", degrees }
					: {
							type: "set_static_shaper_angle",
							element: slot.element ?? 1,
							degrees,
						},
		);
	};
	const applyVisualizationAbsolute = (slotIndex: number, value: number) => {
		const slot = visualizationSlots[slotIndex];
		if (!slot) return;
		accumulated.current.delete(slotIndex);
		setVisualizationValue(slot, value);
	};
	const applyVisualizationStep = (slotIndex: number, delta: number) => {
		const slot = visualizationSlots[slotIndex];
		if (!slot || !target || !visualizationAvailable(slot)) return;
		const observed = visualizationStoredValue(slot);
		const previous = accumulated.current.get(slotIndex);
		const start =
			previous &&
			previous.targetKey === targetKey &&
			(previous.observed === observed || previous.value === observed)
				? previous.value
				: observed;
		const value = normalizeInstalledAngle(start + delta);
		accumulated.current.set(slotIndex, { targetKey, observed, value });
		setVisualizationValue(slot, value);
	};

	useHardwarePatchEncoders({
		enabled: hardwareConnected && Boolean(target),
		group,
		onLocationStep: applyStep,
		onVisualizationStep: applyVisualizationStep,
	});

	const label = patchTargetLabel(patch.status, target);
	const disabled = patch.status !== "ready" || !target;
	return (
		<div className="parameter-controls patch-parameter-controls">
			<PatchParameterEncoderSurfaces
				group={group}
				onGroupChange={setGroup}
				label={label}
				hardwareConnected={hardwareConnected}
				disabled={disabled}
				locationSlots={locationSlots}
				locationValue={storedValue}
				onLocationStep={applyStep}
				onLocationSet={applyAbsolute}
				visualizationSlots={visualizationSlots}
				visualizationValue={visualizationStoredValue}
				visualizationAvailable={visualizationAvailable}
				onVisualizationStep={applyVisualizationStep}
				onVisualizationSet={applyVisualizationAbsolute}
			/>
		</div>
	);
}

function useHardwarePatchEncoders({
	enabled,
	group,
	onLocationStep,
	onVisualizationStep,
}: {
	enabled: boolean;
	group: PatchEncoderGroup;
	onLocationStep: (slotIndex: number, delta: number) => void;
	onVisualizationStep: (slotIndex: number, delta: number) => void;
}) {
	useEffect(() => {
		if (!enabled) return;
		const handleEncoder = (event: Event) => {
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const slotIndex = Number(control.split("/")[1]) - 1;
			if (group === "location") {
				const slot = locationSlots[slotIndex];
				if (!slot) return;
				if (value === "up") onLocationStep(slotIndex, slot.fineStep);
				if (value === "down") onLocationStep(slotIndex, -slot.fineStep);
				if (value === "right") onLocationStep(slotIndex, slot.coarseStep);
				if (value === "left") onLocationStep(slotIndex, -slot.coarseStep);
				return;
			}
			if (!visualizationSlots[slotIndex]) return;
			if (value === "up") onVisualizationStep(slotIndex, 1);
			if (value === "down") onVisualizationStep(slotIndex, -1);
			if (value === "right") onVisualizationStep(slotIndex, 10);
			if (value === "left") onVisualizationStep(slotIndex, -10);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	});
}

function patchTargetLabel(
	status: string,
	target: ReturnType<typeof selectedPhysicalTarget>,
) {
	if (status !== "ready") return "Patch loading…";
	if (!target) return "Select a physical patch row";
	return (
		target.multipatch?.name ||
		target.fixture.name ||
		target.fixture.definition.name
	);
}

function normalizeInstalledAngle(value: number) {
	return ((((value + 180) % 360) + 360) % 360) - 180;
}

function installedVisualizationValue(
	target: ReturnType<typeof selectedPhysicalTarget>,
	slot: VisualizationEncoderSlot,
) {
	if (!target) return 0;
	if (slot.kind === "bracket") return target.instance.bracket_angle ?? 0;
	if (slot.kind === "module") return target.instance.shaper_angle ?? 0;
	return (
		target.instance.installed_appearance?.shaper_angles_degrees[
			(slot.element ?? 1) - 1
		] ?? 0
	);
}

function isVisualizationSlotAvailable(
	capabilities: ReturnType<typeof physicalVisualizationCapabilities> | null,
	slot: VisualizationEncoderSlot,
) {
	if (!capabilities) return false;
	if (slot.kind === "bracket") return capabilities.bracket;
	if (slot.kind === "module")
		return capabilities.module && !capabilities.liveModule;
	const index = (slot.element ?? 1) - 1;
	return capabilities.shapers[index] && !capabilities.liveShaperAngles[index];
}

function physicalVisualizationCapabilities(fixture: PatchedFixture) {
	const profile = fixture.definition.profile_snapshot;
	const mode =
		profile?.modes.find(
			(candidate) => candidate.id === fixture.definition.mode_id,
		) ?? profile?.modes[0];
	const attributes = (mode?.channels ?? []).flatMap((channel) => [
		channel.attribute.toLowerCase(),
		...(channel.functions ?? []).map((fn) => fn.attribute.toLowerCase()),
	]);
	const has = (attribute: string) => attributes.includes(attribute);
	const shapers = ([1, 2, 3, 4] as const).map(
		(element) =>
			has(`shaper.blade.${element}.position`) ||
			has(`shaper.blade.${element}.angle`),
	) as [boolean, boolean, boolean, boolean];
	const liveShaperAngles = ([1, 2, 3, 4] as const).map((element) =>
		has(`shaper.blade.${element}.angle`),
	) as [boolean, boolean, boolean, boolean];
	return {
		bracket: (mode?.geometry.emitters.length ?? 0) > 0,
		shapers,
		liveShaperAngles,
		module: shapers.some(Boolean),
		liveModule: has("shaper.rotation"),
	};
}

function selectedPhysicalTarget(
	fixtures: readonly PatchedFixture[],
	selection: {
		fixtureId: string;
		multipatchInstanceId: string | null;
	} | null,
) {
	if (!selection) return null;
	const fixture = fixtures.find(
		(candidate) => candidate.fixture_id === selection.fixtureId,
	);
	if (!fixture) return null;
	if (!selection.multipatchInstanceId)
		return {
			fixture,
			instance: fixture,
			multipatch: null as MultiPatchInstance | null,
		};
	const multipatch = fixture.multipatch?.find(
		(instance) => instance.id === selection.multipatchInstanceId,
	);
	return multipatch ? { fixture, instance: multipatch, multipatch } : null;
}
