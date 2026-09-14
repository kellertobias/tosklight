import { NumberField, SelectField, TextField } from "@tosklight/ui";
import type {
	AngularMotion,
	ChannelFunctionBehavior,
	FixtureChannel,
} from "../wire";
import { maxRaw } from "../sheet/fixtureProfileModel";

type ChannelFunction = FixtureChannel["functions"][number];

function optionalNumber(value: string): number | null {
	return value.trim() === "" ? null : Number(value);
}

export function FunctionBehaviorEditor({
	behavior,
	modeChannel,
	actionIds,
	onChange,
}: {
	behavior: ChannelFunctionBehavior;
	modeChannel: FixtureChannel;
	actionIds: Array<{ id: string; name: string }>;
	onChange: (behavior: ChannelFunctionBehavior) => void;
}) {
	if (behavior.type === "continuous")
		return (
			<>
				<NumberField
					label="Function physical minimum"
					allowDecimal
					value={behavior.physical_min}
					onChange={(event) =>
						onChange({ ...behavior, physical_min: Number(event.target.value) })
					}
				/>
				<NumberField
					label="Function physical maximum"
					allowDecimal
					value={behavior.physical_max}
					onChange={(event) =>
						onChange({ ...behavior, physical_max: Number(event.target.value) })
					}
				/>
				<TextField
					label="Function unit"
					value={behavior.unit ?? ""}
					onChange={(event) =>
						onChange({ ...behavior, unit: event.target.value || null })
					}
				/>
			</>
		);
	if (behavior.type === "control")
		return (
			<SelectField
				label="Control action"
				value={behavior.action_id}
				options={[
					{ value: "", label: "Choose action" },
					...actionIds.map((action) => ({
						value: action.id,
						label: action.name,
					})),
				]}
				onChange={(action_id) => onChange({ ...behavior, action_id })}
			/>
		);
	return (
		<>
			<TextField
				label="Portable semantic ID"
				value={behavior.semantic_id}
				onChange={(event) =>
					onChange({ ...behavior, semantic_id: event.target.value })
				}
			/>
			<TextField
				label="Fixture label"
				value={behavior.label}
				onChange={(event) =>
					onChange({ ...behavior, label: event.target.value })
				}
			/>
			<NumberField
				label="Exact raw value"
				min={0}
				max={maxRaw(modeChannel.resolution)}
				value={behavior.raw_value}
				onChange={(event) =>
					onChange({ ...behavior, raw_value: Number(event.target.value) })
				}
			/>
		</>
	);
}

export function AngularMotionEditor({
	functionValue,
	onChange,
}: {
	functionValue: ChannelFunction;
	onChange: (fn: ChannelFunction) => void;
}) {
	if (
		functionValue.behavior.type !== "continuous" &&
		functionValue.behavior.type !== "indexed"
	)
		return null;
	const indexedWheelSlot = functionValue.behavior.type === "indexed";
	const motion = functionValue.angular_motion ?? null;
	const setMotionValue = <K extends keyof AngularMotion>(
		key: K,
		value: AngularMotion[K],
	) =>
		motion &&
		onChange({ ...functionValue, angular_motion: { ...motion, [key]: value } });
	return (
		<>
			<SelectField
				label="Angular motion"
				value={motion?.kind ?? ""}
				options={[
					{ value: "", label: "Not angular motion" },
					{ value: "absolute_position", label: "Absolute angular position" },
					...(indexedWheelSlot
						? []
						: [
								{
									value: "angular_velocity",
									label: "Signed angular velocity",
								},
							]),
				]}
				onChange={(kind) =>
					onChange({
						...functionValue,
						angular_motion: kind
							? {
									kind: kind as AngularMotion["kind"],
									max_speed_degrees_per_second: null,
									acceleration_degrees_per_second_squared: null,
									deceleration_degrees_per_second_squared: null,
								}
							: null,
					})
				}
			/>
			{motion && (
				<>
					<NumberField
						label="Maximum speed (degrees per second)"
						allowDecimal
						min={0}
						value={motion.max_speed_degrees_per_second ?? ""}
						onChange={(event) =>
							setMotionValue(
								"max_speed_degrees_per_second",
								optionalNumber(event.target.value),
							)
						}
					/>
					<NumberField
						label="Acceleration (degrees per second squared)"
						allowDecimal
						min={0}
						value={motion.acceleration_degrees_per_second_squared ?? ""}
						onChange={(event) =>
							setMotionValue(
								"acceleration_degrees_per_second_squared",
								optionalNumber(event.target.value),
							)
						}
					/>
					<NumberField
						label="Deceleration (degrees per second squared)"
						allowDecimal
						min={0}
						value={motion.deceleration_degrees_per_second_squared ?? ""}
						onChange={(event) =>
							setMotionValue(
								"deceleration_degrees_per_second_squared",
								optionalNumber(event.target.value),
							)
						}
					/>
				</>
			)}
		</>
	);
}
