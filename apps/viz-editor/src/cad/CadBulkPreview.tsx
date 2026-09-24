/**
 * The plan of what a Place Multiple will place, drawn in the modal before anything is: each stage
 * element's footprint, or each truss section as the bar it is, turned as it will be placed.
 */
import type { PlanPlacement } from "./bulkPlacement";

/** One element's box on the plan before it is turned, in metres: across its heading, then along it. */
export interface PreviewFootprint {
	length: number;
	width: number;
}

function corners(placement: PlanPlacement, footprint: PreviewFootprint): [number, number][] {
	const angle = (placement.rotation.z * Math.PI) / 180;
	const [cos, sin] = [Math.cos(angle), Math.sin(angle)];
	const [cx, cy] = [placement.position.x / 1000, placement.position.y / 1000];
	const [hl, hw] = [footprint.length / 2, footprint.width / 2];
	return (
		[
			[-hl, -hw],
			[hl, -hw],
			[hl, hw],
			[-hl, hw],
		] as const
	).map(([x, y]) => [cx + x * cos - y * sin, cy + x * sin + y * cos]);
}

export function CadBulkPreview({
	placements,
	footprint,
}: {
	placements: readonly PlanPlacement[];
	footprint: PreviewFootprint;
}) {
	const shapes = placements.map((placement) => corners(placement, footprint));
	const points = shapes.flat();
	if (!points.length) return null;
	const xs = points.map(([x]) => x);
	const ys = points.map(([, y]) => y);
	const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
	const pad = Math.max(maxX - minX, maxY - minY, 1) * 0.08;
	// The plan's Y runs up the page, so the preview flips it as the viewports do.
	const viewBox = [minX - pad, -maxY - pad, maxX - minX + 2 * pad, maxY - minY + 2 * pad].join(" ");
	return (
		<svg
			className="cad-bulk-preview"
			role="img"
			aria-label={`Plan of the ${placements.length} elements to be placed`}
			viewBox={viewBox}
			preserveAspectRatio="xMidYMid meet"
		>
			{shapes.map((shape, index) => (
				<polygon
					// biome-ignore lint/suspicious/noArrayIndexKey: the placements are in a fixed order.
					key={index}
					data-testid="cad-bulk-preview-element"
					points={shape.map(([x, y]) => `${x},${-y}`).join(" ")}
					vectorEffect="non-scaling-stroke"
				/>
			))}
		</svg>
	);
}
