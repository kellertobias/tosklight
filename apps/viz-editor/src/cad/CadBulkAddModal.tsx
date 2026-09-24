/**
 * **Place Multiple** for one truss or stage part, opened from the button beside that part in its
 * add menu.
 *
 * A truss run is laid from its first point to its last: the sections are spaced evenly along it,
 * each turned about Z to the run's heading and raised to the height of its own place along it, and
 * a straight section can be given its length (by default the length that fills the run end to end).
 * A stage grid is so many elements across and deep, butted edge to edge from the part's own
 * footprint and centred on the point given, with no gaps and no overlaps.
 *
 * The plan below the fields shows the arrangement as it will be placed. Nothing is placed until
 * **Place**, which places the whole arrangement in one step and selects it; **Cancel** places nothing.
 */
import { Button, ModalFrame } from "@tosklight/ui";
import { useState } from "react";
import {
	type MetrePoint,
	type PlanPlacement,
	stageGridPlacements,
	trussRunPlacements,
	trussRunSectionLength,
} from "./bulkPlacement";
import { CadBulkPreview } from "./CadBulkPreview";
import { CommitNumber } from "./cadFields";

/** Which arrangement a part opens: a grid of stage elements, or a run of truss. */
export type BulkShape = "stage" | "truss";

/** A 2 × 1 m deck, which is what a part with no footprint of its own is laid out as. */
const DEFAULT_FOOTPRINT = { width: 2, depth: 1 };

/** A truss part's size as its profile is built, and whether its length can be set. */
export interface TrussSize {
	metres: MetrePoint;
	lengthAdjustable: boolean;
	minimumLength: number;
	maximumLength: number;
}

/** A plain 0.3 m truss section 2 m long, for a part this computer's library does not describe. */
const DEFAULT_TRUSS: TrussSize = {
	metres: { x: 2, y: 0.3, z: 0.3 },
	lengthAdjustable: false,
	minimumLength: 0.1,
	maximumLength: 30,
};

function PointFields({
	label,
	point,
	onChange,
}: {
	label: string;
	point: MetrePoint;
	onChange(point: MetrePoint): void;
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
					value={point[axis]}
					onCommit={(value) => onChange({ ...point, [axis]: value })}
				/>
			))}
		</div>
	);
}

function Count({ label, value, onChange }: { label: string; value: number; onChange(value: number): void }) {
	return (
		<CommitNumber
			label={label}
			digits={0}
			min={1}
			max={200}
			value={value}
			onCommit={(next) => onChange(Math.max(1, Math.round(next)))}
		/>
	);
}

function useTrussRun(size: TrussSize) {
	const [first, setFirst] = useState<MetrePoint>({ x: -4, y: 0, z: 6 });
	const [last, setLast] = useState<MetrePoint>({ x: 4, y: 0, z: 6 });
	const [count, setCount] = useState(4);
	// Unset, a straight section is as long as fills the run end to end.
	const [length, setLength] = useState<number | null>(null);
	const run = { first, last, count };
	const filling = trussRunSectionLength(run);
	const shownLength = size.lengthAdjustable
		? Math.min(size.maximumLength, Math.max(size.minimumLength, length ?? (filling || size.metres.x)))
		: size.metres.x;
	return {
		fields: (
			<>
				<PointFields label="First point" point={first} onChange={setFirst} />
				<PointFields label="Last point" point={last} onChange={setLast} />
				<div className="cad-info-vector">
					<Count label="Sections" value={count} onChange={setCount} />
					{size.lengthAdjustable ? (
						<CommitNumber
							label="Section length"
							unit="m"
							min={size.minimumLength}
							max={size.maximumLength}
							value={shownLength}
							onCommit={setLength}
						/>
					) : null}
				</div>
				<p className="cad-thru-hint">
					{count} {count === 1 ? "section" : "sections"} along {filling * count > 0 ? (filling * count).toFixed(2) : "0"} m,
					each turned to the run's heading; a run whose ends differ in height is stepped from one to the
					other, never tilted.
				</p>
			</>
		),
		placements: trussRunPlacements(run),
		footprint: { length: shownLength, width: size.metres.z },
		size: size.lengthAdjustable ? { ...size.metres, x: shownLength } : undefined,
	};
}

function useStageGrid(footprint: { width: number; depth: number }) {
	const [turned, setTurned] = useState(false);
	const [columns, setColumns] = useState(2);
	const [rows, setRows] = useState(2);
	const [centre, setCentre] = useState<MetrePoint>({ x: 0, y: 0, z: 0 });
	const [across, deep] = turned ? [footprint.depth, footprint.width] : [footprint.width, footprint.depth];
	return {
		fields: (
			<>
				<div className="cad-placement-assistant-shapes" role="group" aria-label="Orientation">
					<Button aria-pressed={!turned} className={turned ? undefined : "is-active"} onClick={() => setTurned(false)}>
						Long side across
					</Button>
					<Button aria-pressed={turned} className={turned ? "is-active" : undefined} onClick={() => setTurned(true)}>
						Long side deep
					</Button>
				</div>
				<div className="cad-info-vector">
					<Count label="Across (X)" value={columns} onChange={setColumns} />
					<Count label="Deep (Y)" value={rows} onChange={setRows} />
				</div>
				<PointFields label="Grid centre" point={centre} onChange={setCentre} />
				<p className="cad-thru-hint">
					Each element is {across} × {deep} m and butts against the next, so the grid covers{" "}
					{(columns * across).toFixed(2)} × {(rows * deep).toFixed(2)} m with no gaps.
				</p>
			</>
		),
		placements: stageGridPlacements({ columns, rows, turned, footprint, centre }),
		footprint: { length: footprint.width, width: footprint.depth },
		size: undefined,
	};
}

export function CadBulkAddModal({
	shape,
	partLabel,
	footprint = DEFAULT_FOOTPRINT,
	truss = DEFAULT_TRUSS,
	placing,
	onPlace,
	onClose,
}: {
	shape: BulkShape;
	/** The part being placed, as its add menu names it. */
	partLabel: string;
	/** A stage part's own footprint in metres, as its profile is built. */
	footprint?: { width: number; depth: number };
	/** A truss part's own size, and whether its length can be set. */
	truss?: TrussSize;
	placing: boolean;
	/** Places the arrangement in one step; `size` is each section's size when its length was set. */
	onPlace(placements: readonly PlanPlacement[], size?: MetrePoint): void;
	onClose(): void;
}) {
	// Both are always kept, so hooks run in the same order; only the part's own shape is shown.
	const run = useTrussRun(truss);
	const grid = useStageGrid(footprint);
	const layout = shape === "truss" ? run : grid;
	const count = layout.placements.length;
	return (
		<ModalFrame
			role="dialog"
			ariaLabel={shape === "stage" ? "Place multiple stage elements" : "Place multiple trusses"}
			dialogClassName="cad-bulk-add"
			title={`Place Multiple · ${partLabel}`}
			closeLabel="Close without placing"
			onClose={onClose}
		>
			<div className="cad-placement-assistant-body">
				{layout.fields}
				<CadBulkPreview placements={layout.placements} footprint={layout.footprint} />
				<p aria-live="polite">
					{count} {count === 1 ? "element" : "elements"} will be placed.
				</p>
				<footer>
					<Button onClick={onClose}>Cancel</Button>
					<Button
						className="primary"
						disabled={placing || !count}
						onClick={() => onPlace(layout.placements, layout.size)}
					>
						{placing ? "Placing…" : `Place ${count}`}
					</Button>
				</footer>
			</div>
		</ModalFrame>
	);
}
