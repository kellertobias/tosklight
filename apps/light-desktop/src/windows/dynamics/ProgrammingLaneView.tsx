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

/** Inspection for retained typed definitions while the feature-specific composer is staged.
 * No scalar curve or percentage controls are allowed to reinterpret a complete intent. */
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
				<p>Intent curve editing and preview are not available in this build.</p>
			</section>
		</div>
	);
}
