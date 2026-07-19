import type {
	ColorSystem,
	FixtureMode,
	HeadColorSystem,
} from "../../../api/types";
import { FormLayout, NumberField, SelectField } from "../../common";
import { reconcileColorSystemHighlightDefaults } from "../fixtureProfileModel";
import {
	AdditiveColorEditor,
	DiscreteColorEditor,
	SubtractiveColorEditor,
} from "./colorFields";

export function removeColorChannel(
	system: HeadColorSystem,
	channelId: string,
): HeadColorSystem | null {
	if (system.system.type === "additive")
		return {
			...system,
			system: {
				...system.system,
				emitters: system.system.emitters.filter(
					(emitter) => emitter.channel_id !== channelId,
				),
			},
		};
	if (
		system.system.type === "subtractive" &&
		[
			system.system.cyan_channel_id,
			system.system.magenta_channel_id,
			system.system.yellow_channel_id,
		].includes(channelId)
	)
		return null;
	if (
		system.system.type === "discrete_wheel" &&
		system.system.channel_id === channelId
	)
		return null;
	return system;
}

const identityColorCorrectionMatrix =
	(): HeadColorSystem["correction_matrix"] => [
		[1, 0, 0],
		[0, 1, 0],
		[0, 0, 1],
	];

export function replaceHeadColorSystem(
	systems: HeadColorSystem[],
	headId: string,
	system: ColorSystem | null,
): HeadColorSystem[] {
	if (!system)
		return systems.filter((candidate) => candidate.head_id !== headId);
	const existing = systems.find((candidate) => candidate.head_id === headId);
	return [
		...systems.filter((candidate) => candidate.head_id !== headId),
		existing
			? { ...existing, system }
			: {
					head_id: headId,
					correction_matrix: identityColorCorrectionMatrix(),
					system,
				},
	];
}

export function ColorEditor({
	mode,
	onChange,
}: {
	mode: FixtureMode;
	onChange: (mode: FixtureMode) => void;
}) {
	const setSystem = (headId: string, system: ColorSystem | null) =>
		onChange(
			reconcileColorSystemHighlightDefaults(
				mode,
				replaceHeadColorSystem(mode.color_systems, headId, system),
			),
		);
	const setCorrection = (
		headId: string,
		row: number,
		column: number,
		value: number,
	) =>
		onChange(
			reconcileColorSystemHighlightDefaults(
				mode,
				mode.color_systems.map((candidate) =>
					candidate.head_id === headId
						? {
								...candidate,
								correction_matrix: candidate.correction_matrix.map(
									(values, rowIndex) =>
										values.map((entry, columnIndex) =>
											rowIndex === row && columnIndex === column
												? value
												: entry,
										),
								) as HeadColorSystem["correction_matrix"],
							}
						: candidate,
				),
			),
		);
	return (
		<div className="fixture-color-editor">
			<p>
				Abstract XYZ color is resolved through one color system per logical
				head. Direct emitter channels remain available to the programmer.
			</p>
			{mode.heads.map((head) => {
				const record = mode.color_systems.find(
					(candidate) => candidate.head_id === head.id,
				);
				const channels = mode.channels.filter(
					(channel) => channel.head_id === head.id,
				);
				const options = channels.map((channel) => ({
					value: channel.id,
					label: channel.attribute,
				}));
				const type = record?.system.type ?? "none";
				return (
					<section key={head.id}>
						<header>
							<h3>{head.name}</h3>
							<SelectField
								label="Color system"
								value={type}
								options={[
									{ value: "none", label: "No abstraction" },
									{ value: "additive", label: "Additive emitters" },
									{ value: "subtractive", label: "Subtractive CMY" },
									{ value: "discrete_wheel", label: "Discrete color wheel" },
								]}
								onChange={(next) => {
									if (next === "none") return setSystem(head.id, null);
									const first = channels[0]?.id ?? "";
									if (next === "additive")
										return setSystem(head.id, { type: next, emitters: [] });
									if (next === "subtractive")
										return setSystem(head.id, {
											type: next,
											cyan_channel_id: first,
											magenta_channel_id: first,
											yellow_channel_id: first,
										});
									setSystem(head.id, {
										type: next,
										channel_id: first,
										slots: [],
									});
								}}
							/>
						</header>
						{record && (
							<fieldset className="color-correction-matrix">
								<legend>XYZ correction matrix</legend>
								<p>
									Applied before calibrated color matching. Identity leaves
									requested XYZ unchanged.
								</p>
								<FormLayout columns={3}>
									{record.correction_matrix.flatMap((row, rowIndex) =>
										row.map((value, columnIndex) => (
											<NumberField
												key={`${rowIndex}-${columnIndex}`}
												aria-label={`${head.name} correction row ${rowIndex + 1} column ${columnIndex + 1}`}
												allowDecimal
												step={0.001}
												value={value}
												onChange={(event) =>
													setCorrection(
														head.id,
														rowIndex,
														columnIndex,
														Number(event.target.value),
													)
												}
											/>
										)),
									)}
								</FormLayout>
							</fieldset>
						)}
						{record?.system.type === "additive" && (
							<AdditiveColorEditor
								system={record.system}
								channels={channels}
								options={options}
								onChange={(system) => setSystem(head.id, system)}
							/>
						)}
						{record?.system.type === "subtractive" && (
							<SubtractiveColorEditor
								system={record.system}
								options={options}
								onChange={(system) => setSystem(head.id, system)}
							/>
						)}
						{record?.system.type === "discrete_wheel" && (
							<DiscreteColorEditor
								system={record.system}
								options={options}
								onChange={(system) => setSystem(head.id, system)}
							/>
						)}
					</section>
				);
			})}
		</div>
	);
}
