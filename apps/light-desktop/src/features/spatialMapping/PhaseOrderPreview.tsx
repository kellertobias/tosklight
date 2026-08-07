import type { ProjectedSpatialPosition, SpatialRank } from "./contracts";

/**
 * The fixtures on the plane the projection ranks them in, shaded by the order the Phase shape
 * puts them in: first is black, last is white.
 *
 * This is the same `(u, v)` the engine ranks on, so a shape that does the wrong thing shows it
 * here as a gradient running the wrong way, which a list of ranks never makes obvious.
 *
 * Unlike the projection preview this one is flat on purpose. The ranking plane is what it is;
 * there is no viewpoint to convey.
 */

const SIZE = { width: 320, height: 260 };
const PADDING = 18;
const DOT_RADIUS = 5.5;

interface PlottedFixture {
	fixtureId: string;
	x: number;
	y: number;
	shade: number;
	rank: number | null;
}

/** A single fixture, or fixtures sharing one coordinate, still need a plane to sit in. */
function span(values: number[]) {
	const low = Math.min(...values);
	const high = Math.max(...values);
	return high - low <= 1e-9 ? { low: low - 0.5, size: 1 } : { low, size: high - low };
}

function plot(
	positions: readonly ProjectedSpatialPosition[],
	ranks: readonly SpatialRank[],
	rankCount: number,
): PlottedFixture[] {
	const placed = positions.filter(
		(position): position is ProjectedSpatialPosition & { u: number; v: number } =>
			position.u != null && position.v != null,
	);
	if (!placed.length) return [];
	const horizontal = span(placed.map((position) => position.u));
	const vertical = span(placed.map((position) => position.v));
	const last = Math.max(1, rankCount - 1);
	return placed.map((position) => {
		const rank =
			ranks.find((candidate) => candidate.fixture_id === position.fixture_id)
				?.rank ?? null;
		return {
			fixtureId: position.fixture_id,
			x:
				PADDING +
				((position.u - horizontal.low) / horizontal.size) *
					(SIZE.width - PADDING * 2),
			// `v` grows upward on the ranking plane, so the plot flips it to match.
			y:
				SIZE.height -
				PADDING -
				((position.v - vertical.low) / vertical.size) *
					(SIZE.height - PADDING * 2),
			shade: rank == null ? 0.5 : rank / last,
			rank,
		};
	});
}

export function PhaseOrderPreview({
	positions,
	ranks,
	rankCount,
	unplaced,
}: {
	positions: readonly ProjectedSpatialPosition[];
	ranks: readonly SpatialRank[];
	rankCount: number;
	/** Fixtures with no Stage position, which rank after everything that has one. */
	unplaced?: number;
}) {
	const fixtures = plot(positions, ranks, rankCount);
	const label = "Phase order across the projected plane, first black to last white";

	if (!fixtures.length)
		return (
			<p className="phase-order-empty">
				No fixture in this Group has a Stage position to order.
			</p>
		);

	return (
		<figure className="phase-order-preview">
			<svg
				viewBox={`0 0 ${SIZE.width} ${SIZE.height}`}
				role="img"
				aria-label={label}
			>
				<title>{label}</title>
				<rect
					className="phase-order-field"
					x={0}
					y={0}
					width={SIZE.width}
					height={SIZE.height}
				/>
				{fixtures.map((fixture) => (
					<circle
						key={fixture.fixtureId}
						className="phase-order-fixture"
						cx={fixture.x}
						cy={fixture.y}
						r={DOT_RADIUS}
						fill={`hsl(0 0% ${(fixture.shade * 100).toFixed(1)}%)`}
					>
						<title>
							{fixture.rank == null
								? `${fixture.fixtureId}: no rank`
								: `${fixture.fixtureId}: rank ${fixture.rank + 1} of ${rankCount}`}
						</title>
					</circle>
				))}
			</svg>
			<figcaption>
				First <span className="phase-order-swatch phase-order-swatch-first" /> to
				last <span className="phase-order-swatch phase-order-swatch-last" /> ·{" "}
				{rankCount} {rankCount === 1 ? "rank" : "ranks"}
				{unplaced ? ` · ${unplaced} without a Stage position` : ""}
			</figcaption>
		</figure>
	);
}
