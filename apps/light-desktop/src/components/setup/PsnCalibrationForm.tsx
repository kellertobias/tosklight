import { FormLayout, NumberField, SelectField } from "@tosklight/ui";
import type { PsnConfiguration, PsnEdit } from "../../api/client/psn";

/**
 * Where the tracking system's stage is, in the show's stage.
 *
 * Usually nothing: both agree on metres and on which way is up. What differs is where the
 * tracking system was told its origin is, and which way round it was set up.
 */
export function PsnCalibrationForm({
	configuration,
	onEdit,
}: {
	configuration: PsnConfiguration;
	onEdit: (edit: PsnEdit) => void;
}) {
	const edit = onEdit;
	return (
		<>
	<h3>Calibration</h3>
	<FormLayout columns={3}>
		{(["x", "y", "z"] as const).map((axis, index) => (
			<NumberField
				key={`offset-${axis}-${configuration.calibration.offsetMetres[index]}`}
				label={`Origin ${axis} (m)`}
				defaultValue={configuration.calibration.offsetMetres[index]}
				onBlur={(event) => {
					const offsetMetres = [
						...configuration.calibration.offsetMetres,
					] as [number, number, number];
					offsetMetres[index] = Number(event.target.value);
					void edit({
						calibration: { ...configuration.calibration, offsetMetres },
					});
				}}
			/>
		))}
		<NumberField
			label="Rotation (°)"
			description="About the show's up axis, for a tracking system set up facing another way."
			key={`rotation-${configuration.calibration.rotationDegrees}`}
			defaultValue={configuration.calibration.rotationDegrees}
			onBlur={(event) =>
				void edit({
					calibration: {
						...configuration.calibration,
						rotationDegrees: Number(event.target.value),
					},
				})
			}
		/>
		<NumberField
			label="Scale"
			key={`scale-${configuration.calibration.scale}`}
			defaultValue={configuration.calibration.scale}
			onBlur={(event) =>
				void edit({
					calibration: {
						...configuration.calibration,
						scale: Number(event.target.value),
					},
				})
			}
		/>
		<SelectField
			label="Interface"
			value={configuration.interface ?? ""}
			options={[
				{ value: "", label: "Any network card" },
				...(configuration.interface
					? [
							{
								value: configuration.interface,
								label: configuration.interface,
							},
						]
					: []),
			]}
			onChange={(value) => void edit({ interface: value || null })}
		/>
	</FormLayout>
		</>
	);
}
