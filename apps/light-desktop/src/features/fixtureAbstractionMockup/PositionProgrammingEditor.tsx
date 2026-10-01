import { Button, ModalFrame } from "@tosklight/ui";
import { forwardRef, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { RangeFader } from "./RangeControls";
import "./positionModal.css";

export interface PositionProgrammingEditorProps {
	fits: boolean;
	pan: number;
	tilt: number;
	onAngles(pan: number, tilt: number): void;
	onGestureEnd?(): void;
	onClose(): void;
}
const bounded = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value));
const PAN_MIN = -720, PAN_MAX = 720, TILT_MIN = -135, TILT_MAX = 135;
type StopHandle = { stop(): void };

const PanCircle = forwardRef<StopHandle, { value: number; onChange(value: number): void; onGestureEnd?(): void }>(function PanCircle({ value, onChange, onGestureEnd }, ref) {
	const latest = useRef({ value, onChange, onGestureEnd });
	latest.current = { value, onChange, onGestureEnd };
	const gesture = useRef<{ pointerId: number; element: SVGSVGElement; angle: number; value: number; changed: boolean } | null>(null);
	const finish = () => {
		const previous = gesture.current;
		if (!previous) return;
		gesture.current = null;
		if (previous.element.hasPointerCapture(previous.pointerId)) previous.element.releasePointerCapture(previous.pointerId);
		if (previous.changed) latest.current.onGestureEnd?.();
	};
	useImperativeHandle(ref, () => ({ stop: finish }));
	useEffect(() => {
		const hide = () => { if (document.hidden) finish(); };
		window.addEventListener("blur", finish); document.addEventListener("visibilitychange", hide);
		return () => { window.removeEventListener("blur", finish); document.removeEventListener("visibilitychange", hide); gesture.current = null; };
	}, []);
	const pointerAngle = (event: PointerEvent<SVGSVGElement>) => {
		const box = event.currentTarget.getBoundingClientRect();
		const x = event.clientX - box.left - box.width / 2, y = event.clientY - box.top - box.height / 2;
		return Math.hypot(x, y) < 18 ? null : Math.atan2(x, -y) * 180 / Math.PI;
	};
	const move = (event: PointerEvent<SVGSVGElement>) => {
		const current = gesture.current, next = pointerAngle(event);
		if (!current || current.pointerId !== event.pointerId || next === null) return;
		const delta = (next - current.angle + 540) % 360 - 180;
		const updated = Math.round(bounded(current.value + delta, PAN_MIN, PAN_MAX) * 10) / 10;
		current.angle = next;
		if (updated === current.value) return;
		current.value = updated; current.changed = true; latest.current.onChange(updated);
	};
	const angle = value * Math.PI / 180;
	const set = (next: number) => {
		const nextValue = bounded(next, PAN_MIN, PAN_MAX);
		if (nextValue === latest.current.value) return;
		latest.current.onChange(nextValue); latest.current.onGestureEnd?.();
	};
	return <div className="fam-position-pan">
		<svg viewBox="0 0 220 220" role="slider" tabIndex={0} aria-label="Pan circle" aria-valuemin={PAN_MIN} aria-valuemax={PAN_MAX} aria-valuenow={value}
			aria-valuetext={`${value.toFixed(1)} degrees, ${(value / 360).toFixed(2)} turns`} data-testid="pan-circle" onKeyDown={event => {
				if (!["ArrowLeft", "ArrowRight", "Home", "PageUp", "PageDown"].includes(event.key)) return;
				event.preventDefault(); set(event.key === "Home" ? 0 : value + (event.key === "PageUp" ? 360 : event.key === "PageDown" ? -360 : event.key === "ArrowRight" ? 1 : -1));
			}}
			onPointerDown={event => {
				if (gesture.current) return;
				const start = pointerAngle(event); if (start === null) return;
				event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId);
				gesture.current = { pointerId: event.pointerId, element: event.currentTarget, angle: start, value, changed: false };
			}}
			onPointerMove={move} onPointerUp={event => { if (gesture.current?.pointerId !== event.pointerId) return; move(event); finish(); }}
			onPointerCancel={event => { if (gesture.current?.pointerId === event.pointerId) finish(); }}
			onLostPointerCapture={event => { if (gesture.current?.pointerId === event.pointerId) finish(); }} onBlur={finish}>
			<circle cx="110" cy="110" r="81" className="fam-position-pan-ring" />
			{Array.from({ length: 24 }, (_, i) => <line key={i} x1="110" y1="20" x2="110" y2={i % 6 ? "25" : "32"} transform={`rotate(${i * 15} 110 110)`} className="fam-position-pan-tick" />)}
			<text x="110" y="13" textAnchor="middle" className="fam-position-pan-zero">0°</text>
			<line x1="110" y1="110" x2={110 + Math.sin(angle) * 76} y2={110 - Math.cos(angle) * 76} className="fam-position-pan-direction" />
			<circle cx={110 + Math.sin(angle) * 81} cy={110 - Math.cos(angle) * 81} r="10" className="fam-position-pan-handle" />
			<circle cx="110" cy="110" r="47" className="fam-position-pan-center" />
			<text x="110" y="101" textAnchor="middle" className="fam-position-pan-label">Pan</text>
			<text x="110" y="124" textAnchor="middle" className="fam-position-pan-value">{value.toFixed(1)}°</text>
		</svg>
		<div className="fam-position-pan-actions"><Button aria-label="Decrease pan by 90 degrees" disabled={value <= PAN_MIN} onClick={() => set(value - 90)}>−90°</Button>
			<Button aria-label="Reset pan to zero" onClick={() => set(0)}>Reset</Button>
			<Button aria-label="Increase pan by 90 degrees" disabled={value >= PAN_MAX} onClick={() => set(value + 90)}>+90°</Button></div>
		<output className="fam-position-turns" aria-label="Pan turns">{value > 0 ? "+" : ""}{(value / 360).toFixed(2)} turns</output>
	</div>;
});

