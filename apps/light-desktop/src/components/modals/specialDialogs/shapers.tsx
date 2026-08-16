import type { PointerEvent as ReactPointerEvent } from "react";
import { useId, useRef, useState } from "react";

export interface ShaperAttributeValue {
	value: number;
	mixed: boolean;
}

interface ShapersDialogProps {
	attributes: readonly string[];
	values: Readonly<Record<string, ShaperAttributeValue>>;
	disabled: boolean;
	apply: (attribute: string, value: number) => Promise<void>;
}

interface BladeControl {
	index: number;
	position: string | null;
	angle: string | null;
}

interface BladeGesture {
	pointerId: number;
	control: BladeControl;
	startX: number;
	startY: number;
	startPosition: number;
	startAngle: number;
	axisRadians: number;
}

interface RotationGesture {
	pointerId: number;
	startPointerAngle: number;
	startRotation: number;
}

const VIEWBOX_SIZE = 320;
const CENTER = VIEWBOX_SIZE / 2;
const APERTURE_RADIUS = 108;
const HANDLE_RADIUS = 24;
const BLADE_WIDTH = 360;

function clamp(value: number) {
	return Math.max(0, Math.min(1, value));
}

function wrap(value: number) {
	return ((value % 1) + 1) % 1;
}

function bladeControls(attributes: readonly string[]): BladeControl[] {
	const available = new Set(attributes);
	const controls: BladeControl[] = [];
	for (let index = 1; index <= 4; index += 1) {
		const position = `shaper.blade.${index}.position`;
		const angle = `shaper.blade.${index}.angle`;
		if (!available.has(position) && !available.has(angle)) continue;
		controls.push({
			index,
			position: available.has(position) ? position : null,
			angle: available.has(angle) ? angle : null,
		});
	}
	return controls;
}

function pointerInViewbox(event: ReactPointerEvent<SVGElement>) {
	const bounds = event.currentTarget.ownerSVGElement?.getBoundingClientRect();
	if (!bounds?.width || !bounds.height) return { x: CENTER, y: CENTER };
	return {
		x: ((event.clientX - bounds.left) / bounds.width) * VIEWBOX_SIZE,
		y: ((event.clientY - bounds.top) / bounds.height) * VIEWBOX_SIZE,
	};
}

function bladeBaseAngle(index: number) {
	return (index - 1) * 90 - 90;
}

export function shaperBladeGeometry(
	index: number,
	position: number,
	angle: number,
) {
	const localAngle = (angle - 0.5) * 90;
	const rotation = bladeBaseAngle(index) + 90 + localAngle;
	const radians = ((bladeBaseAngle(index) + localAngle) * Math.PI) / 180;
	const radius = APERTURE_RADIUS * (1 - position * 2);
	return {
		innerEdge: CENTER - radius,
		rotation,
		handle: {
			x: CENTER + Math.cos(radians) * radius,
			y: CENTER + Math.sin(radians) * radius,
		},
	};
}

function shortestAngleDelta(current: number, start: number) {
	let delta = current - start;
	while (delta > Math.PI) delta -= Math.PI * 2;
	while (delta < -Math.PI) delta += Math.PI * 2;
	return delta;
}

function valueFor(
	values: Readonly<Record<string, ShaperAttributeValue>>,
	attribute: string | null,
	fallback: number,
) {
	return attribute ? (values[attribute]?.value ?? fallback) : fallback;
}

