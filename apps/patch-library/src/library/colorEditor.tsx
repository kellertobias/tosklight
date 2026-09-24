import { Button, FormLayout, NumberField, SelectField } from "@tosklight/ui";
import type {
	ColorSystem,
	ColorSystemCalibration,
	FixtureMode,
	HeadColorSystem,
} from "../wire";
import { reconcileColorSystemHighlightDefaults } from "../sheet/fixtureProfileModel";
import {
	AdditiveColorEditor,
	ColorCalibrationFields,
	DiscreteColorEditor,
	HueSaturationColorEditor,
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
		system.system.type === "hue_saturation" &&
		[
			system.system.hue_channel_id,
			system.system.saturation_channel_id,
		].includes(channelId)
	)
		return null;
	if (
		system.system.type === "hue_saturation" &&
		system.system.intensity_channel_id === channelId
	)
		return {
			...system,
			system: { ...system.system, intensity_channel_id: null },
		};
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
	// A hybrid head carries further systems (a wheel beside CMY) that this editor does not show;
	// editing the first one keeps them.
	if (existing)
		return systems.map((candidate) =>
			candidate === existing ? { ...existing, system } : candidate,
		);
	return [
		...systems,
		{
			head_id: headId,
			correction_matrix: identityColorCorrectionMatrix(),
			system,
		},
	];
}

/** Every channel ID a color system names, in a stable order. */
function colorSystemChannelIds(system: ColorSystem): string[] {
	if (system.type === "additive")
		return system.emitters.map((emitter) => emitter.channel_id);
	if (system.type === "subtractive")
		return [
			system.cyan_channel_id,
			system.magenta_channel_id,
			system.yellow_channel_id,
		];
	if (system.type === "hue_saturation")
		return [
			system.hue_channel_id,
			system.saturation_channel_id,
			...(system.intensity_channel_id ? [system.intensity_channel_id] : []),
		];
	return [system.channel_id];
}

function rebindColorSystem(
	system: ColorSystem,
	rebind: (channelId: string) => string,
): ColorSystem {
	if (system.type === "additive")
		return {
			...system,
			emitters: system.emitters.map((emitter) => ({
				...emitter,
				channel_id: rebind(emitter.channel_id),
			})),
		};
	if (system.type === "subtractive")
		return {
			...system,
			cyan_channel_id: rebind(system.cyan_channel_id),
			magenta_channel_id: rebind(system.magenta_channel_id),
			yellow_channel_id: rebind(system.yellow_channel_id),
		};
	if (system.type === "hue_saturation")
		return {
			...system,
			hue_channel_id: rebind(system.hue_channel_id),
			saturation_channel_id: rebind(system.saturation_channel_id),
			intensity_channel_id: system.intensity_channel_id
				? rebind(system.intensity_channel_id)
				: null,
		};
	return { ...system, channel_id: rebind(system.channel_id) };
}

/** A channel's place in its head: its attribute and how many same-attribute channels precede it. */
function channelRole(mode: FixtureMode, channelId: string) {
	const channel = mode.channels.find((candidate) => candidate.id === channelId);
	if (!channel) return null;
	const occurrence = mode.channels
		.filter(
			(candidate) =>
				candidate.head_id === channel.head_id &&
				candidate.attribute === channel.attribute,
		)
		.findIndex((candidate) => candidate.id === channelId);
	return { attribute: channel.attribute, occurrence };
}

/**
 * Give every other head that has matching channels its own copy of one head's color system. Each
 * copy is bound to that head's own channels, matched by attribute and order, so heads stay
 * independently editable afterwards. Heads without every matching channel keep what they have.
 */
export function copyColorSystemToOtherHeads(
	mode: FixtureMode,
	sourceHeadId: string,
): HeadColorSystem[] {
	const source = mode.color_systems.find(
		(candidate) => candidate.head_id === sourceHeadId,
	);
	if (!source) return mode.color_systems;
	const roles = new Map(
		colorSystemChannelIds(source.system).map((id) => [
			id,
			channelRole(mode, id),
		]),
	);
	let systems = mode.color_systems;
	for (const head of mode.heads) {
		if (head.id === sourceHeadId) continue;
		const own = mode.channels.filter((channel) => channel.head_id === head.id);
		const mapping = new Map<string, string>();
		for (const [id, role] of roles) {
			const target = role
				? own.filter((channel) => channel.attribute === role.attribute)[
						role.occurrence
					]
				: undefined;
			if (target) mapping.set(id, target.id);
		}
		if (mapping.size !== roles.size) continue;
		const copy = structuredClone(source);
		systems = [
			...systems.filter((candidate) => candidate.head_id !== head.id),
			{
				...copy,
				head_id: head.id,
				system: rebindColorSystem(
					copy.system,
					(id) => mapping.get(id) ?? id,
				),
			},
		];
	}
	return systems;
}

