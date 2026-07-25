import type { FixtureMode, GeometryEmitter } from "../../../api/types";
import {
	Button,
	NumberField,
	SelectField,
	SwitchField,
	TextField,
} from "../../common";
import { GeometryEmitterLayout } from "./geometryEmitterLayout";
import { VectorFields } from "./geometryPreview";

export function GeometryEmitterForm({
	emitter,
	mode,
	onChange,
	onRemove,
}: {
	emitter: GeometryEmitter;
	mode: FixtureMode;
	onChange: (emitter: GeometryEmitter) => void;
	onRemove: () => void;
}) {
	return (
		<div>
			<h3>Emitter properties</h3>
			<TextField
				label="Emitter name"
				value={emitter.name}
				onChange={(event) => onChange({ ...emitter, name: event.target.value })}
			/>
			<SelectField
				label="Geometry part"
				value={emitter.node_id}
				options={mode.geometry.nodes.map((node) => ({
					value: node.id,
					label: node.name,
				}))}
				onChange={(node_id) => onChange({ ...emitter, node_id })}
			/>
			<SelectField
				label="Logical head"
				value={emitter.head_id}
				options={mode.heads.map((head) => ({
					value: head.id,
					label: head.name,
				}))}
				onChange={(head_id) => onChange({ ...emitter, head_id })}
			/>
			<VectorFields
				label="Origin"
				value={emitter.origin}
				onChange={(origin) => onChange({ ...emitter, origin })}
			/>
			<VectorFields
				label="Orientation °"
				value={emitter.orientation_degrees}
				onChange={(orientation_degrees) =>
					onChange({ ...emitter, orientation_degrees })
				}
			/>
			<NumberField
				label="Beam angle °"
				allowDecimal
				min={0}
				value={emitter.beam_angle_degrees}
				onChange={(event) =>
					onChange({
						...emitter,
						beam_angle_degrees: Number(event.target.value),
					})
				}
			/>
			<NumberField
				label="Field angle °"
				allowDecimal
				min={0}
				value={emitter.field_angle_degrees}
				onChange={(event) =>
					onChange({
						...emitter,
						field_angle_degrees: Number(event.target.value),
					})
				}
			/>
			<NumberField
				label="Feather"
				allowDecimal
				min={0}
				max={1}
				step={0.01}
				value={emitter.feather}
				onChange={(event) =>
					onChange({ ...emitter, feather: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Focus"
				allowDecimal
				min={0}
				max={1}
				step={0.01}
				value={emitter.focus}
				onChange={(event) =>
					onChange({ ...emitter, focus: Number(event.target.value) })
				}
			/>
			<SwitchField
				label="Projects a directional beam"
				checked={emitter.directional ?? true}
				onChange={(event) =>
					onChange({ ...emitter, directional: event.target.checked })
				}
			/>
			<GeometryEmitterLayout
				layout={emitter.layout}
				onChange={(layout) => onChange({ ...emitter, layout })}
			/>
			<Button variant="danger" onClick={onRemove}>
				Remove emitter
			</Button>
		</div>
	);
}
