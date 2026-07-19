import type { ColorSystem, FixtureChannel } from "../../../api/types";
import {
	Button,
	CheckboxField,
	FormLayout,
	NumberField,
	SelectField,
	TextField,
} from "../../common";
import { xyyToXyz, xyzToXyy } from "../fixtureProfileModel";

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
	);
}

export function DiscreteColorEditor({
	system,
	options,
	onChange,
}: {
	system: Extract<ColorSystem, { type: "discrete_wheel" }>;
	options: Array<{ value: string; label: string }>;
	onChange: (system: Extract<ColorSystem, { type: "discrete_wheel" }>) => void;
}) {
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
			{system.slots.map((slot, index) => (
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
					<CheckboxField
						label="Measured XYZ available"
						checked={Boolean(slot.measured_xyz)}
						onChange={(event) =>
							setSlot(index, {
								measured_xyz: event.target.checked
									? { x: 0.33, y: 0.33, z: 0.34 }
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
			))}
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
