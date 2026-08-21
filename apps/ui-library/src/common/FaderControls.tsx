import {
	useEffect,
	useRef,
	useState,
	type CSSProperties,
	type ReactNode,
} from "react";
import { FormField, Input } from "./controls";

const CONTINUOUS_CHANGE_INTERVAL_MS = 50;

export interface HorizontalFaderProps {
	label: string;
	value: number;
	minimum?: number;
	maximum?: number;
	step?: number;
	display?: ReactNode;
	/**
	 * How to write the fader's own value. A fader that formats its value renders the position the
	 * operator is dragging, rather than a `display` string built from a value that has to travel
	 * to a server and back before it agrees with the bar.
	 */
	displayFormat?: "percent" | "decimal" | "integer";
	disabled?: boolean;
	accentColor?: string;
	className?: string;
	showLabel?: boolean;
	onChange: (value: number) => void;
}

function formatFaderValue(
	value: number,
	format: "percent" | "decimal" | "integer",
) {
	if (format === "percent") return `${Math.round(value * 100)}%`;
	if (format === "integer") return String(Math.round(value));
	return String(Number(value.toFixed(2)));
}

export function HorizontalFader({
	label,
	value,
	minimum = 0,
	maximum = 100,
	step = 0.1,
	display,
	displayFormat,
	disabled = false,
	accentColor,
	className = "",
	showLabel = true,
	onChange,
}: HorizontalFaderProps) {
	const [local, setLocal] = useState(value);
	const dragging = useRef(false);
	const latest = useRef(value);
	const lastEmitted = useRef(value);
	const lastEmittedAt = useRef(0);
	const trailing = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const emit = (next: number) => {
		if (Object.is(next, lastEmitted.current)) return;
		lastEmitted.current = next;
		lastEmittedAt.current = Date.now();
		onChange(next);
	};
	const schedule = () => {
		if (trailing.current !== undefined) return;
		const elapsed = Date.now() - lastEmittedAt.current;
		if (elapsed >= CONTINUOUS_CHANGE_INTERVAL_MS) {
			emit(latest.current);
			return;
		}
		trailing.current = setTimeout(() => {
			trailing.current = undefined;
			emit(latest.current);
		}, CONTINUOUS_CHANGE_INTERVAL_MS - elapsed);
	};
	useEffect(() => {
		if (!dragging.current) {
			setLocal(value);
			latest.current = value;
			lastEmitted.current = value;
		}
	}, [value]);
	useEffect(
		() => () => {
			if (trailing.current !== undefined) clearTimeout(trailing.current);
		},
		[],
	);
	const finish = () => {
		dragging.current = false;
		if (trailing.current !== undefined) {
			clearTimeout(trailing.current);
			trailing.current = undefined;
		}
		emit(latest.current);
	};
	const span = maximum - minimum;
	const fraction =
		span > 0 ? Math.max(0, Math.min(1, (local - minimum) / span)) : 0;
	return (
		<label
			className={`horizontal-touch-fader ${disabled ? "disabled" : ""} ${className}`.trim()}
			style={
				{
					"--fader-level": fraction,
					"--fader-color": accentColor ?? "#176777",
					"--fader-color-dark": accentColor
						? `color-mix(in srgb, ${accentColor} 42%, #081014)`
						: "#103039",
				} as CSSProperties
			}
		>
			{showLabel && <span>{label}</span>}
			<strong>
				{displayFormat
					? formatFaderValue(local, displayFormat)
					: (display ?? `${Math.round(local)}%`)}
			</strong>
			<Input
				aria-label={label}
				disabled={disabled}
				type="range"
				min={minimum}
				max={maximum}
				step={step}
				value={local}
				onPointerDown={() => {
					dragging.current = true;
				}}
				onInput={(event) => {
					const next = Number(event.currentTarget.value);
					latest.current = next;
					setLocal(next);
					schedule();
				}}
				onPointerUp={finish}
				onPointerCancel={finish}
				onBlur={() => {
					if (dragging.current) finish();
				}}
			/>
		</label>
	);
}

export function HorizontalFaderField({
	fieldLabel,
	description,
	error,
	labelPlacement,
	...props
}: HorizontalFaderProps & {
	fieldLabel?: ReactNode;
	description?: ReactNode;
	error?: ReactNode;
	labelPlacement?: "side" | "top";
}) {
	return (
		<FormField
			label={fieldLabel ?? props.label}
			description={description}
			error={error}
			labelPlacement={labelPlacement}
		>
			<HorizontalFader {...props} showLabel={false} />
		</FormField>
	);
}

/** Compatibility name for existing operator surfaces. */
export const HorizontalTouchFader = HorizontalFader;
