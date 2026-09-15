/**
 * The Placement Assistant: lays the selected elements out along a line, in a grid or around a
 * circle, in the order they were selected. Nothing moves until **Apply**.
 */
import { Button, ModalFrame } from "@tosklight/ui";
import { useState } from "react";
import { CommitNumber } from "./cadFields";
import {
	type AssistantLayout,
	type AssistantShape,
	assistantPositions,
	type PlanPosition,
} from "./placementAssistant";

const SHAPES: readonly { shape: AssistantShape; label: string }[] = [
	{ shape: "line", label: "Line" },
	{ shape: "grid", label: "Grid" },
	{ shape: "circle", label: "Circle" },
];

function metres({ x, y, z }: PlanPosition): PlanPosition {
	return { x: x / 1000, y: y / 1000, z: z / 1000 };
}

function PositionFields({
	label,
	value,
	onChange,
}: {
	label: string;
	value: PlanPosition;
	onChange(value: PlanPosition): void;
}) {
	return (
		<div className="cad-info-vector" role="group" aria-label={label}>
			<span>{label}</span>
			{(["x", "y", "z"] as const).map((axis) => (
				<CommitNumber
					key={axis}
					label={axis.toUpperCase()}
					ariaLabel={`${label} ${axis.toUpperCase()}`}
					unit="m"
					value={value[axis]}
					onCommit={(next) => onChange({ ...value, [axis]: next })}
				/>
			))}
		</div>
	);
}

/** Where each shape starts, taken from where the selection already stands (in millimetres). */
function initialLayouts(current: readonly PlanPosition[]) {
	const first = metres(current[0] ?? { x: 0, y: 0, z: 0 });
	const last = metres(current[current.length - 1] ?? current[0] ?? { x: 0, y: 0, z: 0 });
	const centre = metres({
		x: current.reduce((sum, each) => sum + each.x, 0) / Math.max(1, current.length),
		y: current.reduce((sum, each) => sum + each.y, 0) / Math.max(1, current.length),
		z: current.reduce((sum, each) => sum + each.z, 0) / Math.max(1, current.length),
	});
	return {
		line: { shape: "line", start: first, end: last },
		grid: {
			shape: "grid",
			start: first,
			columns: Math.max(1, Math.ceil(Math.sqrt(current.length))),
			spacingX: 1,
			spacingY: 1,
		},
		circle: { shape: "circle", centre, radius: 2, startAngle: 0, arc: 360 },
	} satisfies { [shape in AssistantShape]: Extract<AssistantLayout, { shape: shape }> };
}

export function CadPlacementAssistant({
	current,
	onApply,
	onClose,
}: {
	/** Where each selected element stands now, in millimetres, in selection order. */
	current: readonly PlanPosition[];
	onApply(positions: PlanPosition[]): void;
	onClose(): void;
}) {
	const [layouts, setLayouts] = useState(() => initialLayouts(current));
	const [shape, setShape] = useState<AssistantShape>("line");
	const change = <S extends AssistantShape>(
		next: S,
		patch: Partial<Extract<AssistantLayout, { shape: S }>>,
	) => setLayouts((all) => ({ ...all, [next]: { ...all[next], ...patch } }));
	const { line, grid, circle } = layouts;

	return (
		<ModalFrame
			role="dialog"
			ariaLabel="Placement Assistant"
			dialogClassName="cad-placement-assistant"
			title="Placement Assistant"
			closeLabel="Close without moving"
			onClose={onClose}
		>
			<div className="cad-placement-assistant-body">
				<div className="cad-placement-assistant-shapes" role="group" aria-label="Arrangement">
					{SHAPES.map((each) => (
						<Button
							key={each.shape}
							aria-pressed={shape === each.shape}
							className={shape === each.shape ? "is-active" : undefined}
							onClick={() => setShape(each.shape)}
						>
							{each.label}
						</Button>
					))}
				</div>
				<p>
					{current.length} elements are placed in the order they were selected.
				</p>
				{shape === "line" ? (
					<>
						<PositionFields label="Start" value={line.start} onChange={(start) => change("line", { start })} />
						<PositionFields label="End" value={line.end} onChange={(end) => change("line", { end })} />
					</>
				) : null}
				{shape === "grid" ? (
					<>
						<PositionFields label="Start" value={grid.start} onChange={(start) => change("grid", { start })} />
						<div className="cad-info-vector">
							<CommitNumber
								label="Columns"
								digits={0}
								min={1}
								value={grid.columns}
								onCommit={(columns) => change("grid", { columns: Math.round(columns) })}
							/>
							<CommitNumber label="Spacing X" unit="m" value={grid.spacingX} onCommit={(spacingX) => change("grid", { spacingX })} />
							<CommitNumber label="Spacing Y" unit="m" value={grid.spacingY} onCommit={(spacingY) => change("grid", { spacingY })} />
						</div>
					</>
				) : null}
				{shape === "circle" ? (
					<>
						<PositionFields label="Centre" value={circle.centre} onChange={(centre) => change("circle", { centre })} />
						<div className="cad-info-vector">
							<CommitNumber label="Radius" unit="m" min={0} value={circle.radius} onCommit={(radius) => change("circle", { radius })} />
							<CommitNumber label="Start angle" unit="°" digits={1} value={circle.startAngle} onCommit={(startAngle) => change("circle", { startAngle })} />
							<CommitNumber label="Arc" unit="°" digits={1} value={circle.arc} onCommit={(arc) => change("circle", { arc })} />
						</div>
					</>
				) : null}
				<footer>
					<Button onClick={onClose}>Cancel</Button>
					<Button
						className="primary"
						onClick={() => onApply(assistantPositions(layouts[shape], current.length))}
					>
						Apply
					</Button>
				</footer>
			</div>
		</ModalFrame>
	);
}
