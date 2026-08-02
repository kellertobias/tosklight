import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { useEffect, useRef, useState } from "react";
import type { MultiPatchInstance, PatchedFixture } from "../../api/types";
import { usePatch, usePatchView } from "../../features/patch/PatchContext";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

type PatchEncoderGroup = "location" | "visualization";
type VectorKind = "location" | "rotation";
type VectorAxis = "x" | "y" | "z";

interface LocationEncoderSlot {
	kind: VectorKind;
	axis: VectorAxis;
	label: string;
	unit: "m" | "°";
	fineStep: number;
	coarseStep: number;
}

interface VisualizationEncoderSlot {
	label: string;
	kind: "bracket" | "shaper" | "module";
	element?: 1 | 2 | 3 | 4;
}

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
	const visualizationStoredValue = (slot: VisualizationEncoderSlot) => {
		if (!target) return 0;
		if (slot.kind === "bracket") return target.instance.bracket_angle ?? 0;
		if (slot.kind === "module") return target.instance.shaper_angle ?? 0;
		return (
			target.instance.installed_appearance?.shaper_angles_degrees[
				(slot.element ?? 1) - 1
			] ?? 0
		);
	};
	const visualizationAvailable = (slot: VisualizationEncoderSlot) => {
		if (!visualizationCapabilities) return false;
		if (slot.kind === "bracket") return visualizationCapabilities.bracket;
		if (slot.kind === "module")
			return (
				visualizationCapabilities.module &&
				!visualizationCapabilities.liveModule
			);
		const index = (slot.element ?? 1) - 1;
		return (
			visualizationCapabilities.shapers[index] &&
			!visualizationCapabilities.liveShaperAngles[index]
		);
	};
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

	useEffect(() => {
		if (!hardwareConnected || !target) return;
		const handleEncoder = (event: Event) => {
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const slotIndex = Number(control.split("/")[1]) - 1;
			if (group === "location") {
				const slot = locationSlots[slotIndex];
				if (!slot) return;
				if (value === "up") applyStep(slotIndex, slot.fineStep);
				if (value === "down") applyStep(slotIndex, -slot.fineStep);
				if (value === "right") applyStep(slotIndex, slot.coarseStep);
				if (value === "left") applyStep(slotIndex, -slot.coarseStep);
				return;
			}
			if (!visualizationSlots[slotIndex]) return;
			if (value === "up") applyVisualizationStep(slotIndex, 1);
			if (value === "down") applyVisualizationStep(slotIndex, -1);
			if (value === "right") applyVisualizationStep(slotIndex, 10);
			if (value === "left") applyVisualizationStep(slotIndex, -10);
		};
		window.addEventListener("light:encoder-action", handleEncoder);
		return () =>
			window.removeEventListener("light:encoder-action", handleEncoder);
	});

	const label =
		patch.status !== "ready"
			? "Patch loading…"
			: target
				? target.multipatch?.name ||
					target.fixture.name ||
					target.fixture.definition.name
				: "Select a physical patch row";
	const disabled = patch.status !== "ready" || !target;
	return (
		<div className="parameter-controls patch-parameter-controls">
			<div className="family-tabs">
				<Button
					active={group === "location"}
					onClick={() => setGroup("location")}
				>
					Location
				</Button>
				<Button
					active={group === "visualization"}
					onClick={() => setGroup("visualization")}
				>
					Visualization
				</Button>
				<span className="family-spacer" />
				<small>{label}</small>
			</div>
			<div className="parameter-surfaces">
				{group === "location"
					? locationSlots.map((slot, index) =>
							hardwareConnected ? (
								<HardwareEncoderDisplay
									key={slot.label}
									slot={index + 1}
									activateOnHardwarePress
									target={
										disabled
											? undefined
											: {
													label: slot.label,
													value: formatLocationEncoderValue(
														storedValue(slot),
														slot,
													),
													role: "Turn · Press-turn coarse",
												}
									}
									editValue={storedValue(slot)}
									onEdit={(value) => applyAbsolute(index, value)}
								/>
							) : (
								<TouchEncoder
									key={slot.label}
									label={`Enc ${index + 1} · ${slot.label}`}
									slot={index + 1}
									attributeLabel={slot.label}
									value={storedValue(slot)}
									display={formatLocationEncoderValue(storedValue(slot), slot)}
									minimum={Number.MIN_SAFE_INTEGER}
									maximum={Number.MAX_SAFE_INTEGER}
									inputScale={1}
									slowStep={slot.fineStep}
									fastStep={slot.coarseStep}
									disabled={disabled}
									onStep={(delta) => applyStep(index, delta)}
									onSet={(value) => applyAbsolute(index, value)}
								/>
							),
						)
					: visualizationSlots.map((slot, index) => {
							const available = !disabled && visualizationAvailable(slot);
							const value = visualizationStoredValue(slot);
							return hardwareConnected ? (
								<HardwareEncoderDisplay
									key={slot.label}
									slot={index + 1}
									activateOnHardwarePress
									target={
										available
											? {
													label: slot.label,
													value: formatVisualizationValue(value),
													role: "Turn · Press-turn coarse",
												}
											: undefined
									}
									editValue={value}
									onEdit={(next) => applyVisualizationAbsolute(index, next)}
								/>
							) : (
								<TouchEncoder
									key={slot.label}
									label={`Enc ${index + 1} · ${slot.label}`}
									slot={index + 1}
									attributeLabel={slot.label}
									value={value}
									display={
										available ? formatVisualizationValue(value) : "Unavailable"
									}
									minimum={-180}
									maximum={179.999}
									inputScale={1}
									slowStep={1}
									fastStep={10}
									disabled={!available}
									onStep={(delta) => applyVisualizationStep(index, delta)}
									onSet={(next) => applyVisualizationAbsolute(index, next)}
								/>
							);
						})}
			</div>
		</div>
	);
}

function formatLocationEncoderValue(value: number, slot: LocationEncoderSlot) {
	return slot.unit === "m"
		? `${value.toFixed(3)} m`
		: `${Number(value.toFixed(3))}°`;
}

function formatVisualizationValue(value: number) {
	return `${Number(value.toFixed(3))}°`;
}

function normalizeInstalledAngle(value: number) {
	return ((((value + 180) % 360) + 360) % 360) - 180;
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
