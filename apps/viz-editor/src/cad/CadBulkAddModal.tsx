/**
 * The wizards behind **Place several…**: a field of stage elements, or rows of truss.
 *
 * A rig is rarely one of anything, and placing twenty decks one press at a time and then dragging
 * each into line is not planning. The stage wizard asks only which way round the elements go and
 * how many across and deep, and butts them edge to edge from the profile's own footprint, so the
 * field has no gaps to close afterwards. The truss wizard asks for the heights the run is flown at
 * and the lines it stands on, and flies the run again at every one.
 *
 * Nothing is placed until **Place**, and **Cancel** places nothing at all. What is placed is
 * selected afterwards, so the whole field can be moved, turned or adjusted as one.
 */
import { Button, ModalFrame } from "@tosklight/ui";
import { useState } from "react";
import {
	parseMetreList,
	type PlanPlacement,
	stageGridPlacements,
	trussRowPlacements,
} from "./bulkPlacement";
import { CommitNumber, CommitText } from "./cadFields";

/** Which wizard a part opens: a field of stage elements, or rows of truss. */
export type BulkShape = "stage" | "truss";

/** A 2 × 1 m deck, which is what a part with no footprint of its own is laid out as. */
const DEFAULT_FOOTPRINT = { width: 2, depth: 1 };

export function CadBulkAddModal({
	shape,
	partLabel,
	footprint = DEFAULT_FOOTPRINT,
	placing,
	onPlace,
	onClose,
}: {
	shape: BulkShape;
	/** What the wizard calls the part it is placing, as the part menu names it. */
	partLabel: string;
	/** The part's own footprint in metres, as its profile is built. */
	footprint?: { width: number; depth: number };
	placing: boolean;
	onPlace(placements: readonly PlanPlacement[]): void;
	onClose(): void;
}) {
	const [turned, setTurned] = useState(false);
	const [columns, setColumns] = useState(2);
	const [rows, setRows] = useState(2);
	const [heights, setHeights] = useState("5");
	const [positions, setPositions] = useState("0");

	const heightList = parseMetreList(heights);
	const positionList = parseMetreList(positions);
	const placements: readonly PlanPlacement[] | null =
		shape === "stage"
			? stageGridPlacements({ columns, rows, turned, footprint })
			: heightList && positionList
				? trussRowPlacements({ heights: heightList, positions: positionList, turned })
				: null;
	const count = placements?.length ?? 0;
	const [across, deep] = turned
		? [footprint.depth, footprint.width]
		: [footprint.width, footprint.depth];

	return (
		<ModalFrame
			role="dialog"
			ariaLabel={shape === "stage" ? "Place several stage elements" : "Place several trusses"}
			dialogClassName="cad-bulk-add"
			title={`Place several · ${partLabel}`}
			closeLabel="Close without placing"
			onClose={onClose}
		>
			<div className="cad-placement-assistant-body">
				<div className="cad-placement-assistant-shapes" role="group" aria-label="Orientation">
					<Button
						aria-pressed={!turned}
						className={turned ? undefined : "is-active"}
						onClick={() => setTurned(false)}
					>
						{shape === "stage" ? "Long side across" : "Runs across"}
					</Button>
					<Button
						aria-pressed={turned}
						className={turned ? "is-active" : undefined}
						onClick={() => setTurned(true)}
					>
						{shape === "stage" ? "Long side deep" : "Runs deep"}
					</Button>
				</div>
				{shape === "stage" ? (
					<>
						<div className="cad-info-vector">
							<CommitNumber
								label="Across"
								ariaLabel="Elements across"
								digits={0}
								min={1}
								value={columns}
								onCommit={(next) => setColumns(Math.max(1, Math.round(next)))}
							/>
							<CommitNumber
								label="Deep"
								ariaLabel="Elements deep"
								digits={0}
								min={1}
								value={rows}
								onCommit={(next) => setRows(Math.max(1, Math.round(next)))}
							/>
						</div>
						<p className="cad-thru-hint">
							Each element is {across} × {deep} m and butts against the last, so the field covers{" "}
							{(columns * across).toFixed(2)} × {(rows * deep).toFixed(2)} m with no gaps. The first
							element lands where one placed on its own would.
						</p>
					</>
				) : (
					<>
						<CommitText
							label="Heights (m)"
							ariaLabel="Heights"
							value={heights}
							accepts={(draft) => parseMetreList(draft) != null}
							onCommit={setHeights}
						/>
						<CommitText
							label={turned ? "Across (m)" : "Back (m)"}
							ariaLabel={turned ? "Positions across" : "Positions back"}
							value={positions}
							accepts={(draft) => parseMetreList(draft) != null}
							onCommit={setPositions}
						/>
						<p className="cad-thru-hint">
							One run per height and line: <kbd>5 7</kbd> for a list, <kbd>4 THRU 8 BY 2</kbd> for an
							evenly spaced run. Turned runs stand at the positions you give across the room instead
							of back into it.
						</p>
					</>
				)}
				<p aria-live="polite">
					{placements
						? `${count} ${count === 1 ? "element" : "elements"} will be placed.`
						: "Type the heights and positions as numbers in metres."}
				</p>
				<footer>
					<Button onClick={onClose}>Cancel</Button>
					<Button
						className="primary"
						disabled={placing || !count}
						onClick={() => placements && onPlace(placements)}
					>
						{placing ? "Placing…" : `Place ${count}`}
					</Button>
				</footer>
			</div>
		</ModalFrame>
	);
}