type JoystickGesture = { pointerId: number | null; element: HTMLDivElement; changed: boolean };
type JoystickVector = { x: number; y: number };
const neutral: JoystickVector = { x: 0, y: 0 };
const response = (value: number) => Math.abs(value) <= .08 ? 0 : Math.sign(value) * ((Math.abs(value) - .08) / .92) ** 2;

const AimJoystick = forwardRef<StopHandle, Pick<PositionProgrammingEditorProps, "pan" | "tilt" | "onAngles" | "onGestureEnd">>(function AimJoystick({ pan, tilt, onAngles, onGestureEnd }, ref) {
	const callbacks = useRef({ onAngles, onGestureEnd });
	callbacks.current = { onAngles, onGestureEnd };
	const angles = useRef({ pan, tilt });
	useLayoutEffect(() => { angles.current = { pan, tilt }; }, [pan, tilt]);
	const gesture = useRef<JoystickGesture | null>(null), velocity = useRef(neutral);
	const frame = useRef<number | null>(null), lastTime = useRef<number | null>(null);
	const keys = useRef(new Set<string>());
	const [marker, setMarker] = useState(neutral), [held, setHeld] = useState(false);
	const stopFrames = () => {
		if (frame.current !== null) cancelAnimationFrame(frame.current);
		frame.current = null; lastTime.current = null;
	};
	const finish = () => {
		const previous = gesture.current;
		gesture.current = null; keys.current.clear(); velocity.current = neutral;
		stopFrames(); setMarker(neutral); setHeld(false);
		if (!previous) return;
		if (previous.pointerId !== null && previous.element.hasPointerCapture(previous.pointerId)) previous.element.releasePointerCapture(previous.pointerId);
		if (previous.changed) callbacks.current.onGestureEnd?.();
	};
	useImperativeHandle(ref, () => ({ stop: finish }));
	useEffect(() => {
		const hide = () => { if (document.hidden) finish(); };
		window.addEventListener("blur", finish); document.addEventListener("visibilitychange", hide);
		return () => {
			window.removeEventListener("blur", finish); document.removeEventListener("visibilitychange", hide);
			gesture.current = null; velocity.current = neutral; keys.current.clear(); stopFrames();
		};
	}, []);
	const tick = (now: number) => {
		frame.current = null;
		const current = gesture.current;
		if (!current) return;
		const seconds = Math.min(.05, Math.max(0, now - (lastTime.current ?? now)) / 1000);
		lastTime.current = now;
		const next = { pan: bounded(angles.current.pan + velocity.current.x * seconds, PAN_MIN, PAN_MAX),
			tilt: bounded(angles.current.tilt + velocity.current.y * seconds, TILT_MIN, TILT_MAX) };
		const changed = next.pan !== angles.current.pan || next.tilt !== angles.current.tilt;
		if (changed) { angles.current = next; current.changed = true; callbacks.current.onAngles(next.pan, next.tilt); }
		// No idle loop at a hard stop; changing displacement restarts motion.
		if ((velocity.current.x || velocity.current.y) && (changed || seconds === 0)) frame.current = requestAnimationFrame(tick);
	};
	const setVector = (vector: JoystickVector) => {
		const radius = Math.max(1, Math.hypot(vector.x, vector.y));
		const normalized = { x: vector.x / radius, y: vector.y / radius };
		const x = response(normalized.x), y = response(normalized.y);
		velocity.current = { x: x * 120, y: -y * 90 }; setMarker(x || y ? normalized : neutral);
		if (!x && !y) { stopFrames(); return; }
		if (gesture.current && frame.current === null) { lastTime.current = performance.now(); frame.current = requestAnimationFrame(tick); }
	};
	const point = (event: PointerEvent<HTMLDivElement>) => {
		const rect = event.currentTarget.getBoundingClientRect();
		return { x: bounded((event.clientX - rect.left - rect.width / 2) / Math.max(1, rect.width / 2 - 26), -1, 1),
			y: bounded((event.clientY - rect.top - rect.height / 2) / Math.max(1, rect.height / 2 - 26), -1, 1) };
	};
	const keyboardVector = () => setVector({ x: (keys.current.has("ArrowRight") ? 1 : 0) - (keys.current.has("ArrowLeft") ? 1 : 0),
		y: (keys.current.has("ArrowDown") ? 1 : 0) - (keys.current.has("ArrowUp") ? 1 : 0) });
	const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
		event.preventDefault();
		if (gesture.current?.pointerId != null) return;
		if (!gesture.current) { gesture.current = { pointerId: null, element: event.currentTarget, changed: false }; setHeld(true); }
		keys.current.add(event.key); keyboardVector();
	};
	return <div className="fam-position-aim">
		<div className={`fam-position-joystick${held ? " is-held" : ""}`} role="application" tabIndex={0} aria-label="Position aim joystick" data-testid="position-joystick"
			data-active={held} data-pan-rate={velocity.current.x.toFixed(2)} data-tilt-rate={velocity.current.y.toFixed(2)}
			onPointerDown={event => {
				if (gesture.current) return;
				event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId);
				gesture.current = { pointerId: event.pointerId, element: event.currentTarget, changed: false }; setHeld(true); setVector(point(event));
			}}
			onPointerMove={event => { if (gesture.current?.pointerId === event.pointerId) setVector(point(event)); }}
			onPointerUp={event => { if (gesture.current?.pointerId === event.pointerId) finish(); }}
			onPointerCancel={event => { if (gesture.current?.pointerId === event.pointerId) finish(); }}
			onLostPointerCapture={event => { if (gesture.current?.pointerId === event.pointerId) finish(); }} onBlur={finish}
			onKeyDown={keyDown} onKeyUp={event => { if (!keys.current.delete(event.key)) return; event.preventDefault(); if (keys.current.size) keyboardVector(); else finish(); }}>
			<span className="fam-position-joystick-ring" aria-hidden="true" /><span className="fam-position-joystick-neutral" aria-hidden="true" />
			<span className="fam-position-joystick-marker" data-testid="joystick-marker" style={{ left: `${50 + marker.x * 42}%`, top: `${50 + marker.y * 42}%` }} aria-hidden="true" />
			<span className="fam-position-joystick-label">Aim</span>
		</div>
		<p>Hold away from center to move · release to stop</p>
	</div>;
});

