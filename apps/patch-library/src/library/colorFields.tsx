import {
	Button,
	CheckboxField,
	ColorPickerField,
	FormLayout,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import type {
	ColorSystem,
	ColorSystemCalibration,
	FixtureChannel,
	SubtractiveCalibration,
} from "../wire";
import {
	hexToXyz,
	wheelSlotDisplayXyz,
	wheelSlotsFromChannel,
	xyyToXyz,
	xyzToHex,
	xyzToXyy,
} from "../sheet/fixtureProfileModel";

export function AdditiveColorEditor({
	system,
	channels,
	options,
	onChange,
}: {
	system: Extract<ColorSystem, { type: "additive" }>;
	channels: FixtureChannel[];
	options: Array<{ value: string; label: string }>;
	onChange: (system: Extract<ColorSystem, { type: "additive" }>) => void;
}) {
	const setEmitter = (
		index: number,
		patch: Partial<(typeof system.emitters)[number]>,
	) =>
		onChange({
			...system,
			emitters: system.emitters.map((candidate, itemIndex) =>
				itemIndex === index ? { ...candidate, ...patch } : candidate,
			),
		});
	return (
		<div className="color-emitter-list">
			{system.emitters.map((emitter, index) => (
				<article key={`${emitter.channel_id}-${index}`}>
					<SelectField
						label="Emitter channel"
						value={emitter.channel_id}
						options={options}
						onChange={(channel_id) => setEmitter(index, { channel_id })}
					/>
					<TextField
						label="Emitter name"
						value={emitter.name}
						onChange={(event) =>
							setEmitter(index, { name: event.target.value })
						}
					/>
					{(["x", "y", "z"] as const).map((axis) => (
						<NumberField
							key={axis}
							label={`Measured XYZ ${axis.toUpperCase()}`}
							allowDecimal
							min={0}
							value={emitter.xyz[axis]}
							onChange={(event) =>
								setEmitter(index, {
									xyz: { ...emitter.xyz, [axis]: Number(event.target.value) },
								})
							}
						/>
					))}
					<XyyFields
						xyz={emitter.xyz}
						onChange={(xyz) => setEmitter(index, { xyz })}
					/>
					<NumberField
						label="Maximum level"
						allowDecimal
						min={0}
						max={1}
						step={0.01}
						value={emitter.maximum_level}
						onChange={(event) =>
							setEmitter(index, { maximum_level: Number(event.target.value) })
						}
					/>
					<NumberField
						label="Response curve"
						allowDecimal
						min={0.01}
						step={0.01}
						value={emitter.response_curve}
						onChange={(event) =>
							setEmitter(index, { response_curve: Number(event.target.value) })
						}
					/>
					<CheckboxField
						label="Participates in visible color matching"
						stateLabel="Include emitter"
						checked={emitter.visible}
						onChange={(event) =>
							setEmitter(index, { visible: event.target.checked })
						}
					/>
					<Button
						onClick={() =>
							onChange({
								...system,
								emitters: system.emitters.filter(
									(_, itemIndex) => itemIndex !== index,
								),
							})
						}
					>
						Remove emitter
					</Button>
				</article>
			))}
			<Button
				disabled={!channels.length}
				onClick={() => {
					const channel = channels[0];
					if (channel)
						onChange({
							...system,
							emitters: [
								...system.emitters,
								{
									channel_id: channel.id,
									name: channel.attribute,
									xyz: { x: 0.33, y: 0.33, z: 0.34 },
									maximum_level: 1,
									response_curve: 1,
									visible: !channel.attribute.endsWith("uv"),
								},
							],
						});
				}}
			>
				Add emitter
			</Button>
		</div>
	);
}

export function XyyFields({
	xyz,
	onChange,
}: {
	xyz: { x: number; y: number; z: number };
	onChange: (xyz: { x: number; y: number; z: number }) => void;
}) {
	const value = xyzToXyy(xyz);
	const set = (patch: Partial<typeof value>) =>
		onChange(xyyToXyz({ ...value, ...patch }));
	return (
		<details className="xyy-entry">
			<summary>Enter measured xyY</summary>
			<NumberField
				label="Chromaticity x"
				allowDecimal
				min={0}
				max={1}
				step={0.0001}
				value={value.x}
				onChange={(event) => set({ x: Number(event.target.value) })}
			/>
			<NumberField
				label="Chromaticity y"
				allowDecimal
				min={0}
				max={1}
				step={0.0001}
				value={value.y}
				onChange={(event) => set({ y: Number(event.target.value) })}
			/>
			<NumberField
				label="Luminance Y"
				allowDecimal
				min={0}
				value={value.luminance}
				onChange={(event) => set({ luminance: Number(event.target.value) })}
			/>
		</details>
	);
}

const DEFAULT_CALIBRATION: ColorSystemCalibration = {
	status: "nominal",
	revision: 0,
};

/** How far Color Intent may trust this system's colour data, and which revision it is. */
export function ColorCalibrationFields({
	headName,
	calibration,
	onChange,
}: {
	headName: string;
	calibration: ColorSystemCalibration | undefined;
	onChange: (calibration: ColorSystemCalibration) => void;
}) {
	const value = calibration ?? DEFAULT_CALIBRATION;
	return (
		<fieldset className="color-calibration">
			<legend>Color Intent calibration</legend>
			<p>
				Measured data can promise an exact colour; nominal data is typical;
				uncalibrated data cannot promise a colour. Raise the revision whenever
				the colour data changes.
			</p>
			<FormLayout columns={3} minColumnWidth={200}>
				<SelectField
					label="Calibration"
					ariaLabel={`${headName} calibration`}
					value={value.status}
					options={[
						{ value: "measured", label: "Measured" },
						{ value: "nominal", label: "Nominal (datasheet)" },
						{ value: "uncalibrated", label: "Uncalibrated" },
					]}
					onChange={(status) => onChange({ ...value, status })}
				/>
				<NumberField
					label="Calibration revision"
					min={0}
					value={value.revision}
					onChange={(event) =>
						onChange({ ...value, revision: Number(event.target.value) })
					}
				/>
				<TextField
					label="Calibration source"
					value={value.source ?? ""}
					onChange={(event) =>
						onChange({ ...value, source: event.target.value || null })
					}
				/>
			</FormLayout>
		</fieldset>
	);
}

const FILTER_FIELDS: ReadonlyArray<{
	key: keyof SubtractiveCalibration;
	label: string;
}> = [
	{ key: "open_xyz", label: "Open beam" },
	{ key: "cyan_xyz", label: "Cyan flag in" },
	{ key: "magenta_xyz", label: "Magenta flag in" },
	{ key: "yellow_xyz", label: "Yellow flag in" },
];

function SubtractiveFilterFields({
	filters,
	onChange,
}: {
	filters: SubtractiveCalibration | null | undefined;
	onChange: (filters: SubtractiveCalibration | null) => void;
}) {
	const white = { x: 0.95047, y: 1, z: 1.08883 };
	return (
		<>
			<CheckboxField
				label="Measured filter output"
				stateLabel="Use measured CMY filters"
				checked={Boolean(filters)}
				onChange={(event) =>
					onChange(
						event.target.checked
							? {
									open_xyz: white,
									cyan_xyz: { x: 0.54, y: 0.79, z: 1.07 },
									magenta_xyz: { x: 0.59, y: 0.28, z: 0.97 },
									yellow_xyz: { x: 0.77, y: 0.93, z: 0.14 },
								}
							: null,
					)
				}
			/>
			{filters &&
				FILTER_FIELDS.map(({ key, label }) => (
					<FormLayout key={key} columns={3} minColumnWidth={140}>
						{(["x", "y", "z"] as const).map((axis) => (
							<NumberField
								key={axis}
								label={`${label} ${axis.toUpperCase()}`}
								allowDecimal
								min={0}
								value={filters[key][axis]}
								onChange={(event) =>
									onChange({
										...filters,
										[key]: {
											...filters[key],
											[axis]: Number(event.target.value),
										},
									})
								}
							/>
						))}
					</FormLayout>
				))}
		</>
	);
}

export function SubtractiveColorEditor({
	system,
	options,
	onChange,
}: {
	system: Extract<ColorSystem, { type: "subtractive" }>;
	options: Array<{ value: string; label: string }>;
	onChange: (system: Extract<ColorSystem, { type: "subtractive" }>) => void;
}) {
	return (
		<>
		<SubtractiveFilterFields
			filters={system.filters}
			onChange={(filters) => onChange({ ...system, filters })}
		/>
		<FormLayout columns={3}>
			{(
				["cyan_channel_id", "magenta_channel_id", "yellow_channel_id"] as const
			).map((key) => (
				<SelectField
					key={key}
					label={
						key.split("_")[0][0].toUpperCase() + key.split("_")[0].slice(1)
					}
					value={system[key]}
					options={options}
					onChange={(value) => onChange({ ...system, [key]: value })}
				/>
			))}
		</FormLayout>
		</>
	);
}

export function HueSaturationColorEditor({
	system,
	options,
	onChange,
}: {
	system: Extract<ColorSystem, { type: "hue_saturation" }>;
	options: Array<{ value: string; label: string }>;
	onChange: (system: Extract<ColorSystem, { type: "hue_saturation" }>) => void;
}) {
	return (
		<FormLayout columns={3}>
			<SelectField
				label="Hue"
				value={system.hue_channel_id}
				options={options}
				onChange={(hue_channel_id) => onChange({ ...system, hue_channel_id })}
			/>
			<SelectField
				label="Saturation"
				value={system.saturation_channel_id}
				options={options}
				onChange={(saturation_channel_id) =>
					onChange({ ...system, saturation_channel_id })
				}
			/>
			<SelectField
				label="Color intensity"
				value={system.intensity_channel_id ?? ""}
				options={[
					{ value: "", label: "Independent fixture intensity" },
					...options,
				]}
				onChange={(intensity_channel_id) =>
					onChange({
						...system,
						intensity_channel_id: intensity_channel_id || null,
					})
				}
			/>
		</FormLayout>
	);
}

export function DiscreteColorEditor({
	system,
	channels,
	options,
	onChange,
}: {
	system: Extract<ColorSystem, { type: "discrete_wheel" }>;
	channels: FixtureChannel[];
	options: Array<{ value: string; label: string }>;
	onChange: (system: Extract<ColorSystem, { type: "discrete_wheel" }>) => void;
}) {
	const wheel = channels.find((channel) => channel.id === system.channel_id);
	const wheelSlots = wheel ? wheelSlotsFromChannel(wheel, system.slots) : [];
	const setSlot = (
		index: number,
		patch: Partial<(typeof system.slots)[number]>,
	) =>
		onChange({
			...system,
			slots: system.slots.map((slot, itemIndex) =>
				itemIndex === index ? { ...slot, ...patch } : slot,
			),
		});
	return (
		<div className="color-wheel-editor">
			<SelectField
				label="Wheel channel"
				value={system.channel_id}
				options={options}
				onChange={(channel_id) => onChange({ ...system, channel_id })}
			/>
			<p>
				Each slot's display color drives the Visualizer. A slot without one
				shows the color its name describes, such as Open as white.
			</p>
			<Button
				disabled={!wheelSlots.length}
				onClick={() => onChange({ ...system, slots: wheelSlots })}
			>
				Fill slots from wheel functions
			</Button>
			{system.slots.map((slot, index) => {
				const display = wheelSlotDisplayXyz(slot);
				return (
				<article key={`${slot.semantic_id}-${index}`}>
					<TextField
						label="Portable color ID"
						value={slot.semantic_id}
						onChange={(event) =>
							setSlot(index, { semantic_id: event.target.value })
						}
					/>
					<TextField
						label="Fixture label"
						value={slot.label}
						onChange={(event) => setSlot(index, { label: event.target.value })}
					/>
					<NumberField
						label="DMX from"
						min={0}
						value={slot.dmx_from}
						onChange={(event) =>
							setSlot(index, { dmx_from: Number(event.target.value) })
						}
					/>
					<NumberField
						label="DMX to"
						min={0}
						value={slot.dmx_to}
						onChange={(event) =>
							setSlot(index, { dmx_to: Number(event.target.value) })
						}
					/>
					<ColorPickerField
						label={`${slot.label || slot.semantic_id} display color`}
						description={
							slot.measured_xyz
								? "Defined color"
								: display
									? "From the slot name"
									: "No color: the Visualizer keeps the fixture's own"
						}
						value={display ? xyzToHex(display) : ""}
						onChange={(hex) => setSlot(index, { measured_xyz: hexToXyz(hex) })}
					/>
					<SelectField
						label="Steady colour for Color Intent"
						ariaLabel={`${slot.label || slot.semantic_id} steady colour`}
						value={
							slot.steady == null ? "auto" : slot.steady ? "steady" : "moving"
						}
						options={[
							{ value: "auto", label: "Judge by the slot name" },
							{ value: "steady", label: "Steady colour" },
							{ value: "moving", label: "Not steady (split, scroll, effect)" },
						]}
						onChange={(value) =>
							setSlot(index, {
								steady: value === "auto" ? null : value === "steady",
							})
						}
					/>
					<CheckboxField
						label="Measured XYZ available"
						stateLabel="Use measured color"
						checked={Boolean(slot.measured_xyz)}
						onChange={(event) =>
							setSlot(index, {
								measured_xyz: event.target.checked
									? (display ?? { x: 0.33, y: 0.33, z: 0.34 })
									: null,
							})
						}
					/>
					{slot.measured_xyz && (
						<>
							{(["x", "y", "z"] as const).map((axis) => (
								<NumberField
									key={axis}
									label={`Measured XYZ ${axis.toUpperCase()}`}
									allowDecimal
									min={0}
									value={slot.measured_xyz?.[axis] ?? 0}
									onChange={(event) =>
										setSlot(index, {
											measured_xyz: {
												...(slot.measured_xyz ?? { x: 0, y: 0, z: 0 }),
												[axis]: Number(event.target.value),
											},
										})
									}
								/>
							))}
							<XyyFields
								xyz={slot.measured_xyz}
								onChange={(measured_xyz) => setSlot(index, { measured_xyz })}
							/>
						</>
					)}
					<Button
						onClick={() =>
							onChange({
								...system,
								slots: system.slots.filter(
									(_, itemIndex) => itemIndex !== index,
								),
							})
						}
					>
						Remove slot
					</Button>
				</article>
				);
			})}
			<Button
				onClick={() =>
					onChange({
						...system,
						slots: [
							...system.slots,
							{
								semantic_id: "color.open",
								label: "Open",
								dmx_from: 0,
								dmx_to: 0,
								measured_xyz: null,
							},
						],
					})
				}
			>
				Add color slot
			</Button>
		</div>
	);
}
