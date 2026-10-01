import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import type { ValueRange } from "./RangeControls";

type ColorPoint = { hue: number; saturation: number };
const clamp = (value: number, max: number) => Math.max(0, Math.min(max, value));

/** The compact hue/saturation sheet shares the modal's color and ordered endpoints. */
export function ColorPlane({ hue, saturation, hueRange, saturationRange, preview, shiftArmed, onChange }: {
	hue: number; saturation: number; hueRange?: ValueRange; saturationRange?: ValueRange; preview: string; shiftArmed?: boolean;
	onChange(hue: number, saturation: number, hueRange?: ValueRange, saturationRange?: ValueRange): void;
}) {
	const first = useRef<ColorPoint | null>(hueRange || saturationRange ? { hue: hueRange?.[0] ?? hue, saturation: saturationRange?.[0] ?? saturation } : null);
	const drag = useRef<{ shifted: boolean; start: ColorPoint; x: number; y: number } | null>(null);
	const [pending, setPending] = useState(false);
	const point = (event: PointerEvent<HTMLDivElement>) => {
		const rect = event.currentTarget.getBoundingClientRect();
		return { hue: Math.round(clamp((event.clientX - rect.left) / rect.width * 359, 359)), saturation: Math.round(clamp((1 - (event.clientY - rect.top) / rect.height) * 100, 100)) };
	};
	const choose = (next: ColorPoint, shifted: boolean) => {
		if (shifted && first.current) { onChange(first.current.hue, first.current.saturation, [first.current.hue, next.hue], [first.current.saturation, next.saturation]); setPending(false); }
		else { first.current = next; onChange(next.hue, next.saturation); setPending(shifted); }
	};
	const key = (event: KeyboardEvent<HTMLDivElement>) => {
		if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
		event.preventDefault();
		const shifted = event.shiftKey || !!shiftArmed;
		const current = { hue: shifted ? hueRange?.[1] ?? hue : hue, saturation: shifted ? saturationRange?.[1] ?? saturation : saturation };
		if (shifted && !first.current) first.current = { hue, saturation };
		choose({ hue: clamp(current.hue + (event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0), 359), saturation: clamp(current.saturation + (event.key === "ArrowUp" ? 1 : event.key === "ArrowDown" ? -1 : 0), 100) }, shifted);
	};
	const ranged = !!(hueRange || saturationRange);
	const handles = ranged ? [{ hue: hueRange?.[0] ?? hue, saturation: saturationRange?.[0] ?? saturation }, { hue: hueRange?.[1] ?? hue, saturation: saturationRange?.[1] ?? saturation }] : [{ hue, saturation }];
	return <div className="fam-color-plane">
		<div className="color-sheet fam-2d-sheet" role="application" aria-label="Color picker" aria-description="Hue left to right, saturation bottom to top. Use arrow keys to adjust; hold Shift for range endpoints." tabIndex={0} data-testid="color-picker" data-hue={hue} data-saturation={saturation}
			onKeyDown={key} onPointerDown={event => { event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId); const next = point(event), shifted = event.shiftKey || !!shiftArmed; choose(next, shifted); drag.current = { shifted, start: first.current ?? next, x: event.clientX, y: event.clientY }; }}
			onPointerMove={event => { const gesture = drag.current; if (!gesture || !event.currentTarget.hasPointerCapture(event.pointerId) || Math.hypot(event.clientX - gesture.x, event.clientY - gesture.y) < 3) return; const next = point(event); if (gesture.shifted) { onChange(gesture.start.hue, gesture.start.saturation, [gesture.start.hue, next.hue], [gesture.start.saturation, next.saturation]); setPending(false); } else choose(next, false); }}
			onPointerUp={event => { drag.current = null; if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId); }} onPointerCancel={() => { drag.current = null; }}>
			{handles.map((value, index) => <i key={index} className="fam-plane-marker" style={{ left: `clamp(10px, ${value.hue / 359 * 100}%, calc(100% - 10px))`, top: `clamp(10px, ${100 - value.saturation}%, calc(100% - 10px))` }}>{ranged ? index + 1 : pending ? "1" : ""}</i>)}
		</div>
		<div className="fam-2d-caption"><span data-testid="color-preview" style={{ background: preview }} /><span>{pending ? "Shift-click the last color" : "Hue ↔ · Saturation ↕ · Shift for range"}</span></div>
	</div>;
}
