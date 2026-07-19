import { HardwareEncoderDisplay } from "../HardwareEncoderDisplay";
import { VerticalTouchFader } from "../VerticalTouchFader";
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
}: {
	controller: ParameterController;
	attribute: string | null;
	index: number;
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
	const hasScopedValue = controller.selectedGroupId
		? Boolean(
				controller.ownProgrammer?.group_values?.[
					controller.selectedGroupId
				]?.[attribute],
			)
		: controller.programmerValues.some(
				(entry) =>
					entry.attribute === attribute &&
					controller.selectedFixtureIds.includes(entry.fixture_id),
			);
	const label = parameterLabels[attribute] ?? attribute.replaceAll(".", " ");
	if (controller.hardwareConnected)
		return (
			<HardwareEncoderDisplay
				slot={index + 1}
				target={{ label, value: discrete ?? display }}
				editValue={discrete ? undefined : value * 100}
				onEdit={
					discrete
						? undefined
						: (next) =>
								void controller.applyParameter(
									attribute,
									Math.max(0, Math.min(100, next)) / 100,
								)
				}
				onEditRange={
					discrete
						? undefined
						: (points) => void controller.applyParameterRange(attribute, points)
				}
				onRelease={
					hasScopedValue
						? () => void controller.releaseParameter(attribute)
						: undefined
				}
			/>
		);
	return (
		<VerticalTouchFader
			label={`Enc ${index + 1} · ${label}`}
			value={value * 100}
			display={formatNormalizedValue(value)}
			accentColor={attributeColor(attribute)}
			mode={controller.dynamicsMode ? "Dynamics" : undefined}
			directInput
			actions={
				hasScopedValue
					? [
							{
								id: "release",
								label: "Release",
								"aria-label": `Release ${parameterLabels[attribute] ?? attribute}`,
								onClick: () => void controller.releaseParameter(attribute),
							},
						]
					: []
			}
			onChange={(next) => void controller.applyParameter(attribute, next / 100)}
		/>
	);
}

export function EncoderSurfaces({
	controller,
}: {
	controller: ParameterController;
}) {
	if (controller.directMode && controller.hardwareConnected)
		return (
			<>
				{Array.from({ length: 6 }, (_, index) => (
					<HardwareEncoderDisplay key={index} slot={index + 1} />
				))}
			</>
		);
	if (
		!controller.selectedFixtureIds.length &&
		!controller.selectedGroupId
	)
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
				/>
			))}
		</>
	);
}
