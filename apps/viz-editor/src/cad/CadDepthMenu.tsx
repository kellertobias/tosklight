import { Button } from "@tosklight/ui";
import type { PointerEvent as ReactPointerEvent, ReactNode } from "react";
import { useRef } from "react";
import { type CutPlanes, cutPlanesAreOpen, depthExtent } from "./cutPlanes";
import {
	depthFraction,
	fractionDepth,
	previewBounds,
	previewLabel,
	previewMarks,
} from "./depthPreview";
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

/**
 * How much of the drawing one view shows, and the picture that makes it obvious.
 *
 * Seen from the side, a stage with curtains on both sides is a wall: the near one hides everything
 * behind it. Naming a nearest and a furthest depth cuts those away. The two numbers alone are hard
 * to aim, so the same slice is shown from the angle the view cannot see along — an elevation for a
 * plan, a plan for an elevation — with the cuts as two lines to drag across it.
 */
export function CadDepthMenu({
	view,
	entities,
	cutPlanes,
	onChange,
	onClose,
	children,
}: {
	view: CadViewDirection;
	entities: readonly CadEntity[];
	cutPlanes: CutPlanes | undefined;
	onChange(planes: CutPlanes): void;
	onClose(): void;
	children?: ReactNode;
}) {
	const surface = useRef<HTMLDivElement>(null);
	const extent = depthExtent(entities, view);
	const planes: CutPlanes = cutPlanes ?? {
		nearMillimetres: null,
		farMillimetres: null,
	};
	const open = cutPlanesAreOpen(cutPlanes);
	const axis = CAD_CUT_AXIS_LABELS[view];
	const marks = previewMarks(entities, view);
	const bounds = previewBounds(marks);
	// An unset cut sits at the edge of what the preview draws, so both lines are always grabbable
	// and an operator can pull a limit inward without first typing a number to create one.
	const near = planes.nearMillimetres ?? bounds.minDepth;
	const far = planes.farMillimetres ?? bounds.maxDepth;

	const dragTo = (event: ReactPointerEvent, edge: "near" | "far") => {
		const node = surface.current;
		if (!node) return;
		const box = node.getBoundingClientRect();
		if (box.width <= 0) return;
		const depth = Math.round(
			fractionDepth((event.clientX - box.left) / box.width, bounds),
		);
		// The two cuts cannot cross: a slice with its far edge in front of its near one shows
		// nothing, which reads as a broken drawing rather than as an empty selection.
		onChange(
			edge === "near"
				? { ...planes, nearMillimetres: Math.min(depth, far) }
				: { ...planes, farMillimetres: Math.max(depth, near) },
		);
	};

	return (
		<div className="cad-depth-menu" aria-label={`${axis} range`}>
			<div className="cad-depth-menu-head">
				<span>{axis}</span>
				<small>{previewLabel(view)}</small>
				<Button
					className="cad-depth-menu-close"
					aria-label="Close range settings"
					title="Close range settings"
					onClick={onClose}
				>
					×
				</Button>
			</div>
			<div className="cad-depth-preview" ref={surface}>
				<svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
					{marks.map((mark) => (
						<circle
							key={mark.id}
							cx={depthFraction(mark.depth, bounds) * 100}
							cy={
								100 -
								((mark.lateral - bounds.minLateral) /
									Math.max(bounds.maxLateral - bounds.minLateral, 1)) *
									100
							}
							r={1.6}
						/>
					))}
				</svg>
				{(["near", "far"] as const).map((edge) => (
					<div
						key={edge}
						className={`cad-depth-line is-${edge}`}
						role="slider"
						tabIndex={0}
						aria-label={`${axis} ${edge === "near" ? "from" : "to"}`}
						aria-valuemin={Math.round(bounds.minDepth)}
						aria-valuemax={Math.round(bounds.maxDepth)}
						aria-valuenow={Math.round(edge === "near" ? near : far)}
						style={{
							left: `${depthFraction(edge === "near" ? near : far, bounds) * 100}%`,
						}}
						onPointerDown={(event) => {
							event.preventDefault();
							event.currentTarget.setPointerCapture?.(event.pointerId);
							dragTo(event, edge);
						}}
						onPointerMove={(event) => {
							if (event.buttons === 0) return;
							dragTo(event, edge);
						}}
					/>
				))}
			</div>
			<div className="cad-depth-fields">
				{(["near", "far"] as const).map((edge) => (
					<label key={edge} className="cad-cut-plane-field">
						<span>{edge === "near" ? "From" : "To"}</span>
						<input
							type="number"
							step="0.1"
							inputMode="decimal"
							aria-label={`${axis} ${edge === "near" ? "from" : "to"}, in metres`}
							placeholder={
								extent
									? (Math.round(extent[edge] / 100) / 10).toFixed(1)
									: ""
							}
							value={metresOf(
								edge === "near" ? planes.nearMillimetres : planes.farMillimetres,
							)}
							onChange={(event) =>
								onChange({
									...planes,
									[edge === "near" ? "nearMillimetres" : "farMillimetres"]:
										millimetresOf(event.currentTarget.value),
								})
							}
						/>
					</label>
				))}
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
			{children ? <div className="cad-depth-extras">{children}</div> : null}
		</div>
	);
}
