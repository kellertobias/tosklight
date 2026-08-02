import { Button } from "@tosklight/ui";
import { TouchEncoder } from "@tosklight/ui/encoders";
import { HardwareEncoderDisplay } from "./HardwareEncoderDisplay";

export interface LocationEncoderSlot {
	kind: "location" | "rotation";
	axis: "x" | "y" | "z";
	label: string;
	unit: "m" | "°";
	fineStep: number;
	coarseStep: number;
}

export interface VisualizationEncoderSlot {
	label: string;
	kind: "bracket" | "shaper" | "module";
	element?: 1 | 2 | 3 | 4;
}

interface PatchParameterEncoderSurfacesProps {
	group: "location" | "visualization";
	onGroupChange: (group: "location" | "visualization") => void;
	label: string;
	hardwareConnected: boolean;
	disabled: boolean;
	locationSlots: readonly LocationEncoderSlot[];
	locationValue: (slot: LocationEncoderSlot) => number;
	onLocationStep: (slotIndex: number, delta: number) => void;
	onLocationSet: (slotIndex: number, value: number) => void;
	visualizationSlots: readonly VisualizationEncoderSlot[];
	visualizationValue: (slot: VisualizationEncoderSlot) => number;
	visualizationAvailable: (slot: VisualizationEncoderSlot) => boolean;
	onVisualizationStep: (slotIndex: number, delta: number) => void;
	onVisualizationSet: (slotIndex: number, value: number) => void;
}

export function PatchParameterEncoderSurfaces({
	group,
	onGroupChange,
	label,
	hardwareConnected,
	disabled,
	locationSlots,
	locationValue,
	onLocationStep,
	onLocationSet,
	visualizationSlots,
	visualizationValue,
	visualizationAvailable,
	onVisualizationStep,
	onVisualizationSet,
}: PatchParameterEncoderSurfacesProps) {
	return (
		<>
			<div className="family-tabs">
				<Button
					active={group === "location"}
					onClick={() => onGroupChange("location")}
				>
					Location
				</Button>
				<Button
					active={group === "visualization"}
					onClick={() => onGroupChange("visualization")}
				>
					Visualization
				</Button>
				<span className="family-spacer" />
				<small>{label}</small>
			</div>
			<div className="parameter-surfaces">
				{group === "location" ? (
					<LocationSurfaces
						hardwareConnected={hardwareConnected}
						disabled={disabled}
						slots={locationSlots}
						value={locationValue}
						onStep={onLocationStep}
						onSet={onLocationSet}
					/>
				) : (
					<VisualizationSurfaces
						hardwareConnected={hardwareConnected}
						disabled={disabled}
						slots={visualizationSlots}
						value={visualizationValue}
						available={visualizationAvailable}
						onStep={onVisualizationStep}
						onSet={onVisualizationSet}
					/>
				)}
			</div>
		</>
	);
}

interface LocationSurfacesProps {
	hardwareConnected: boolean;
	disabled: boolean;
	slots: readonly LocationEncoderSlot[];
	value: (slot: LocationEncoderSlot) => number;
	onStep: (slotIndex: number, delta: number) => void;
	onSet: (slotIndex: number, value: number) => void;
}

function LocationSurfaces({
	hardwareConnected,
	disabled,
	slots,
	value,
	onStep,
	onSet,
}: LocationSurfacesProps) {
	return slots.map((slot, index) => {
		const currentValue = value(slot);
		return hardwareConnected ? (
			<HardwareEncoderDisplay
				key={slot.label}
				slot={index + 1}
				activateOnHardwarePress
				target={
					disabled
						? undefined
						: {
								label: slot.label,
								value: formatLocationEncoderValue(currentValue, slot),
								role: "Turn · Press-turn coarse",
							}
				}
				editValue={currentValue}
				onEdit={(next) => onSet(index, next)}
			/>
		) : (
			<TouchEncoder
				key={slot.label}
				label={`Enc ${index + 1} · ${slot.label}`}
				slot={index + 1}
				attributeLabel={slot.label}
				value={currentValue}
				display={formatLocationEncoderValue(currentValue, slot)}
				minimum={Number.MIN_SAFE_INTEGER}
				maximum={Number.MAX_SAFE_INTEGER}
				inputScale={1}
				slowStep={slot.fineStep}
				fastStep={slot.coarseStep}
				disabled={disabled}
				onStep={(delta) => onStep(index, delta)}
				onSet={(next) => onSet(index, next)}
			/>
		);
	});
}

interface VisualizationSurfacesProps {
	hardwareConnected: boolean;
	disabled: boolean;
	slots: readonly VisualizationEncoderSlot[];
	value: (slot: VisualizationEncoderSlot) => number;
	available: (slot: VisualizationEncoderSlot) => boolean;
	onStep: (slotIndex: number, delta: number) => void;
	onSet: (slotIndex: number, value: number) => void;
}

function VisualizationSurfaces({
	hardwareConnected,
	disabled,
	slots,
	value,
	available,
	onStep,
	onSet,
}: VisualizationSurfacesProps) {
	return slots.map((slot, index) => {
		const isAvailable = !disabled && available(slot);
		const currentValue = value(slot);
		return hardwareConnected ? (
			<HardwareEncoderDisplay
				key={slot.label}
				slot={index + 1}
				activateOnHardwarePress
				target={
					isAvailable
						? {
								label: slot.label,
								value: formatVisualizationValue(currentValue),
								role: "Turn · Press-turn coarse",
							}
						: undefined
				}
				editValue={currentValue}
				onEdit={(next) => onSet(index, next)}
			/>
		) : (
			<TouchEncoder
				key={slot.label}
				label={`Enc ${index + 1} · ${slot.label}`}
				slot={index + 1}
				attributeLabel={slot.label}
				value={currentValue}
				display={
					isAvailable ? formatVisualizationValue(currentValue) : "Unavailable"
				}
				minimum={-180}
				maximum={179.999}
				inputScale={1}
				slowStep={1}
				fastStep={10}
				disabled={!isAvailable}
				onStep={(delta) => onStep(index, delta)}
				onSet={(next) => onSet(index, next)}
			/>
		);
	});
}

function formatLocationEncoderValue(value: number, slot: LocationEncoderSlot) {
	return slot.unit === "m"
		? `${value.toFixed(3)} m`
		: `${Number(value.toFixed(3))}°`;
}

function formatVisualizationValue(value: number) {
	return `${Number(value.toFixed(3))}°`;
}
