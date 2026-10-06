import { Button } from "@tosklight/ui";
import type {
	DynamicDefinitionProjection,
	DynamicLaneProjection,
} from "../../api/types";
import {
	dynamicLaneLabel,
	dynamicLaneMode,
	type ProgrammingDynamicLane,
} from "../../features/dynamics/laneModel";

export function ProgrammingLaneRow({
	lane,
	index,
	selected,
	onSelect,
}: {
	lane: DynamicLaneProjection;
	index: number;
	selected: boolean;
	onSelect(id: string, additive: boolean): void;
}) {
	return (
		<li className={`dynamic-lane-overview ${selected ? "selected" : ""}`}>
			<Button
				className="dynamic-lane-identity-select"
				aria-label={`Select lane ${index + 1}, ${dynamicLaneLabel(lane)}`}
				aria-pressed={selected}
				onClick={(event) => onSelect(lane.id, event.shiftKey)}
			>
				<span className="dynamic-lane-identity">
					<small>Lane {index + 1}</small>
					<strong>{dynamicLaneLabel(lane)}</strong>
					<span>
						{dynamicLaneMode(lane).replaceAll("_", " ")} ·{" "}
						{lane.speed_multiplier.numerator}/
						{lane.speed_multiplier.denominator} speed
					</span>
				</span>
			</Button>
		</li>
	);
}

/** Inspection for a typed lane the curve composer cannot express: a whole-family keyframe lane
 * or a native Direct colour lane. Component lanes (Pan, Tilt, Zoom, Color components) are
 * composed in their descriptor units instead (see `editableLane`); no scalar curve or percentage
 * control may reinterpret a complete intent. */
export function ProgrammingLaneView({
	dynamic,
	lane,
	selectedLanes,
	onSelect,
}: {
	dynamic: DynamicDefinitionProjection;
	lane: ProgrammingDynamicLane;
	selectedLanes: ReadonlySet<string>;
	onSelect(id: string, additive: boolean): void;
}) {
	return (
		<div className="dynamic-curves-view">
			<ul className="dynamic-lane-overview-list" aria-label="Dynamic lanes">
				{dynamic.lanes.map((candidate, index) => (
					<ProgrammingLaneRow
						key={candidate.id}
						lane={candidate}
						index={index}
						selected={selectedLanes.has(candidate.id)}
						onSelect={onSelect}
					/>
				))}
			</ul>
			<section className="dynamic-lane-bottom-editor" aria-label="Intent lane">
				<strong>{dynamicLaneLabel(lane)}</strong>
				<span>{dynamicLaneMode(lane).replaceAll("_", " ")}</span>
				<p>The curve composer edits Pan, Tilt, Zoom and Color component lanes; this lane holds whole values.</p>
			</section>
		</div>
	);
}
