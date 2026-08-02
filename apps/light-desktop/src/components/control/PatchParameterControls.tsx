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
	useEffect(() => accumulated.current.clear(), [targetKey]);

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

	useEffect(() => {
		if (!hardwareConnected || group !== "location" || !target) return;
		const handleEncoder = (event: Event) => {
			const { control, value } = (
				event as CustomEvent<{ control: string; value?: string }>
			).detail;
			const slotIndex = Number(control.split("/")[1]) - 1;
			const slot = locationSlots[slotIndex];
			if (!slot) return;
			if (value === "up") applyStep(slotIndex, slot.fineStep);
			if (value === "down") applyStep(slotIndex, -slot.fineStep);
			if (value === "right") applyStep(slotIndex, slot.coarseStep);
			if (value === "left") applyStep(slotIndex, -slot.coarseStep);
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
					: Array.from({ length: 6 }, (_, index) =>
							hardwareConnected ? (
								<HardwareEncoderDisplay key={index} slot={index + 1} />
							) : (
								<div
									key={index}
									className="parameter-placeholder"
									role="img"
									aria-label={`Visualization encoder ${index + 1} unavailable`}
								>
									<span>Enc {index + 1}</span>
									<small>Unavailable</small>
								</div>
							),
						)}
			</div>
		</div>
	);
}

function formatLocationEncoderValue(value: number, slot: LocationEncoderSlot) {
	return slot.unit === "m"
		? `${value.toFixed(3)} m`
		: `${Number(value.toFixed(3))}°`;
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