function newColorSystem(
	next: string,
	channels: FixtureMode["channels"],
): ColorSystem | null {
	if (next === "none") return null;
	const first = channels[0]?.id ?? "";
	if (next === "additive") return { type: next, emitters: [] };
	if (next === "subtractive")
		return {
			type: next,
			cyan_channel_id: first,
			magenta_channel_id: first,
			yellow_channel_id: first,
		};
	if (next === "hue_saturation")
		return {
			type: next,
			hue_channel_id: first,
			saturation_channel_id: channels[1]?.id ?? first,
			intensity_channel_id: null,
		};
	return { type: "discrete_wheel", channel_id: first, slots: [] };
}

function CorrectionMatrixFields({
	headName,
	matrix,
	onChange,
}: {
	headName: string;
	matrix: HeadColorSystem["correction_matrix"];
	onChange: (row: number, column: number, value: number) => void;
}) {
	return (
		<fieldset className="color-correction-matrix">
			<legend>XYZ correction matrix</legend>
			<p>
				Applied before calibrated color matching. Identity leaves requested XYZ
				unchanged.
			</p>
			{/* Narrower than the default column, so the three columns still fit a narrow mode
			    editor with room for a decimal beside the steppers. */}
			<FormLayout columns={3} minColumnWidth={200}>
				{matrix.flatMap((row, rowIndex) =>
					row.map((value, columnIndex) => (
						<NumberField
							key={`${rowIndex}-${columnIndex}`}
							aria-label={`${headName} correction row ${rowIndex + 1} column ${columnIndex + 1}`}
							allowDecimal
							step={0.001}
							value={value}
							onChange={(event) =>
								onChange(rowIndex, columnIndex, Number(event.target.value))
							}
						/>
					)),
				)}
			</FormLayout>
		</fieldset>
	);
}

/** One head's correction matrix with a single entry changed. */
function withCorrection(
	systems: HeadColorSystem[],
	headId: string,
	row: number,
	column: number,
	value: number,
): HeadColorSystem[] {
	return systems.map((candidate) =>
		candidate.head_id === headId
			? {
					...candidate,
					correction_matrix: candidate.correction_matrix.map(
						(values, rowIndex) =>
							values.map((entry, columnIndex) =>
								rowIndex === row && columnIndex === column ? value : entry,
							),
					) as HeadColorSystem["correction_matrix"],
				}
			: candidate,
	);
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
				withCorrection(mode.color_systems, headId, row, column, value),
			),
		);
	const setCalibration = (
		record: HeadColorSystem,
		calibration: ColorSystemCalibration,
	) =>
		onChange({
			...mode,
			color_systems: mode.color_systems.map((candidate) =>
				candidate === record ? { ...candidate, calibration } : candidate,
			),
		});
	return (
		<div className="fixture-color-editor">
			<p>
				Abstract XYZ color is resolved through one color system per logical
				head, so every head of a multi-head fixture is configured on its own.
				Direct emitter channels remain available to the programmer.
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
									{ value: "hue_saturation", label: "Hue / saturation" },
									{ value: "discrete_wheel", label: "Discrete color wheel" },
								]}
								onChange={(next) => {
									setSystem(head.id, newColorSystem(next, channels));
								}}
							/>
							{record && mode.heads.length > 1 && (
								<Button
									onClick={() =>
										onChange(
											reconcileColorSystemHighlightDefaults(
												mode,
												copyColorSystemToOtherHeads(mode, head.id),
											),
										)
									}
								>
									Copy to other heads
								</Button>
							)}
						</header>
						{record && (
							<ColorCalibrationFields
								headName={head.name}
								calibration={record.calibration}
								onChange={(calibration) => setCalibration(record, calibration)}
							/>
						)}
						{record && (
							<CorrectionMatrixFields
								headName={head.name}
								matrix={record.correction_matrix}
								onChange={(row, column, value) =>
									setCorrection(head.id, row, column, value)
								}
							/>
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
								channels={channels}
								options={options}
								onChange={(system) => setSystem(head.id, system)}
							/>
						)}
						{record?.system.type === "hue_saturation" && (
							<HueSaturationColorEditor
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