export function ShapersDialog({
	attributes,
	values,
	disabled,
	apply,
}: ShapersDialogProps) {
	const controls = bladeControls(attributes);
	const rotationAttribute = attributes.includes("shaper.rotation")
		? "shaper.rotation"
		: null;
	const [preview, setPreview] = useState<Record<string, number>>({});
	const bladeGesture = useRef<BladeGesture | null>(null);
	const rotationGesture = useRef<RotationGesture | null>(null);
	const clipId = `shapers-aperture-${useId().replaceAll(":", "")}`;
	const resolved = (attribute: string | null, fallback: number) =>
		attribute && preview[attribute] !== undefined
			? preview[attribute]
			: valueFor(values, attribute, fallback);
	const moduleRotation = resolved(rotationAttribute, 0.5);

	const write = (attribute: string | null, value: number, cyclic = false) => {
		if (!attribute || disabled) return;
		const next = cyclic ? wrap(value) : clamp(value);
		setPreview((current) => ({ ...current, [attribute]: next }));
		void apply(attribute, next);
	};

	const moveBlade = (event: ReactPointerEvent<SVGCircleElement>) => {
		const gesture = bladeGesture.current;
		if (!gesture || gesture.pointerId !== event.pointerId || disabled) return;
		const point = pointerInViewbox(event);
		const deltaX = point.x - gesture.startX;
		const deltaY = point.y - gesture.startY;
		const radial =
			deltaX * Math.cos(gesture.axisRadians) +
			deltaY * Math.sin(gesture.axisRadians);
		const tangent =
			-deltaX * Math.sin(gesture.axisRadians) +
			deltaY * Math.cos(gesture.axisRadians);
		write(
			gesture.control.position,
			gesture.startPosition - radial / (APERTURE_RADIUS * 2),
		);
		write(gesture.control.angle, gesture.startAngle + tangent / 120);
	};

	const moveRotation = (event: ReactPointerEvent<SVGCircleElement>) => {
		const gesture = rotationGesture.current;
		if (!gesture || gesture.pointerId !== event.pointerId || !rotationAttribute)
			return;
		const point = pointerInViewbox(event);
		const angle = Math.atan2(point.y - CENTER, point.x - CENTER);
		write(
			rotationAttribute,
			gesture.startRotation +
				shortestAngleDelta(angle, gesture.startPointerAngle) / (Math.PI * 2),
			true,
		);
	};

	if (!controls.length && !rotationAttribute)
		return <p>No shaper attributes exist on the selected fixtures.</p>;

	return (
		<div className="shapers-special-dialog">
			<svg
				className="shapers-aperture"
				viewBox={`0 0 ${VIEWBOX_SIZE} ${VIEWBOX_SIZE}`}
				role="img"
				aria-label="Shaper aperture"
			>
				<title>Shaper aperture</title>
				<defs>
					<clipPath id={clipId}>
						<circle cx={CENTER} cy={CENTER} r={APERTURE_RADIUS} />
					</clipPath>
				</defs>
				<circle
					className="shapers-aperture-field"
					cx={CENTER}
					cy={CENTER}
					r={APERTURE_RADIUS}
				/>
				<g
					className="shapers-module"
					style={{ transform: `rotate(${(moduleRotation - 0.5) * 360}deg)` }}
				>
					<g clipPath={`url(#${clipId})`} data-testid="shaper-result-shape">
						{controls.map((control) => {
							const position = resolved(control.position, 0);
							const angle = resolved(control.angle, 0.5);
							const geometry = shaperBladeGeometry(
								control.index,
								position,
								angle,
							);
							return (
								<rect
									key={`plate-${control.index}`}
									className="shaper-blade-plate"
									data-testid={`shaper-blade-plate-${control.index}`}
									data-inner-edge={geometry.innerEdge}
									x={CENTER - BLADE_WIDTH / 2}
									y={-VIEWBOX_SIZE}
									width={BLADE_WIDTH}
									height={geometry.innerEdge + VIEWBOX_SIZE}
									transform={`rotate(${geometry.rotation} ${CENTER} ${CENTER})`}
								/>
							);
						})}
					</g>
					{controls.map((control) => {
						const position = resolved(control.position, 0);
						const angle = resolved(control.angle, 0.5);
						const geometry = shaperBladeGeometry(
							control.index,
							position,
							angle,
						);
						const point = geometry.handle;
						const mixed = Boolean(
							(control.position && values[control.position]?.mixed) ||
								(control.angle && values[control.angle]?.mixed),
						);
						return (
							<g key={control.index}>
								<circle
									className={`shaper-blade-handle${mixed ? " mixed" : ""}`}
									cx={point.x}
									cy={point.y}
									r={HANDLE_RADIUS}
									role="slider"
									aria-label={`Blade ${control.index} insertion and rotation`}
									aria-valuemin={0}
									aria-valuemax={100}
									aria-valuenow={Math.round(position * 100)}
									aria-valuetext={`${Math.round(position * 100)}% inserted, ${Math.round((angle - 0.5) * 90)}°${mixed ? ", mixed" : ""}`}
									aria-disabled={disabled}
									onPointerDown={(event) => {
										if (disabled) return;
										event.currentTarget.setPointerCapture?.(event.pointerId);
										const start = pointerInViewbox(event);
										bladeGesture.current = {
											pointerId: event.pointerId,
											control,
											startX: start.x,
											startY: start.y,
											startPosition: position,
											startAngle: angle,
											axisRadians:
												((bladeBaseAngle(control.index) +
													(angle - 0.5) * 90 +
													(moduleRotation - 0.5) * 360) *
													Math.PI) /
												180,
										};
									}}
									onPointerMove={moveBlade}
									onPointerUp={() => {
										bladeGesture.current = null;
									}}
									onPointerCancel={() => {
										bladeGesture.current = null;
									}}
								/>
								<text className="shaper-blade-number" x={point.x} y={point.y}>
									{control.index}
								</text>
							</g>
						);
					})}
				</g>
				{rotationAttribute && (
					<>
						<circle
							className="shapers-rotation-track"
							cx={CENTER}
							cy={CENTER}
							r="142"
						/>
						<circle
							className="shapers-rotation-hitarea"
							cx={CENTER}
							cy={CENTER}
							r="142"
							role="slider"
							aria-label="Shaper module rotation"
							aria-valuemin={-180}
							aria-valuemax={180}
							aria-valuenow={Math.round((moduleRotation - 0.5) * 360)}
							aria-valuetext={`${Math.round((moduleRotation - 0.5) * 360)}°${values[rotationAttribute]?.mixed ? ", mixed" : ""}`}
							aria-disabled={disabled}
							onPointerDown={(event) => {
								if (disabled) return;
								event.currentTarget.setPointerCapture?.(event.pointerId);
								const point = pointerInViewbox(event);
								rotationGesture.current = {
									pointerId: event.pointerId,
									startPointerAngle: Math.atan2(
										point.y - CENTER,
										point.x - CENTER,
									),
									startRotation: moduleRotation,
								};
							}}
							onPointerMove={moveRotation}
							onPointerUp={() => {
								rotationGesture.current = null;
							}}
							onPointerCancel={() => {
								rotationGesture.current = null;
							}}
						/>
						<circle
							className="shapers-rotation-handle"
							cx={CENTER}
							cy="18"
							r="10"
							style={{
								transformOrigin: `${CENTER}px ${CENTER}px`,
								transform: `rotate(${(moduleRotation - 0.5) * 360}deg)`,
							}}
						/>
					</>
				)}
			</svg>
			<div className="shapers-special-help">
				<b>Shapers</b>
				<span>Move a numbered blade inward to cut the beam.</span>
				<span>Move it sideways to rotate that blade.</span>
				{rotationAttribute && (
					<span>Drag the outer ring to rotate the module.</span>
				)}
			</div>
		</div>
	);
}
