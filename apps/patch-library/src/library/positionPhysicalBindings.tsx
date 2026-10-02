import { Button, FormLayout, SelectField, NumberField } from "@tosklight/ui";
import type {
	FixtureMode,
	GeometryGraph,
	MotionFunctionBinding,
	PositionPhysicalModel,
} from "../fixtureProfile";
export function PositionPhysicalBindings({
	mode,
	geometry,
	onChange,
}: {
	mode: FixtureMode;
	geometry: GeometryGraph;
	onChange(mode: FixtureMode): void;
}) {
	const model = mode.position_physical;
	const axes = geometry.nodes.filter((n) => n.motion?.kind === "rotation");
	const functions = mode.channels.flatMap((c) =>
		c.functions
			.filter((f) => f.angular_motion && f.behavior.type === "continuous")
			.map((f) => ({
				value: `${c.id}:${f.id}`,
				label: `${mode.heads.find((h) => h.id === c.head_id)?.name ?? "Head"} · ${c.fixture_attribute} · ${f.name} · ${f.angular_motion!.kind === "angular_velocity" ? "°/s" : "°"}`,
				channel_id: c.id,
				function_id: f.id,
			})),
	);
	const set = (position_physical: PositionPhysicalModel | null) =>
		onChange({ ...mode, position_physical });
	const add = () => {
		if (axes[0] && functions[0])
			set({
				version: 1,
				revision: model?.revision ?? 0,
				bindings: [
					...(model?.bindings ?? []),
					{
						node_id: axes[0].id,
						channel_id: functions[0].channel_id,
						function_id: functions[0].function_id,
						role: "pan",
					},
				],
			});
	};
	if (!model)
		return (
			<section>
				<Button
					disabled={
						!geometry.physical_contract || !axes.length || !functions.length
					}
					onClick={add}
				>
					Configure physical Position
				</Button>
				<p className="field-hint">
					Requires a physical geometry declaration and functions explicitly
					typed as absolute degrees or rotation speed. Attribute names alone do
					not identify a motor.
				</p>
			</section>
		);
	const update = (index: number, change: Partial<MotionFunctionBinding>) =>
		set({
			...model,
			bindings: model.bindings.map((b, i) =>
				i === index ? { ...b, ...change } : b,
			),
		});
	return (
		<fieldset>
			<legend>Physical Position bindings</legend>
			<p className="field-hint">
				Exact functions drive physical axes; multi-turn degrees remain
				unwrapped. Velocity alone cannot supply an absolute position or target.
			</p>
			<NumberField
				label="Position model revision"
				min={0}
				max={0xffff_ffff}
				value={model.revision}
				onChange={(e) => set({ ...model, revision: Number(e.target.value) })}
			/>
			{model.bindings.map((b, i) => (
				<div key={i}>
					<FormLayout columns={3} minColumnWidth={170}>
						<SelectField
							label={`Binding ${i + 1} axis`}
							ariaLabel={`Binding ${i + 1} axis`}
							value={b.node_id}
							options={axes.map((n) => ({ value: n.id, label: n.name }))}
							onChange={(node_id) => update(i, { node_id })}
						/>
						<SelectField
							label={`Binding ${i + 1} role`}
							ariaLabel={`Binding ${i + 1} role`}
							value={b.role}
							options={[
								{ value: "pan", label: "Pan" },
								{ value: "tilt", label: "Tilt" },
							]}
							onChange={(role) => update(i, { role: role as "pan" | "tilt" })}
						/>
						<SelectField
							label={`Binding ${i + 1} native function`}
							ariaLabel={`Binding ${i + 1} native function`}
							value={`${b.channel_id}:${b.function_id}`}
							options={functions}
							onChange={(id) => {
								const f = functions.find((f) => f.value === id);
								if (f)
									update(i, {
										channel_id: f.channel_id,
										function_id: f.function_id,
									});
							}}
						/>
					</FormLayout>
					<Button
						onClick={() => {
							const bindings = model.bindings.filter((_, n) => n !== i);
							set(bindings.length ? { ...model, bindings } : null);
						}}
					>
						Remove binding {i + 1}
					</Button>
				</div>
			))}
			<Button disabled={!axes.length || !functions.length} onClick={add}>
				Add physical binding
			</Button>
		</fieldset>
	);
}
