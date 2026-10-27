import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { LABEL_CAPACITY_MESSAGE, labelWidth, placeLabels, type LabelRequest } from "./labelPlacement";
import { fixtureLabelLines } from "./printFixtureLabels";
/**
 * The DOM drawn over a CAD viewport's canvas: the scale bar an operator reads a distance off, and
 * the fixture identities beside the rig.
 *
 * Both follow the same camera the canvas does, in CSS rather than in the renderer, so text stays
 * crisp at any zoom and is selectable by a screen reader rather than baked into pixels.
 */
import type { CadViewportScale } from "./CadViewport";
import type { MoveReadout } from "./moveEntry";
import type {
	CadEntity,
	CadTransformPreview,
	CadViewDirection,
	TileCamera,
} from "./types";
import { previewDeltaForEntity, projectPoint } from "./types";

/** The measured rule in the viewport corner. */
export function CadScaleBar({
	scale,
	printMode,
}: {
	scale: CadViewportScale;
	printMode: boolean;
}) {
	return (
		<div
			className={`cad-viewport-scale ${printMode ? "is-print-mode" : ""}`.trim()}
			aria-label={`Scale ${scale.label}`}
			style={{ width: `${scale.pixelWidth}px` }}
		>
			<span className="cad-viewport-scale-label">{scale.label}</span>
			<span className="cad-viewport-scale-rule" aria-hidden="true">
				<span className="cad-viewport-scale-tick is-start" />
				<span className="cad-viewport-scale-tick is-quarter" />
				<span className="cad-viewport-scale-tick is-middle" />
				<span className="cad-viewport-scale-tick is-three-quarter" />
				<span className="cad-viewport-scale-tick is-end" />
			</span>
		</div>
	);
}

function useLabelViewport(enabled: boolean) {
	const ref = useRef<HTMLDivElement>(null);
	const [size, setSize] = useState({ width: 1000, height: 800 });
	useLayoutEffect(() => {
		const element = ref.current;
		if (!element || !enabled) return;
		const resize = () => {
			const bounds = element.getBoundingClientRect();
			if (bounds.width > 0 && bounds.height > 0) setSize({ width: bounds.width, height: bounds.height });
		};
		resize();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(resize);
		observer.observe(element);
		return () => observer.disconnect();
	}, [enabled]);
	return { ref, ...size };
}

/** Complete fixture instance labels share the indexed collision solver used by printed plans. */
export function CadEntityLabels({ entities, preview, view, rotationQuarterTurns, camera, showFixtureIds, showDmxAddresses }: {
	entities: readonly CadEntity[];
	preview: CadTransformPreview | null;
	view: CadViewDirection;
	rotationQuarterTurns: number;
	camera: TileCamera;
	showFixtureIds: boolean;
	showDmxAddresses: boolean;
}) {
	const enabled = showFixtureIds || showDmxAddresses;
	const viewport = useLabelViewport(enabled);
	const bounds = { x: -viewport.width / 2 + 4, y: -viewport.height / 2 + 4, width: viewport.width - 8, height: viewport.height - 8 };
	const layout = useMemo(() => {
		const requests: LabelRequest[] = [];
		for (const entity of entities) {
			const delta = previewDeltaForEntity(preview, entity.logicalFixtureId);
			const point = projectPoint(entity.positionMillimetres.map((value, index) => value + delta[index]) as [number, number, number], view, rotationQuarterTurns);
			const anchor: [number, number] = [(point[0] + camera.pan[0]) * camera.zoom, -(point[1] + camera.pan[1]) * camera.zoom];
			if (anchor[0] < bounds.x || anchor[0] > bounds.x + bounds.width || anchor[1] < bounds.y || anchor[1] > bounds.y + bounds.height) continue;
			const lines = [fixtureLabelLines(entity, showFixtureIds, showDmxAddresses).join(" · ")].filter(Boolean);
			if (showFixtureIds && entity.positionReference) lines.push(`Follows 3D Point ${entity.positionReference}`);
			if (lines.length) requests.push({ key: entity.id, anchor, lines, width: labelWidth(lines, 10, 5), height: lines.length * 12 + 6 });
		}
		return placeLabels(requests, bounds, 3);
	}, [entities, preview, view, rotationQuarterTurns, camera, showFixtureIds, showDmxAddresses, viewport.width, viewport.height]);
	if (!enabled) return null;
	return <div ref={viewport.ref} className="cad-entity-labels">
		<svg aria-hidden="true" width="100%" height="100%" viewBox={`${-viewport.width / 2} ${-viewport.height / 2} ${viewport.width} ${viewport.height}`} style={{ position: "absolute", inset: 0 }}>
			{layout.labels.map((label) => <line key={label.key} x1={label.leader[0][0]} y1={label.leader[0][1]} x2={label.leader[1][0]} y2={label.leader[1][1]} stroke="#8797a5" strokeWidth={0.7} />)}
		</svg>
		{layout.labels.map((label) => <span key={label.key} className="cad-entity-label" data-fixture-instance={label.key}
			style={{ left: `calc(50% + ${label.anchor[0]}px)`, top: `calc(50% + ${label.anchor[1]}px)`, translate: "none", transform: `translate(${label.box.x - label.anchor[0]}px, ${label.box.y - label.anchor[1]}px)`, boxSizing: "border-box", width: label.box.width, height: label.box.height }}>
			{label.lines[0]}{label.lines.slice(1).map((line, index) => <span key={index} style={{ display: "block", lineHeight: "12px" }} className="cad-entity-label-reference">{line}</span>)}
		</span>)}
		{layout.unplaced.length ? <div role="status" className="cad-label-capacity-warning">{layout.unplaced.length} labels do not fit. {LABEL_CAPACITY_MESSAGE}</div> : null}
	</div>;
}

/**
 * The live position beside the move gizmo while a move is in flight, and what the operator has
 * typed: a number sets the active axis, +/- moves along it, Enter commits, Tab switches axis on a
 * free drag, Escape clears the entry and then abandons the move.
 */
export function CadMoveReadout({
	readout,
	camera,
}: {
	readout: MoveReadout | null;
	camera: TileCamera;
}) {
	if (!readout) return null;
	const [x, y] = readout.anchor;
	return (
		<div
			className="cad-move-readout"
			role="status"
			aria-label={readout.title ?? "Move position"}
			style={{
				left: `calc(50% + ${(x + camera.pan[0]) * camera.zoom}px)`,
				top: `calc(50% - ${(y + camera.pan[1]) * camera.zoom}px)`,
			}}
		>
			{readout.coordinates.map((coordinate) => (
				<span
					key={coordinate.axis}
					className="cad-move-readout-axis"
					data-active={coordinate.active ? "true" : undefined}
				>
					{coordinate.label} {coordinate.value}
				</span>
			))}
			{readout.entry ? (
				<span
					className="cad-move-readout-entry"
					data-invalid={readout.invalid ? "true" : undefined}
				>
					{readout.coordinates.find((coordinate) => coordinate.active)?.label}{" "}
					{readout.entry}
					{readout.invalid ? " — not a number yet" : " — Enter to move"}
				</span>
			) : null}
		</div>
	);
}

/** While Add Several holds an element: what a press places, and the way to stop. */
export function CadPlacingBanner({ name, onDone }: { name: string; onDone(): void }) {
	return (
		<div className="cad-placing-banner" role="status" aria-label="Adding several">
			<span>
				Adding several <strong>{name}</strong> — click to place a copy, Esc to finish
			</span>
			<button type="button" onClick={onDone}>
				Done
			</button>
		</div>
	);
}
