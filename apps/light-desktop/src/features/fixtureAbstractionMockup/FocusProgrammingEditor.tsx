import { ModalFrame } from "@tosklight/ui";
import { useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import "./positionFocusEditors.css";

export interface FocusProgrammingEditorProps {
	fits: boolean;
	zoom: number;
	focus: number;
	onZoom(value: number): void;
	onFocus(value: number): void;
	onClose(): void;
}

const bounded = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));

function BeamEditor({ zoom, focus, onZoom, onFocus }: Omit<FocusProgrammingEditorProps, "fits" | "onClose">) {
	const svg = useRef<SVGSVGElement>(null);
	const viewport = useRef<HTMLDivElement>(null);
	const [size, setSize] = useState({ width: 640, height: 180 });
	const active = useRef<{
		kind: "angle" | "focus"; pointerId: number; x: number; y: number;
		focus: number; opening: number; side: number; fraction: number; length: number; maximum: number;
	} | null>(null);
	useLayoutEffect(() => {
		const element = viewport.current;
		if (!element) return;
		const observer = new ResizeObserver(([entry]) => {
			const { width, height } = entry.contentRect;
			if (!width || !height) return;
			setSize(current => Math.abs(current.width - width) < .1 && Math.abs(current.height - height) < .1 ? current : { width, height });
		});
		observer.observe(element);
		return () => observer.disconnect();
	}, []);
	const left = 32;
	const right = Math.max(left + 60, size.width - 32);
	const length = right - left;
	const center = size.height / 2;
	const maximum = Math.max(8, center - 28);
	const tangentAtMaximum = Math.tan(24 * Math.PI / 180);
	// This is an angle/focus control schematic. Focus is a normalized setting, not metres.
	const halfOpening = maximum * Math.tan(zoom / 2 * Math.PI / 180) / tangentAtMaximum;
	const planeX = left + focus / 100 * length;
	const planeHalfHeight = halfOpening * focus / 100;
	// Keep the two end handles independently touchable even for a very narrow beam.
	const handleOffset = Math.max(26, halfOpening);
	const edgeStartX = left + length * .2;
	const edgeStartHeight = halfOpening * .2;
	const position = (event: PointerEvent<SVGGElement>) => {
		const matrix = svg.current?.getScreenCTM();
		if (!matrix) return null;
		return new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
	};
	const move = (event: PointerEvent<SVGGElement>) => {
		const point = position(event);
		const gesture = active.current;
		if (!point || !gesture || gesture.pointerId !== event.pointerId) return;
		if (gesture.kind === "focus") onFocus(Math.round(bounded(gesture.focus + (point.x - gesture.x) / gesture.length * 100, 0, 100)));
		else {
			// Retain the edge, local beam fraction and initial grab offset for the full gesture.
			const opening = Math.max(0, gesture.opening + gesture.side * (point.y - gesture.y) / gesture.fraction);
			const angle = 2 * Math.atan(opening / gesture.maximum * tangentAtMaximum) * 180 / Math.PI;
			onZoom(Math.round(bounded(angle, 8, 48) * 10) / 10);
		}
	};
	const down = (kind: "angle" | "focus", event: PointerEvent<SVGGElement>) => {
		if (active.current) return;
		const point = position(event);
		if (!point) return;
		event.preventDefault();
		const side = Number((event.target as Element).closest("[data-beam-side]")?.getAttribute("data-beam-side")) || (point.y < center ? -1 : 1);
		const handle = (event.target as Element).closest("[data-angle-handle]");
		active.current = { kind, pointerId: event.pointerId, x: point.x, y: point.y, focus, opening: halfOpening, side,
			fraction: handle ? 1 : bounded((point.x - left) / length, .2, 1), length, maximum };
		event.currentTarget.setPointerCapture(event.pointerId);
	};
	const up = (event: PointerEvent<SVGGElement>) => {
		if (active.current?.pointerId !== event.pointerId) return;
		move(event); active.current = null; event.currentTarget.releasePointerCapture(event.pointerId);
	};
	const cancel = (event: PointerEvent<SVGGElement>) => { if (active.current?.pointerId === event.pointerId) active.current = null; };
	const keys = (kind: "angle" | "focus", event: KeyboardEvent<SVGGElement>) => {
		if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
		event.preventDefault();
		const positive = event.key === "ArrowRight" || event.key === "ArrowUp";
		const value = kind === "angle" ? zoom : focus;
		const low = kind === "angle" ? 8 : 0;
		const high = kind === "angle" ? 48 : 100;
		const next = event.key === "Home" ? low : event.key === "End" ? high : bounded(value + (positive ? 1 : -1) * (event.shiftKey ? 5 : 1), low, high);
		if (kind === "angle") onZoom(next); else onFocus(next);
	};
	return <div className="fam-beam-editor"><div ref={viewport} className="fam-beam-viewport"><svg ref={svg} viewBox={`0 0 ${size.width} ${size.height}`} preserveAspectRatio="none" aria-label="Beam angle and focus diagram" role="group">
		<path d={`M${left} ${center} L${right} ${center - halfOpening} L${right} ${center + halfOpening} Z`} className="fam-beam-cone" />
		<line x1={left} y1={center} x2={right} y2={center} className="fam-beam-centerline" />
		<g className="fam-beam-edge-gestures" onPointerDown={event => down("angle", event)} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel}>
			{[-1, 1].map(side => <path key={side} data-beam-side={side} d={`M${edgeStartX} ${center + side * edgeStartHeight} L${right} ${center + side * halfOpening}`} className="fam-beam-angle-hit" />)}
			<path d={`M${left} ${center} L${right} ${center - halfOpening} M${left} ${center} L${right} ${center + halfOpening}`} className="fam-beam-edge" />
		</g>
		<g role="slider" tabIndex={0} aria-label="Focus position" aria-valuemin={0} aria-valuemax={100} aria-valuenow={focus} aria-valuetext={`${focus}%`}
			onPointerDown={event => down("focus", event)} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel} onKeyDown={event => keys("focus", event)} data-testid="focus-position-drag">
			<rect x={planeX - 22} y={center - Math.max(22, planeHalfHeight + 10)} width="44" height={Math.max(44, planeHalfHeight * 2 + 20)} className="fam-focus-hit" />
			<line x1={planeX} y1={center - planeHalfHeight - 10} x2={planeX} y2={center + planeHalfHeight + 10} className="fam-focus-plane" />
			<rect x={planeX - 13} y={center - 11} width="26" height="22" rx="4" className="fam-focus-handle" />
		</g>
		<g role="slider" tabIndex={0} aria-label="Beam opening angle" aria-valuemin={8} aria-valuemax={48} aria-valuenow={zoom} aria-valuetext={`${zoom.toFixed(1)} degrees`}
			onPointerDown={event => down("angle", event)} onPointerMove={move} onPointerUp={up} onPointerCancel={cancel} onKeyDown={event => keys("angle", event)} data-testid="beam-angle-drag">
			{[-1, 1].map(side => <g key={side} data-beam-side={side} data-angle-handle>
				<line x1={right} y1={center + side * halfOpening} x2={right} y2={center + side * handleOffset} className="fam-beam-handle-guide" />
				<circle cx={right} cy={center + side * handleOffset} r="22" className="fam-angle-handle-hit" />
				<circle cx={right} cy={center + side * handleOffset} r="9" className="fam-guide-handle" />
			</g>)}
		</g>
		<text x={left} y={size.height - 6} className="fam-guide-label">Near · 0%</text><text x={right - 24} y={size.height - 6} textAnchor="end" className="fam-guide-label">Far · 100%</text>
	</svg></div><div className="fam-beam-readouts"><span>Beam <strong>{zoom.toFixed(1)}°</strong></span><span>Focus <strong>{focus}%</strong></span></div></div>;
}

export function FocusProgrammingEditor({ zoom, focus, onZoom, onFocus, onClose }: FocusProgrammingEditorProps) {
	return <ModalFrame title="Focus" ariaLabel="Focus Special Dialog" closeLabel="Close Focus Special Dialog" onClose={onClose}
		dialogClassName="fixture-abstraction-panel fam-spatial-editor fam-focus-modal" className="fixture-abstraction-layer fam-spatial-layer">
		<div className="fam-focus-editor-body" data-testid="editor-page"><BeamEditor zoom={zoom} focus={focus} onZoom={onZoom} onFocus={onFocus} /></div>
	</ModalFrame>;
}
