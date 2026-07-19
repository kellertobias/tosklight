import type { FixtureMode } from "../../../api/types";
import { Button, NumberField, SelectField, TextField } from "../../common";
import { maxRaw } from "../fixtureProfileModel";

type ControlAction = FixtureMode["control_actions"][number];
type Assignment = ControlAction["assignments"][number];

function AssignmentRow({
	assignment,
	mode,
	onChange,
	onRemove,
}: {
	assignment: Assignment;
	mode: FixtureMode;
	onChange: (assignment: Assignment) => void;
	onRemove: () => void;
}) {
	const channel = mode.channels.find(
		(candidate) => candidate.id === assignment.channel_id,
	);
	const maximum = channel ? maxRaw(channel.resolution) : 0xffffffff;
	return (
		<div>
			<SelectField
				label="Channel"
				value={assignment.channel_id}
				options={mode.channels.map((candidate) => ({
					value: candidate.id,
					label: candidate.attribute,
				}))}
				onChange={(channel_id) => onChange({ ...assignment, channel_id })}
			/>
			<NumberField
				label="Active raw"
				min={0}
				max={maximum}
				value={assignment.active_raw}
				onChange={(event) =>
					onChange({
						...assignment,
						active_raw: Number(event.target.value),
					})
				}
			/>
			<NumberField
				label="Inactive raw"
				min={0}
				max={maximum}
				value={assignment.inactive_raw}
				onChange={(event) =>
					onChange({
						...assignment,
						inactive_raw: Number(event.target.value),
					})
				}
			/>
			<Button
				iconOnly
				aria-label="Remove control assignment"
				onClick={onRemove}
			>
				×
			</Button>
		</div>
	);
}

function ControlAssignments({
	action,
	mode,
	onChange,
}: {
	action: ControlAction;
	mode: FixtureMode;
	onChange: (action: ControlAction) => void;
}) {
	const setAssignment = (index: number, assignment: Assignment) =>
		onChange({
			...action,
			assignments: action.assignments.map((candidate, itemIndex) =>
				itemIndex === index ? assignment : candidate,
			),
		});
	return (
		<div className="control-assignments">
			{action.assignments.map((assignment, index) => (
				<AssignmentRow
					key={`${assignment.channel_id}-${index}`}
					assignment={assignment}
					mode={mode}
					onChange={(next) => setAssignment(index, next)}
					onRemove={() =>
						onChange({
							...action,
							assignments: action.assignments.filter(
								(_, itemIndex) => itemIndex !== index,
							),
						})
					}
				/>
			))}
			<Button
				disabled={!mode.channels.length}
				onClick={() => {
					const channel = mode.channels[0];
					if (!channel) return;
					onChange({
						...action,
						assignments: [
							...action.assignments,
							{
								channel_id: channel.id,
								active_raw: maxRaw(channel.resolution),
								inactive_raw: 0,
							},
						],
					});
				}}
			>
				Add channel assignment
			</Button>
		</div>
	);
}

export function ControlActionCard({
	action,
	mode,
	onChange,
	onRemove,
}: {
	action: ControlAction;
	mode: FixtureMode;
	onChange: (action: ControlAction) => void;
	onRemove: () => void;
}) {
	return (
		<article>
			<TextField
				label="Action name"
				value={action.name}
				onChange={(event) => onChange({ ...action, name: event.target.value })}
			/>
			<SelectField
				label="Operator action"
				value={action.semantic ?? "custom"}
				options={[
					{ value: "custom", label: "Custom / Direct Mode only" },
					{ value: "lamp_on", label: "Lamp On (strike)" },
					{ value: "lamp_off", label: "Lamp Off" },
					{ value: "reset", label: "Reset" },
					{ value: "fan_auto", label: "Fan Auto" },
					{ value: "fan_low", label: "Fan Low" },
					{ value: "fan_high", label: "Fan High" },
					{ value: "fan_max", label: "Fan Max" },
				]}
				onChange={(semantic) => onChange({ ...action, semantic })}
			/>
			<SelectField
				label="Action kind"
				value={action.kind}
				options={[
					{ value: "latched", label: "Latched" },
					{ value: "momentary", label: "Momentary" },
					{ value: "timed_pulse", label: "Timed pulse" },
				]}
				onChange={(kind) =>
					onChange({
						...action,
						kind,
						duration_millis:
							kind === "timed_pulse" ? (action.duration_millis ?? 1000) : null,
					})
				}
			/>
			{action.kind === "timed_pulse" && (
				<NumberField
					label="Duration (ms)"
					min={1}
					value={action.duration_millis ?? 1000}
					onChange={(event) =>
						onChange({
							...action,
							duration_millis: Number(event.target.value),
						})
					}
				/>
			)}
			<Button onClick={onRemove}>Remove action</Button>
			<ControlAssignments action={action} mode={mode} onChange={onChange} />
		</article>
	);
}
