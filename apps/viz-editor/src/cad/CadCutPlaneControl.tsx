import { Button } from "@tosklight/ui";
import {
	type CutPlanes,
	type DepthRange,
	cutPlanesAreOpen,
	depthExtent,
} from "./cutPlanes";
import type { CadEntity, CadViewDirection } from "./types";

const CAD_CUT_AXIS_LABELS: Record<CadViewDirection, string> = {
	top_down: "Height",
	left_to_right: "Depth",
	right_to_left: "Depth",
	front_to_back: "Depth",
	back_to_front: "Depth",
};

function metresOf(millimetres: number | null): string {
	return millimetres === null ? "" : String(millimetres / 1_000);
}

function millimetresOf(metres: string): number | null {
	const trimmed = metres.trim();
	if (!trimmed) return null;
	const value = Number(trimmed);
	return Number.isFinite(value) ? Math.round(value * 1_000) : null;
}

function placeholderOf(extent: DepthRange | null, edge: "near" | "far") {
	if (!extent) return "";
	return (Math.round(extent[edge] / 100) / 10).toFixed(1);
}

/**
 * Which slice of the drawing one view shows.
 *
 * Seen from the side, a stage with curtains on both sides is a wall: the near one hides everything
 * behind it. Naming a nearest and a furthest depth cuts those away and leaves the part of the rig
 * the drawing is actually about.
 */
export function CadCutPlaneControl({
	view,
	entities,
	cutPlanes,
	onChange,
}: {
	view: CadViewDirection;
	entities: readonly CadEntity[];
	cutPlanes: CutPlanes | undefined;
	onChange(planes: CutPlanes): void;
}) {
	const extent = depthExtent(entities, view);
	const planes: CutPlanes = cutPlanes ?? {
		nearMillimetres: null,
		farMillimetres: null,
	};
	const open = cutPlanesAreOpen(cutPlanes);
	const axis = CAD_CUT_AXIS_LABELS[view];
	return (
		<div className="cad-cut-plane-control" data-open={open ? "true" : "false"}>
			<span className="cad-cut-plane-axis">{axis}</span>
			<label className="cad-cut-plane-field">
				<span>From</span>
				<input
					type="number"
					step="0.1"
					inputMode="decimal"
					aria-label={`${axis} from, in metres`}
					placeholder={placeholderOf(extent, "near")}
					value={metresOf(planes.nearMillimetres)}
					onChange={(event) =>
						onChange({
							...planes,
							nearMillimetres: millimetresOf(event.currentTarget.value),
						})
					}
				/>
			</label>
			<label className="cad-cut-plane-field">
				<span>To</span>
				<input
					type="number"
					step="0.1"
					inputMode="decimal"
					aria-label={`${axis} to, in metres`}
					placeholder={placeholderOf(extent, "far")}
					value={metresOf(planes.farMillimetres)}
					onChange={(event) =>
						onChange({
							...planes,
							farMillimetres: millimetresOf(event.currentTarget.value),
						})
					}
				/>
			</label>
			<Button
				className="cad-cut-plane-reset"
				disabled={open}
				title="Show the whole drawing again"
				onClick={() =>
					onChange({ nearMillimetres: null, farMillimetres: null })
				}
			>
				Show all
			</Button>
		</div>
	);
}
