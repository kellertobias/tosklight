import { HardwareEncoderDisplay } from "../HardwareEncoderDisplay";
import { TouchEncoder } from "@tosklight/ui/encoders";
import {
	useProgrammingCommandLineActions,
	useProgrammingDeleteCommandActive,
} from "../../../features/programmingInteraction/ProgrammingInteractionView";
import { formatNormalizedValue, parameterLabels } from "./model";
import type { ParameterController } from "./useParameterController";

function attributeColor(attribute: string) {
	return (
		{
			"color.red": "#ff3d45",
			"color.green": "#35d568",
			"color.blue": "#378eff",
			"color.white": "#ffffff",
			"color.amber": "#ffb30f",
			"color.uv": "#9a55ff",
		} as Record<string, string>
	)[attribute];
}

function UnassignedEncoder({
	hardwareConnected,
	index,
}: {
	hardwareConnected: boolean;
	index: number;
}) {
	return hardwareConnected ? (
		<HardwareEncoderDisplay slot={index + 1} />
	) : (
		<div
			className="parameter-placeholder"
			role="img"
			aria-label={`Encoder ${index + 1} unassigned`}
		>
			<span>Enc {index + 1}</span>
			<small>Unassigned</small>
		</div>
	);
}

function EncoderSurface({
	controller,
	attribute,
	index,
	deleteArmed,
	resetCommandLine,
}: {
	controller: ParameterController;
	attribute: string | null;
	index: number;
	deleteArmed: boolean;
	resetCommandLine(): Promise<boolean>;
}) {
	if (!attribute)
		return (
			<UnassignedEncoder
				hardwareConnected={controller.hardwareConnected}
				index={index}
			/>
		);
	const value =
		controller.programmerTarget(attribute) ??
		controller.normalized.get(attribute) ??
		0;
	const discrete = controller.encoderDiscreteDisplay(attribute);
	const display =
		controller.encoderNormalizedDisplay(attribute) ??
		formatNormalizedValue(value);
	const hasScopedValue = controller.hasProgrammerValue(attribute);
	const label = parameterLabels[attribute] ?? attribute.replaceAll(".", " ");
	if (controller.hardwareConnected)
		return (
			<HardwareEncoderDisplay
				slot={index + 1}
				activateOnHardwarePress
				target={{ label, value: discrete ?? display }}
				editValue={discrete ? undefined : value * 100}
				canRelease={hasScopedValue}
				onEdit={
					discrete || !controller.canWriteValues
						? undefined
						: (next) =>
								void controller.applyParameter(
									attribute,
									Math.max(0, Math.min(100, next)) / 100,
								)
				}
				onEditRange={
					discrete || !controller.canWriteValues
						? undefined
						: (points) => void controller.applyParameterRange(attribute, points)
				}
				onRelease={
					hasScopedValue && controller.canWriteValues
						? () => controller.releaseParameter(attribute).then(() => undefined)
						: undefined
				}
				onHardwarePress={() => {
					if (!deleteArmed) return false;
					if (hasScopedValue && controller.canWriteValues) {
						void controller
							.releaseParameter(attribute)
							.then(() => resetCommandLine());
					}
					return true;
				}}
			/>
		);
	return (
		<TouchEncoder
			label={`Enc ${index + 1} · ${label}`}
			slot={index + 1}
			attributeLabel={label}
			value={value}
			display={discrete ?? display}
			accentColor={attributeColor(attribute)}
			mode={controller.dynamicsMode ? "Dynamics" : undefined}
			disabled={!controller.canWriteValues}
			indexed={Boolean(discrete)}
			canRelease={hasScopedValue}
			onStep={(delta, undoGroup) =>
				void controller.stepParameter(attribute, delta, undoGroup)
			}
			onSet={(next) => void controller.applyParameter(attribute, next)}
			onSetRange={
				discrete || !controller.canWriteValues
					? undefined
					: (points) => void controller.applyParameterRange(attribute, points)
			}
			onRelease={() => void controller.releaseParameter(attribute)}
		/>
	);
}

export function EncoderSurfaces({
	controller,
}: {
	controller: ParameterController;
}) {
	const deleteArmed = useProgrammingDeleteCommandActive(
		controller.hardwareConnected,
	);
	const commandLineActions = useProgrammingCommandLineActions();
	if (!controller.selectedFixtureIds.length && !controller.selectedGroupId)
		return (
			<div className="parameter-empty">
				<b>No fixtures selected</b>
				<small>Select fixtures to inspect or edit their real parameters.</small>
			</div>
		);
	return (
		<>
			{controller.encoderSlots.map((attribute, index) => (
				<EncoderSurface
					key={attribute ?? `empty-${index}`}
					controller={controller}
					attribute={attribute}
					index={index}
					deleteArmed={deleteArmed}
					resetCommandLine={() =>
						commandLineActions?.reset() ?? Promise.resolve(false)
					}
				/>
			))}
		</>
	);
}