export function PositionProgrammingEditor({ pan, tilt, onAngles, onClose, onGestureEnd }: PositionProgrammingEditorProps) {
	const panControl = useRef<StopHandle>(null), joystick = useRef<StopHandle>(null);
	const latest = useRef({ pan, tilt, onAngles, onGestureEnd });
	latest.current = { pan, tilt, onAngles, onGestureEnd };
	const tiltGesture = useRef<{ pointerId: number | null; element: HTMLDivElement; changed: boolean } | null>(null);
	const finishTilt = () => {
		const previous = tiltGesture.current; tiltGesture.current = null;
		if (previous?.pointerId != null) {
			const slider = previous.element.querySelector<HTMLElement>('[role="slider"], input[type="range"]');
			if (slider?.hasPointerCapture(previous.pointerId)) slider.releasePointerCapture(previous.pointerId);
		}
		if (previous?.changed) latest.current.onGestureEnd?.();
	};
	useEffect(() => { window.addEventListener("blur", finishTilt); return () => { window.removeEventListener("blur", finishTilt); tiltGesture.current = null; }; }, []);
	const close = () => { joystick.current?.stop(); panControl.current?.stop(); finishTilt(); onClose(); };
	return <ModalFrame title="Position" ariaLabel="Position Special Dialog" closeLabel="Close Position Special Dialog" onClose={close}
		dialogClassName="fixture-abstraction-panel fam-position-control-modal" className="fixture-abstraction-layer fam-position-control-layer">
		<div className="fam-position-control-body" data-testid="editor-page">
			<div className="fam-position-layout"><div className="fam-position-angle-controls">
				<PanCircle ref={panControl} value={pan} onChange={value => onAngles(value, latest.current.tilt)} onGestureEnd={onGestureEnd} />
				<div className="fam-position-tilt">
					<div onPointerDownCapture={event => { joystick.current?.stop(); tiltGesture.current = { pointerId: event.pointerId, element: event.currentTarget, changed: false }; }}
						onPointerUp={event => { if (tiltGesture.current?.pointerId === event.pointerId) finishTilt(); }} onPointerCancel={event => { if (tiltGesture.current?.pointerId === event.pointerId) finishTilt(); }}
						onLostPointerCapture={event => { if (tiltGesture.current?.pointerId === event.pointerId) finishTilt(); }}
						onKeyDownCapture={event => { if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key) && !tiltGesture.current) tiltGesture.current = { pointerId: null, element: event.currentTarget, changed: false }; }}
						onKeyUp={finishTilt} onBlur={finishTilt}>
						<RangeFader label="Tilt angle" value={tilt} min={TILT_MIN} max={TILT_MAX} step={.1} format={value => `${value.toFixed(1)}°`} allowRange={false}
							onChange={value => { if (value === latest.current.tilt) return; if (tiltGesture.current) tiltGesture.current.changed = true; onAngles(latest.current.pan, value); }} />
					</div>
					<div className="fam-position-tilt-limits"><span>−135°</span><span>0°</span><span>+135°</span></div>
				</div>
			</div>
			<AimJoystick ref={joystick} pan={pan} tilt={tilt} onAngles={onAngles} onGestureEnd={onGestureEnd} /></div>
		</div>
	</ModalFrame>;
}
